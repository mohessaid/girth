# Girth v0.5 Lens and Meter: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** a per-project skill listing (Lens) and real per-turn usage in the ledger (Meter), delivered as a thin mod in the existing plugin and decided by the Rust binary. Plus a capped SkillsBench proof.

**Architecture:**
- New Rust modules `lens`, `meter` and `price`.
- New ledger events `seen` and `turn`.
- New CLI commands `girth lens` and `girth meter record`, and a meter section in `girth stats`.
- `hooks/girth.js` (the mod) is listed in `hooks/hooks.json` `modules`, with tests under `hooks/tests/`.

**Tech Stack:** Rust 2024, serde, proptest, assert_cmd; Claude Code mods (JS) and `claude plugin test`; Harbor + Docker for SkillsBench.

**Spec:** `docs/superpowers/specs/2026-10-15-girth-lens-meter-design.md`

## Global Constraints
- **Lens never loses a name:** output names equal input names, and described entries are byte-identical.
- **Every mod hook fails open**, with a `.catch` → `next(e)`.
- **`share` never reads `turn` or `seen` content.**
- **New frozen strings:**
  - `- Also installed (names only; ask \`girth search "<need>"\` which fits): `
  - `meter: `
- **Tests never touch the real `~/.girth`** (`GIRTH_HOME` is a temp dir). Live runs export `GIRTH_HOME` for the whole process.
- **Version** 0.5.0.

## Review Focus
1. **A listing entry whose description spans several lines,** or contains `- ` mid-line, must stay one entry.
2. **Plugin-qualified names** (`superpowers:brainstorming`) match ledger `use` events exactly, and crafted detection uses the base name.
3. **The baseline:** on the first run every name is baseline (not "new"), so Lens is effective from day one.
4. **The mod memoises by input text,** so the rewrite is stable across requests and the prompt cache stays warm.
5. **`turn.complete` without `usage`** (aborted, or refused before any request) records nothing.

---

### Task 1: Ledger events `seen` and `turn`
- `Event::Seen { names: Vec<String> }` and `Event::Turn { project, model, input, output, cache_read, cache_write, agent, aborted, lens_before: Option<u64>, lens_after: Option<u64> }`.
- `share::payload` ignores both. Its property test is extended with a sentinel project path in `turn`.
- `stats` compiles: its match arms ignore both.
- **Tests:** round trip; share sentinel; old ledgers still parse.

### Task 2: `lens` module and `girth lens`
- `parse(&str) -> Listing { header, entries: Vec<Entry{name, text}>, trailing }`.
- `Policy::from(records, project, config, now)`.
- `rewrite(&Listing, &Policy) -> Rewrite { text, described, collapsed }`.
- `new_names(records, names) -> Vec<String>`.
- **CLI:** reads stdin, `--project`, `--json`. Appends a `seen` event. Honours `GIRTH_LENS=off` and `lens.enabled=false`.
- **Config:** `lens: { enabled, keep }` in project and personal `config.json`. Unknown keys are tolerated.
- **Tests:** the spec §8 unit list, the property tests, and the CLI tests.

### Task 3: `meter` and `price`, and `girth meter record` and `stats --meter`
- `price::rate(model) -> Option<Rates>`. Rates are verified against the claude-api skill's pricing before they're written.
- `meter::summarise(records, project) -> MeterSummary { turns, input, output, cache_read, cache_write, cost: Option<f64>, lens_saved_tokens, lens_saved_cost }`.
- **CLI:** `girth meter record` reads JSON from stdin and appends a `turn` event. `stats` text gains `meter: …` lines; JSON gains a `meter` object. `stats --project DIR` filters.

### Task 4: The mod, the skill line and the README
- **`hooks/girth.js`:**
  - `prompt.attachment {type: 'skill_listing'}` → memoised `girth lens --project <cwd> --json`;
  - `turn.complete` → `girth meter record`;
  - `session.start` registers `/girth-meter`;
  - `command.run {command: 'girth-meter'}` → `girth stats --project <cwd> --meter`.
- **`hooks/tests/girth.test.ts`:** the spec §8 mod tests. The Makefile `check` gains `modtest: claude plugin test`.
- **`skills/girth/SKILL.md`:** the "Also installed" line. Always-on cost is unchanged.
- **README:** Lens and Meter, the requirement (2.1.287+), "Updating" (auto-update and `/reload-plugins`), and the Lens limits.

### Task 5: Behavioural proof of Lens
- The `lens-hidden-skill` eval, if plugin eval loads mods; otherwise `examples/lens/hidden.sh`, with the reason recorded.
- **`examples/lens/ab.sh`:** a live A/B with the real configuration and isolated `GIRTH_HOME`; results in `examples/lens/RESULTS.md`.

### Task 6: SkillsBench proof (`examples/skillsbench/`)
- Install Harbor; pick 6 tasks; scripts for the none, curated and girth arms; the craft step; a $10 cap; RESULTS.md.

### Task 7: Gates and release
- `make check`, `make mutate` (≥ 80% on the new modules), `make evals`.
- CHANGELOG, versions 0.5.0, merge, CI, tag, release, update the local plugin.
