# Girth: Claude Code's side queries

Status: design, 2026-11-08.

Evidence:
- `examples/sidequery/RESULTS.md`
- `docs/radar/2026-11-08.md`, which reports that `agent_summary` takes 16–26% of spend

## Why

Claude Code also sends model requests outside the conversation. They are progress summaries of background agents (`agent_summary`), prompt suggestions, session titles, compaction and more.

The probe showed two facts:
- The usage in Girth's `turn.complete` events equals the main-loop and subagent requests exactly.
- None of the side requests is in any turn.

So Girth's meter and `advise` under-count Claude Code spend by the whole side-query share. They also can't name the largest piece of that share, which one environment variable removes.

## What

1. **The mod hooks `telemetry.log` with `{ to: 'collector' }`.** These records reach mods with telemetry off.
   - For each `api_request` record whose `query_source` is not a turn source, it adds to a running total for each `(query_source, model)` pair: requests, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens` and `cost_usd_micros`.
   - **Turn sources:** `repl_main_thread`, `sdk`, and anything starting `agent:`. These are already counted through `turn.complete`.
   - **Also skipped:**
     - `hook_prompt`, the source of mod forks. Girth's own keeper pings are recorded as `ping` events.
     - `plugin_model_complete` and `plugin_model_classify`.
   - **Privacy rule:** copy only those numbers, the model id and `query_source`. Never `user.*`, `organization.id`, `session.id`, `prompt.id` or request ids.
   - The hook is gating, so it always returns `next(e)`, catches every error, never awaits a process and never forks.
2. **Flush points:**
   - When a main-loop turn completes, after its `meter record`.
   - At `session.end`.
   - Each flush runs `girth side record` once, with a JSON array of totals on stdin, then clears the totals.
3. **A new ledger event** `side`, holding `project`, `session?`, `source`, `model`, `requests`, `input`, `output`, `cache_read`, `cache_write` and `cost_micros?`.
   - Older Girth versions skip it, because unknown kinds fail to parse and are dropped.
   - Its cost is `cost_micros` when present. Otherwise it is list price at main-loop rates.
4. **Meter:**
   - Side cost is added to `cost`, to `since_commit` and to a new `side_cost` / `side_requests`. It is not added to `turns`.
   - `girth stats` prints `meter: side queries N requests ≈ $X (largest: <source> $Y)`.
5. **Advise:**
   - Side cost is part of `spend` and of the `claude-code` harness.
   - One contributor, of kind `agent_summary` and label "background-agent progress summaries", is the `agent_summary` source's cost. Its fix is: `set CLAUDE_CODE_DISABLE_AGENT_PROGRESS_SUMMARIES=1 (Claude Code 2.1.319: progress lines for background agents stop)`. Its guard is `you watch background agents' progress lines`.
   - Other sources count in `spend` only. No fix for them has been verified yet.

## Not in scope

- Replacing list prices with `cost_usd_micros` for turns.
- Using the measured 5-minute and 1-hour write split.
- Fixes for `prompt_suggestion` and `compact`.

Each is a follow-up of one spec line.

## Tests

- **Rust, unit tests:**
  - `side` round-trips through the ledger.
  - `side record` appends one event per array item, and bad input is ignored.
  - Meter adds side cost but no turns.
  - `advise` adds side cost to spend and names the `agent_summary` contributor with its fix.
- **Mod (`claude plugin test`):**
  - Only side sources are totalled, and identity fields never reach the record.
  - The totals flush at main-loop `turn.complete` and at `session.end`.
  - If the test harness can't raise `telemetry.log`, the headless probe in `examples/sidequery` is the test.
- **End to end:** an interactive tmux session with a background subagent records a `side` event with `source: agent_summary`.
