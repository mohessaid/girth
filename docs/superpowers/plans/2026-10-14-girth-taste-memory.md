# Girth v0.3 Taste Memory: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Cross-project house rules and a variation history per domain, delivered through the existing Skill hook as `additionalContext`.

**Architecture:**
- A new module, `taste`, covers events, tiers, replay, the merged view, rendering and domain resolution.
- It is wired into the clap CLI (`taste`, `made`), into `record use` (hook output) and into `validate` (link events).
- The skill text gains a taste section and `references/taste.md`.

**Tech Stack:** Rust 2024, serde, proptest, assert_cmd, tempfile. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-14-girth-taste-memory-design.md`

## Global Constraints
- The files are `<cwd>/.girth/taste.jsonl` (project) and `GIRTH_HOME/taste.jsonl` (personal). Every test sets `GIRTH_HOME` to a temp dir.
- Events are `v: 1`. Readers skip malformed lines and unknown kinds.
- `record use` always exits 0, and prints hook JSON only when taste exists for a resolved domain.
- `share` never reads taste files.
- Frozen v0.2 strings stay unchanged. New frozen strings:
  - `remembered for `, `remembered N choices for `
  - `taste: `, `(nothing remembered yet)`
  - `no taste remembered yet`
  - `house rules (always apply):`, `recently made (newest first; vary these unless a rule or the user pins them):`
- Version 0.3.0.

## Review Focus
1. **Dropping a rule in one tier must not hide the same text in the other tier.** (Task 1)
2. **Hook stdin for an unrelated skill, or with no taste,** must print nothing; stdout must stay empty so the hook adds no context. (Task 3)
3. **`KEY=VALUE` with `=` inside the value** (`url=a=b`) keeps everything after the first `=`. An empty key or value is rejected. (Task 2)
4. **A taste file that is a directory or unreadable** must read as empty, never crash the hook. (Task 1)
5. **A rule text equal to a skill name or path** must still never appear in `share preview`. (Task 3)

---

### Task 1: `taste` module
**Produces:**
- `pub enum TasteEvent { Rule{domain, text, op}, Made{domain, project, choices: BTreeMap<String,String>}, Link{domain, skill} }`, serde-tagged `kind`, with `v`/`ts` in a wrapper like `ledger`.
- `pub enum Tier { Personal, Project }`
- `pub fn path(tier, home: &Path, cwd: &Path) -> PathBuf`
- `pub fn append(path: &Path, ev: TasteEvent) -> io::Result<()>`
- `pub fn read(path: &Path) -> Vec<TasteEvent>`
- `pub fn view(personal: &[TasteEvent], project: &[TasteEvent], domain: &str) -> TasteView { rules: Vec<RuleRow{n, text, tier}>, recent: Vec<(String, Vec<String>)> }`
- `pub fn domains(...) -> Vec<DomainRow{domain, rules, made}>`
- `pub fn resolve_domains(skill: &str, events: &[TasteEvent]) -> Vec<String>` (cap 2)
- `pub fn render(view, launcher) -> String`
- `pub fn slug_key(&str) -> String`
- `pub fn is_domain(&str) -> bool`

**Tests:**
- Round trip.
- Malformed and unknown lines are skipped; a directory path reads as empty.
- Replay of add/drop, duplicates and per-tier drops.
- Recent values: newest first, 3 distinct per key, 8 keys, ordered by recency.
- `resolve_domains` handles the prefix, links, the cap and namespaced `plugin:girth-x`.
- Render: empty, full, and the launcher path.
- Proptest: no panic; no dropped rule shown; no duplicate recent values.

### Task 2: CLI `taste` and `made`
**Tests (CLI):**
- `taste` with no data → `no taste remembered yet`.
- add (personal and `--project`) → show lists both, the project one tagged.
- drop N → removed; a missing N → exit 1.
- An invalid domain or empty text → exit 2.
- `show --json` shape.
- `made` with `k=v` pairs → remembered N; `url=a=b` keeps `a=b`; a bad pair → exit 2.
- `taste` list counts.

### Task 3: Hook context, link events, share isolation, stats counts
- `record use` prints `{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":…}}` for each resolved domain with taste (one JSON object; multiple domains are joined with a blank line), capped at 1,500 bytes. `GIRTH_LAUNCHER` env feeds `render`.
- `validate` (recording) appends `Link` events for each distinct source skill in `src` markers, plus the crafted name, unless a link for that pair already exists.
- `stats` text adds `taste: D domains, R rules, M made` when any taste exists, and its JSON adds a `taste` object.

**Tests:**
- Hook with taste → JSON containing the rule; unrelated skill → empty stdout; bad stdin → empty.
- A link resolves a source skill.
- `share preview` with sentinel taste → sentinel absent.
- Stats counts.

### Task 4: Launcher, skill text, reference
- The launcher sets `export GIRTH_LAUNCHER="$here/girth"` (and the version bump comes in Task 5).
- `SKILL.md`: an intents row, step 1 domain reuse, a taste section, the additive rule.
- `references/taste.md`: remember / forget / show flows.
- Tests in `tests/skill_md.rs` and `tests/launcher.rs`.
- Measure always-on bytes before and after.

### Task 5: Eval, version, docs, gates, release
- `evals/taste-launch` with its scaffold and graders.
- `docs/perf.md`: hook taste path on a 1K-event taste file.
- README "Taste memory" section, CHANGELOG, versions 0.3.0.
- `make check`, mutants ≥ 80%, `make evals` (4 of 4).
- Merge, push, CI green, tag `v0.3.0`, update the local plugin.
