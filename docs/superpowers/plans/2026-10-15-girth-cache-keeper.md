# Girth v0.7 Cache keeper and doctor: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** stop paying to rewrite the prompt cache after idle gaps (keeper), show when it happens (doctor), and price 1-hour cache writes correctly.

**Architecture:**
- **Rust:** `price` gets 1-hour write rates. The ledger gets `Event::Ping` and `Turn.breaks`. A new `keeper` module holds the plan, the doctor line and the stats aggregation. `girth meter record` prints `{keeper, line?}`; `girth keeper record` appends pings.
- **Mod:** a `turn.step` break detector, timers after main turns, and an exported `ping` step with injected dependencies.

**Spec:** `docs/superpowers/specs/2026-10-15-girth-cache-keeper-design.md`

## Global Constraints
- **Write rates are 2× base input:** opus 8.0, sonnet 4.0, haiku 0.20, fable 20.0.
- **Defaults:** keeper `max_pings` 3, `min_context` 20,000, interval 55 min.
- **Break threshold:** lost ≥ 5,000 and ≥ 20% of expected, and no shrink below 80%. A line is printed when rewritten ≥ 20,000.
- **Off switches:** `keeper.enabled=false` and `GIRTH_KEEPER=off`. `GIRTH_KEEPER_INTERVAL_MS` is for tests and probes only.
- **`share` never reads `ping` or `breaks`.**
- **Every mod hook fails open.** Tests never touch the real `~/.girth`.
- **Version** 0.7.0.

## Review Focus
1. **A subagent's or background request on another model must never count as a break.** Compare per model, main only.
2. **A shrink (compaction, `/clear`) is not a break.**
3. **Timers from an earlier turn must be cancelled** when a new turn starts, or pings would fire mid-turn.
4. **A ping miss** (read < 50% of expected) stops the rest of that idle period's pings.
5. **An old ledger** without `breaks` or `ping` still parses, and stats are unchanged for it.

---

### Task 1: 1-hour cache-write prices
Test first: the rates table expects the new write rates, plus a cost check that reproduces the logged Haiku run (input 2+…, `total_cost_usd` 0.01462). Then update `price.rs` and its doc comment.

### Task 2: Ledger: `Event::Ping` and `Turn.breaks`
- `Ping { project, model, read, write, input, output, expected }`.
- `Break { lost, rewritten, idle_s }`, and `Turn.breaks: Vec<Break>` (default, skipped when empty).
- **Tests:** round trip; old lines parse; `share` sentinel; `stats` match arms.

### Task 3: `keeper` module
- `plan(config, env, context) -> Plan { pings, interval_ms }`.
- `doctor_line(turn) -> Option<String>`.
- `summarise(records, project) -> KeeperSummary { breaks, rewritten, rewrite_cost, pings, ping_cost, warm_returns, warm_saved }`, plus `render`.
- **Config:** `keeper: { enabled, max_pings, min_context }`.
- **Tests:** each rule, including the warm-return definition (≥ 60 min gap, a ping in between, no break).

### Task 4: CLI
- `meter record` prints `{"keeper":{…},"line":…}`.
- `keeper record` (stdin ping JSON) appends a `ping` event.
- `stats --meter` gains the keeper lines.
- **Tests:** CLI tests for each.

### Task 5: Mod: doctor
- **`turn.step`:** for main requests, per-model previous `{at, read, write}`; compute breaks, collected per turn.
- **`turn.complete`:** send `breaks`; parse the JSON stdout; return `{ text: line }` when present.
- **Tests:** healthy, break, shrink, other model, agent; the line returned; fail-open.

### Task 6: Mod: keeper (only after the probe passes)
- **After a main `turn.complete`,** arm timers per the plan, and cancel them on `turn.start`.
- **Exported `ping(deps, state)`:** `deps.fork` and `deps.record`; a miss stops later pings.
- **Tests:** `clock.after` delays, cancellation, agent and zero plans, ping record, a miss.

### Task 7: Live checks, docs, gates, release
- Write the probe results to `examples/cache/RESULTS.md`, and run one isolated end-to-end with a short interval.
- README (the trade-off, plus the subscription note), CHANGELOG.
- `make check`, mutants, evals; release 0.7.0.
