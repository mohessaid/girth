# Girth v0.8: idle compaction and a subagent model (design)

**Status:** the shape was approved in conversation on 2026-10-16: both features opt-in, and the keeper stays on by default. This written spec is pending the author's review.
**Evidence:** `docs/research/2026-10-15-where-the-money-goes.md`, `examples/compact/SPIKE.md`.

## Why
- **Cache reads are 62% of the author's main-thread spend.** The median request re-reads 387K tokens.
- **Compacting a large conversation after 55 minutes idle,** while the 1-hour cache is still warm, would have saved about $1.8K net on the author's history. That is roughly $1.4K more than the keeper.
  - The return then writes a ~50K context instead of the whole one, and every later request reads less.
- **Subagents are 6% of spend.** A cheaper subagent model is a small, opt-in saving, built because the user asked for it.

## Feature 1: idle compaction (opt-in)
- **Config:** `{"compact": {"idle": true, "min_context": 150000}}` in `~/.girth/config.json` or the project's `.girth/config.json`.
  - `idle` defaults to false. If either file sets `"idle": false`, compaction stays off.
  - For `min_context` the project file wins, and the default is 150,000.
  - `GIRTH_COMPACT=off` turns it off everywhere.
- **Plan:** `girth meter record` adds `"compact": true` to the keeper plan when compaction is on, the turn is the main conversation's, and its context is at least `min_context`.
  - A compact plan has `pings: 0`. For a large context the compaction replaces the keeper's pings; the two would fire at the same moment.
  - It is independent of `keeper.enabled`.
- **Mod:**
  - **Arming:** after a main `turn.complete` whose plan says compact, one timer is armed at `interval_ms` (55 min).
  - **Cancelling:** `turn.start` (main) cancels it, as with the keeper.
  - **Firing:** when the timer fires, `$.session.compact()` runs with the engine's own prompt.
    - If it succeeds, Girth records a `compact` event (`project`, `model`, `before`, `after` tokens) with `girth compact record`.
    - It remembers a note for the next main `turn.complete`, which adds one line under the answer: `girth: compacted this conversation while you were away (583K → 21K tokens)`.
    - If it is refused (headless, a hook said skip, or it threw), nothing is recorded and no line appears.
- **Stats:** `girth stats --meter` adds `idle compaction: N times · 583K → 21K tokens on average`. `share` never reads `compact` events.
- **Interactive only.** `$.session.compact` rejects in `claude -p`/SDK, and timers end with the process anyway.

## Feature 2: subagent model (opt-in)
- **Config:** `{"agents": {"model": "sonnet"}}`. The value is an alias (`sonnet`, `haiku`, `opus`) or a full model id. The project file wins, and there is no default.
- **CLI:** `girth settings --project <dir>` prints `{"agents":{"model":"sonnet"}}`, or `{}` when nothing is set.
- **Mod:**
  - At `session.start`, read the settings once. On failure, use none.
  - On `agent.spawn`, set `model` only when all of these hold:
    - a model is configured;
    - `e.model` is undefined, so a choice by Claude or by the agent's definition (when the engine passes it) wins;
    - the agent is built in (`provider.plugin === 'engine'`);
    - it is not a `fork`, since forks always inherit.
  - Otherwise pass the event on unchanged.
- **Meter** already records each subagent turn's model, so the saving shows in `/girth-meter`.

## Review focus
1. **A compaction timer from an earlier turn must never fire after a new turn has started,** or it would compact mid-work.
2. **Compaction must stay opt-in.** No config means no `compact` in the plan, and pings exactly as in v0.7.
3. **A refused or failed compaction** records nothing, shows no line and never throws out of a hook.
4. **`agent.spawn` must never override an explicit model, a plugin's agent or a fork.**
5. **An old ledger without `compact` events** parses, and stats are unchanged for it.

## Testing
- **Rust:**
  - settings precedence (home vs project, `idle:false` wins, env off);
  - plan (compact replaces pings only at or above `min_context`);
  - the ledger round trip and old-line parse;
  - the stats line and the `share` sentinel;
  - CLI tests for `girth settings`, `girth compact record` and `meter record`'s plan.
- **Mod (`claude plugin test`):**
  - the compactor arms, cancels on a new turn, records, and leaves a note for the next turn's line;
  - failure paths;
  - the `agent.spawn` rules, with one test per case.
- **Mutants** scoped to the changed files.
- **Live:**
  - one tmux run of the installed plugin with a short interval (`GIRTH_KEEPER_INTERVAL_MS`), checking the compaction, the ledger event and the line on the next turn;
  - one `claude -p` run with `agents.model` set, checking that the subagent's model in the transcript is the configured one.
- **Quality A/B (before any talk of a default):**
  - the author turns idle compaction on for a week;
  - Meter and the `compact` events give the cost side;
  - the author's judgement gives the quality side.
