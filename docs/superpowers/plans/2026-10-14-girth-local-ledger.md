# Girth v0.2 Local Ledger: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Record skill use, crafted-skill section provenance and diffs, and feedback in a local JSONL ledger. Show it back to the user with `girth stats`, and add a consent flow plus a share-payload preview.

**Architecture:** New modules `sections` (split/slug/normalise/hash/markers), `ledger` (paths, events, append, read), `record` (craft-event construction and diffing), `stats` and `share`. These are wired into the existing clap CLI. A plugin hook records skill use, and the skill text gains a consent step, provenance markers and intent sub-flows in `references/`.

**Tech Stack:** Rust 2024; new deps `sha2` (hashing) and `serde` derive (already present). Tests use proptest, assert_cmd and tempfile.

**Spec:** `docs/superpowers/specs/2026-10-14-girth-local-ledger-design.md`

## Global Constraints
- `GIRTH_HOME` overrides `~/.girth`. Every test sets it to a temp dir.
- Event schema `v: 1`. Readers skip malformed lines.
- `girth record use` always exits 0.
- The share payload holds only source-section hashes and counts. No names, paths, headings or notes.
- Existing frozen interfaces stay unchanged. New output strings are frozen once shipped: `recorded: +A −R sections`, `sharing: unset|allowed|denied`.
- Version 0.2.0 in `Cargo.toml`, `plugin.json` and the launcher's `GIRTH_VERSION`.

## Review Focus
1. **A corrupt or partially written ledger line** (a crash mid-append) must be skipped, never fatal. (Task 2)
2. **Hook stdin** that is empty, non-JSON or missing `tool_input.skill` must record nothing and exit 0. (Task 4)
3. **Validating the same skill twice with no edits** must report `+0 −0`, with no phantom diffs. (Task 3)
4. **A source skill named in a marker that isn't installed** must still record the crafted section, with `total: 0` for that source and no crash. (Task 3)
5. **The share preview** must never leak headings, skill names, paths or notes, even when a heading equals a skill name. (Task 6, property test)

## Plan-format note
This plan lists exact interfaces and test cases instead of pre-written code bodies, because the implementer is the plan's author and works test-first. (Ruling recorded in the ledger.)

---

### Task 1: `sections` module
**Produces:**
- `pub struct Section { pub heading: String, pub slug: String, pub src: Vec<String>, pub hash: String }`
- `pub fn split_sections(body: &str) -> Vec<Section>`
- `pub fn slug(heading: &str) -> String`
- `pub fn normalize(text: &str) -> String`
- `pub fn section_hash(heading_line: &str, content: &str) -> String` (16 hex)
- `pub fn parse_src(heading_line: &str) -> Vec<String>`
- `pub fn strip_html_comments(s: &str) -> String`
- `extract_headings` (in skill.rs) now strips HTML comments, so markers don't pollute search.

**Tests:**
- Splits at each ATX heading and ignores headings inside fenced code.
- Text before the first heading is ignored.
- Slug examples: `"Caption timing"` → `caption-timing`; `"C# & Rust!"` → `c-rust`.
- The hash is stable and ignores case, whitespace and HTML comments.
- `parse_src` handles one or many entries, spaces and missing markers.
- Markers aren't part of the heading text or slug.
- Proptest: never panics, and the hash is invariant under whitespace and case changes.

### Task 2: `ledger` module
**Produces:**
- `pub fn girth_home() -> PathBuf` (`GIRTH_HOME` or `~/.girth`)
- `pub enum Event { Use{..}, Craft{..}, Feedback{..} }` (serde, tagged `kind`)
- `pub fn append(home: &Path, event: &Event) -> io::Result<()>` (creates the dir; one `write_all` of a line)
- `pub fn read(home: &Path) -> Vec<Event>` (skips malformed lines)
- `pub fn now() -> u64`
- Config: `pub fn sharing(home) -> Sharing { Unset | Allowed | Denied }`, `pub fn set_sharing(home, Sharing)`

**Tests:**
- An append/read round trip.
- A missing file reads as empty.
- Malformed and truncated lines are skipped.
- Unknown `kind` is skipped.
- Config: unset by default, round trip, invalid file treated as unset.

### Task 3: `record` (craft events and diffs) + the `validate` hook-in
**Produces:**
- `pub fn craft_event(skill_dir: &Path, project: &Path, roots: &[PathBuf], previous: Option<&Event>) -> Event`
- `pub fn last_craft<'a>(events: &'a [Event], project: &Path, skill: &str) -> Option<&'a Event>`
- The CLI `validate` appends a craft event per VALID `girth-*` dir (unless `--no-record`) and prints `VALID <dir> (recorded: +A −R sections)`.

**Tests:**
- Sections and `src` are captured.
- A source is resolved via roots, giving `total` and `used` slugs plus their hashes.
- An unknown source gives `total: 0`.
- The first craft shows all sections added and none removed.
- An identical re-validate gives `+0 −0`.
- Removing a section gives `removed` with its `src`.
- `--no-record` writes nothing.
- The VALID line stays a prefix match for `\bVALID girth-x`.

### Task 4: `record use` and `feedback` commands, plus the plugin hook
**Produces:**
- `girth record use [--skill NAME]`: reads stdin JSON (`tool_input.skill`, `cwd`) when no flag is given. Exits 0 always.
- `girth feedback SKILL up|down [--note TEXT]`
- `hooks/hooks.json`: `PostToolUse` with matcher `Skill`, running `"${CLAUDE_PLUGIN_ROOT}/skills/girth/scripts/girth" record use 2>/dev/null || true`

**Tests (CLI):**
- Stdin JSON records `use` with the skill name and cwd.
- Namespaced `plugin:skill` is kept as-is.
- Empty, garbage or missing-field stdin → nothing recorded, exit 0.
- `--skill` works without stdin.
- `feedback` up/down records, and an invalid rating is a usage error (exit 2).
- `claude plugin validate .` passes with the hook.

### Task 5: `girth stats`
**Produces:**
- `pub struct Stats` (crafted rows: skill, project, uses, sections, removed_total, up, down, sources[(skill, used, total)]; top uses: (skill, n); sharing)
- `pub fn compute(events: &[Event], sharing) -> Stats`
- A text renderer
- `--json`

**Tests:**
- Uses are counted per skill.
- Crafted rows use the *latest* craft per (project, skill).
- `removed_total` accumulates across crafts.
- Ratings are tallied.
- Source percentages are computed, and `total: 0` renders as `n/a`.
- An empty ledger gives a friendly message.
- The JSON shape test freezes the keys.

### Task 6: `girth share status|allow|deny|preview`
**Produces:**
- `pub fn payload(events: &[Event]) -> SharePayload` (the §6 shape)
- The CLI subcommands

**Tests:**
- status unset/allowed/denied.
- allow and deny persist.
- `preview` aggregates `used`/`kept`/`removed`/`up`/`down` per source hash.
- `skills[].h` is the hash of the sorted section hashes.
- **Proptest:** for random events with random names, notes and headings, the serialised payload contains none of those strings.
- A `preview` header line states that nothing is uploaded in v0.2.

### Task 7: Skill text, references and consent UX
**Files:**
- `skills/girth/SKILL.md`: step 0 consent check; `src` markers in the template; an intents table.
- `skills/girth/references/{review,feedback,stats,sharing}.md`

**Tests (`tests/skill_md.rs`):**
- The body mentions `girth share status`, `<!-- src:`, `references/review.md` and `girth feedback`.
- The body stays ≤ 700 words.
- Each reference file exists and is ≤ 1.5 KB.
- The description is still trigger-only.

### Task 8: Version 0.2.0, docs, performance, gates, evals, release
- Bump versions, CHANGELOG and README (a "Ledger & feedback" section).
- `docs/perf.md`: release-build timings for `record use` and `stats` on a generated 10K-event ledger.
- `make check`, `cargo mutants` (≥ 80%), `make evals` (3/3).
- Commit, push, wait for CI to go green, tag `v0.2.0`, check the release, update the local plugin install (`claude plugin update girth@girth`).
