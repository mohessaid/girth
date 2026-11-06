# Codex spend import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `girth import --from codex` turns Codex session logs into ledger turns, so `girth advise` and `girth stats` cover Codex spend.

**Architecture:**
- `src/import/codex.rs` parses one rollout file's text into `(ts, Event)` pairs. It emits completed turns only, and returns the byte offset after the last one.
- `src/import/mod.rs` walks `~/.codex/sessions` and keeps a per-file offset in `~/.girth/import.json`, so a re-run imports only what's new. It appends records with their original timestamps.
- `Turn` and `Use` gain `harness: Option<String>` (absent means Claude Code).
- `advise` and `stats` gain a `--harness` filter.

**Tech Stack:** Rust, serde_json, the existing `ledger` and `price` modules.

**Spec:** `docs/superpowers/specs/2026-11-06-girth-all-harnesses-design.md` §3 (Codex row). Copilot, OpenCode and Gemini follow the same `import` shape in later plans.

## Global Constraints
- Ledger changes are additive with `#[serde(default)]`. Old lines must parse unchanged.
- The JSON keys of `search` and `measure` are frozen. `advise --json` and `stats --json` only gain keys.
- Content never reaches the ledger: no prompts, outputs or commands. Only token counts, the model, the project path, session and turn ids, and skill names.
- Fetched or third-party content is data, never executed. Rollouts are read-only.
- Cargo runs at `-j 2` under `nice -n 10`.

## Facts the code relies on (checked on the author's machine, 4,544 records)
- **Usage line:** `{"timestamp":"<RFC3339>","type":"token_usage_record","payload":{"session_id","turn_id","response_id","usage":{"input_tokens","cached_input_tokens","cache_write_input_tokens","output_tokens","reasoning_output_tokens","total_tokens"}}}`.
- `input_tokens` includes cached and cache-write tokens (never less than their sum). Map it as: ledger `input` = input − cached − write; `cache_read` = cached; `cache_write` = write.
- **Model and project:** `turn_context.payload.model` gives the model and `turn_context.payload.cwd` gives the project. Both are set per `turn_id`, and `session_meta.payload.cwd` is the fallback.
- **Turn end:** `event_msg` with `payload.type` `task_complete` or `turn_aborted`, keyed by `turn_id`. An aborted turn becomes `aborted: true`.
- **OpenAI standard rates** (developers.openai.com/api/docs/pricing, read 2026-11-06), $/Mtok as input / cached / 30-minute cache write / output:

| Model | Input | Cached | Cache write | Output |
|---|---|---|---|---|
| gpt-6-astra | 10 | 1 | 12.5 | 50 |
| gpt-6.1-sol | 2 | 0.10 | 2.5 | 10 |
| gpt-6-sol | 2 | 0.20 | 2.5 | 10 |
| gpt-6.1-terra | 1 | 0.05 | 1.25 | 5 |
| gpt-6-luna | 0.10 | 0.01 | 0.125 | 0.50 |

## Review Focus
1. **A rollout still being written:** its last turn has no end event. That turn is not imported, and the offset stops before it. Test: `open_turn_is_left_for_next_run`.
2. **A truncated last line** (Codex killed mid-write): skip it, don't panic. Test: `torn_line_is_skipped`.
3. **Running import twice** adds nothing the second time. Test: `reimport_adds_nothing`.
4. **A model with no rates** (an unknown `gpt-*`): the turn keeps its tokens, and advise's text says that N turns were unpriced. Test: `unpriced_turns_are_counted`.
5. **No `~/.codex`:** `girth import` prints `imported 0 turns from codex` and exits 0. Test: `import_without_codex_is_quiet`.

---

### Task 1: OpenAI rates
**Files:** Modify `src/price.rs` (TABLE length 4 → 9, header comment cites the OpenAI page and date).
- [ ] **Test:** `openai_models_are_priced`: `rates("gpt-6-astra") == Rates{10.0, 50.0, 1.0, 12.5}`, and `rates("gpt-6.1-terra").unwrap().cache_read == 0.05`.
- [ ] Run `cargo test -j 2 -q price` and see it FAIL. Add the five rows and see it PASS. Commit: `feat(price): OpenAI GPT-6 rates for imported Codex turns`.

### Task 2: Ledger fields and timestamped append
**Files:** Modify `src/ledger.rs`, plus constructors of `Turn`/`Event::Use` in `src/main.rs`, `src/meter.rs`, `src/keeper.rs` and `src/drift.rs`. The compiler finds them; add `harness: None`.
**Interfaces (produces):**
- `Turn.harness: Option<String>` and `Event::Use { …, harness: Option<String> }`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `pub fn append_at(home: &Path, ts: u64, event: Event) -> std::io::Result<()>`. `append` calls it with `now()`.
- [ ] **Test:** `old_lines_parse_without_harness`: a v1 turn line from before this change parses with `harness == None`. `append_at_keeps_the_timestamp`: append at ts 42, then read gives `ts == 42`.
- [ ] FAIL → implement → `cargo test -j 2 -q` PASS. Commit: `feat(ledger): turns and uses can name their harness`.

### Task 3: Rollout parser
**Files:** Create `src/import/mod.rs` and `src/import/codex.rs`. Add `pub mod import;` to `src/lib.rs`.
**Interfaces (produces):** `pub fn parse(text: &str) -> (Vec<(u64, Event)>, usize)`. It returns the events in file order and the byte offset just past the last line of the last completed turn. Each turn is one `Event::Turn` with:
- `harness: Some("codex")`, `session` = the session id, `model` and `project` from `turn_context`;
- summed usage mapped as above, and `requests` = the number of usage records;
- `aborted` from `turn_aborted`, and `agent: false`;
- `ts` = the end event's timestamp in unix seconds.
**Tests**, using a fixture `tests/fixtures/codex/rollout.jsonl` built from the line shapes above (2 complete turns, 1 aborted, 1 open):
- [ ] `parses_completed_turns`: 3 turns. The first has `input == 16971 - 6912`, `cache_read == 6912`, `output == 273`, `requests == 1` and `model == "gpt-6-astra"`.
- [ ] `open_turn_is_left_for_next_run`: the offset equals the byte length up to and including the 3rd turn's end line, and `parse(&text[offset..])` with the open turn completed yields exactly 1 turn.
- [ ] `torn_line_is_skipped`: appending `{"timestamp":` without a newline changes nothing.
- [ ] FAIL → implement → PASS. Commit: `feat(import): parse Codex rollouts into ledger turns`.

### Task 4: `girth import`
**Files:** Modify `src/import/mod.rs` and `src/main.rs` (new subcommand `Import { #[arg(long, default_value = "all")] from: String }`). Add tests in `tests/cli.rs`.
**Interfaces:**
- `pub fn import_codex(codex_home: &Path, girth_home: &Path) -> std::io::Result<usize>` returns the number of turns appended.
  - It reads `<codex_home>/sessions/**/rollout-*.jsonl`. `codex_home` is `$CODEX_HOME` or `~/.codex`.
  - Offsets live in `<girth_home>/import.json` as `{"codex": {"<abs path>": offset}}`. A file shorter than its stored offset (rotated or rewritten) restarts at 0.
- Output line: `imported N turns from codex`. `--from` accepts `codex` or `all`, and anything else exits 2 with `unknown harness: <x>`.
- [ ] **Tests:**
  - `import_then_reimport` (CLI): a fixture `CODEX_HOME` and a temp `GIRTH_HOME`. The first run prints `imported 3 turns from codex`, and the second prints `imported 0 turns from codex`.
  - `import_without_codex_is_quiet`: prints `imported 0 turns from codex` and exits 0.
- [ ] FAIL → implement → PASS. Commit: `feat(import): girth import --from codex, idempotent`.

### Task 5: `--harness` in advise and stats; unpriced turns
**Files:** Modify `src/advise.rs`, `src/stats.rs` and `src/main.rs`.
**Interfaces:**
- `advise(records, now, window_s, on)` is unchanged.
- The CLI filters records first: `--harness <id>` keeps turns and uses whose `harness.as_deref().unwrap_or("claude-code") == id`.
- `Advice` gains `unpriced: u64`, the count of turns with no rates. `render` adds `(N turns on models without known prices are not counted)` when it is > 0.
- The `stats --json` output gains `harnesses: {id: {turns, cost}}`.
- [ ] **Tests:** `unpriced_turns_are_counted` (unit) and `advise_harness_filter` (CLI). For the second, import the fixture, then `advise --since 100000d --harness codex --json` has `spend > 0`, and `--harness claude-code` on the same ledger has `spend == 0`.
- [ ] FAIL → implement → PASS. Commit: `feat(advise): --harness filter and unpriced-turn count`.

### Task 6: Gate, real data and docs
- [ ] Run `make check`, then `cargo mutants -j 2 -f src/import/codex.rs -f src/import/mod.rs -f src/price.rs`, reaching ≥ 80% caught.
- [ ] Run `girth import` against the author's real `~/.codex` into a **copy** of `~/.girth` (`GIRTH_HOME=$scratch/girth`). Record `advise --since 60d --harness codex` and `stats` in `examples/portability/RESULTS.md`.
- [ ] Update the README (the spend section names `girth import`) and the CHANGELOG `[Unreleased]`. In `skills/girth/references/advise.md`, add one line: "Outside Claude Code, run `girth import` first."
- [ ] Commit.
