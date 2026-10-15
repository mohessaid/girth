# Girth v0.7: Cache keeper and cache doctor

**Date:** 2026-10-15
**Status:** Re-scoped on evidence; pending the author's review. The author picked "cache doctor + loop breaker" for v0.7. The evidence below drops the loop breaker and adds the keeper. Scripts: `examples/cache/analyze-breaks.py`, `examples/cache/analyze-loops.py`.
**Builds on:** v0.5 Meter, v0.6 X-ray.

## 1. Evidence (the author's real transcripts, 108 sessions, 30,593 main-conversation requests)
- **A healthy request reads back exactly what the previous one wrote:** `read = prev.read + prev.write`. This was checked on stream-json logs. Any shortfall is cache that was lost and written again.
- **Where cache writes come from:** of 147.1M cache-write tokens, **92.1M (62.6%) followed an idle gap of more than 60 minutes.**
  - Claude Code writes the 1-hour cache (`ephemeral_1h_input_tokens`), and nothing breaks between 5 and 60 minutes of idle (2 cases).
  - Breaks with under 5 minutes idle, meaning a changed prompt prefix, are 2.3% of writes.
  - Compaction and `/clear` (the prompt shrinks) are excluded.
- **Return gaps** for the 264 idle periods of 55 minutes or more that ended in a return: p25 1.7 h, median 3.4 h, p75 11.3 h. Another 106 periods never returned.
- **Simulated keep-warm**, one ping every 55 minutes and at most N pings, pessimistically counting every never-returned period as N wasted pings:

  | N | pings | rewrites avoided | net |
  |---|---|---|---|
  | 1 | $24.60 | $240.11 | +$216 |
  | 3 | $55.79 | $471.55 | **+$416** |
  | 8 | $107.39 | $616.30 | +$509 |

  The dollar figures are at API prices, with the 1-hour cache-write rate (2× base input; see §2). N=3 keeps the cache for about 3¾ hours, and further pings bring diminishing returns.
- **Loop breaker:** across 32,774 tool calls, an identical call with an identical failing result reached 3 attempts **2 times**. It is dropped: no evidence it would pay. The analysis can be re-run if usage changes.

## 2. Mechanism
`$.model.fork({ prompt })` repeats the conversation's last main-thread request, with `skipCacheWrite`, plus a one-line prompt. Its prefix is the conversation's cached prefix, so the API serves it from the cache, and a cache read refreshes the entry's lifetime. One ping costs about `context × read rate`. At Opus 5.5 that is 1/40 of a rewrite: $0.40 against $8 per Mtok, because Claude Code writes the **1-hour** cache at 2× base input.

**Price fix bundled in v0.7:** `src/price.rs` used the 5-minute write rate (1.25×). Recomputing logged runs with the 1-hour rate reproduces Claude Code's `total_cost_usd` exactly (Haiku, two runs) and within 5% (Sonnet). This is the known ~25% Meter under-estimate. The write rates become 2× base input.

**Live probe** (`examples/cache/probe/`): two Haiku sessions, each with a 72.5K-token context of its own.
- **control:** no ping, resumes at 66 minutes.
- **warm:** a fork at 55 minutes, resumes at 66 minutes.

Pass condition: warm's resume reads the context from the cache, and control's writes it again. **v0.7 ships only if the probe passes.**

**Result (2026-10-15): passed.** Warm read 83,641 and wrote 1,840; control wrote 74,319. The return cost 81% less (`examples/cache/RESULTS.md`).

## 3. Keeper (the mod and `girth keeper`)
- **After each main-conversation `turn.complete`,** the mod reads the keeper plan `{pings, interval_ms}` from the JSON that `girth meter record` already prints, so there is no extra process per turn. It then arms one timer (`$.clock.after`) per ping, each at `interval_ms` after the previous activity.
  - `turn.start` cancels the timers.
  - `<tokens>` is the last main request's `read + write + input`.
- **Plan rules (Rust, `src/keeper.rs`):**
  - `pings = 0` when the keeper is disabled (`keeper.enabled=false` or `GIRTH_KEEPER=off`), or when the context is below `keeper.min_context` (default 20,000 tokens). Otherwise `pings = keeper.max_pings` (default 3).
  - `interval_ms = 55 min`.
- **A ping** is `$.model.fork({ prompt: 'Reply with one character: .' })`. Its `usage` goes to `girth keeper record` as a `ping` event `{ model, read, write, input, output, expected }` in the ledger, with numbers only.
- **Self-correction:** if a ping's `read` is below half of `expected`, the cache was already gone (a shorter cache lifetime, or a server-side eviction). The mod stops pinging for that idle period, and the ledger keeps the miss.
- **Fails open:** a fork error or a binary error stops the pings for that period and nothing else.

## 4. Doctor (Meter and stats)
- **`turn.step` observer (main conversation only):** the mod keeps the previous request's `{at, model, read, write}` per model. For each new request it reports `lost = (prev.read + prev.write) − read`. This is counted as a break when `lost ≥ 5,000` and `lost ≥ 20%` of the expected read, and the prompt didn't shrink below 80% of the expected size.
  - Each break goes into the turn record as `breaks: [{ lost, rewritten, idle_s }]`, where `rewritten = min(lost, write)`.
- **`girth meter record`:** stores the breaks in `Turn.breaks` (skipped when empty). It prints one JSON object `{ keeper: {pings, interval_ms}, line? }`. `line` is set when a break has `rewritten ≥ 20,000` tokens, and the mod returns it as `{ text }` under the answer:
  - `girth: the prompt cache had expired (idle 3h 12m); this turn re-wrote 450K tokens (≈ $2.25)`;
  - or, when idle was under 60 minutes: `girth: the prompt changed before the cached part; this turn re-wrote 80K tokens (≈ $0.40)`.
- **`girth stats --meter`** gains:
  - `meter: cache re-written N times: X tokens (≈ $Y)`;
  - `meter: keeper: P pings (≈ $C); R returns found the cache warm (≈ $S not re-written)`.

  A *warm return* is a turn whose first request came at least 60 minutes after the previous one, with a ping in between and no break.
- **`share`** ignores `ping` events and `breaks`.

## 5. Out of scope
- Compacting on return: the keeper prevents the rewrite instead.
- Changing models or effort per step.
- Any upload.

## 6. Testing
- **Rust:**
  - `keeper::plan` (disabled, below the minimum, defaults, config and env precedence);
  - the `ping` round trip;
  - `Turn.breaks` round trip, with old ledgers still parsing;
  - the doctor line (idle versus prefix, the threshold, money from `price`);
  - stats aggregation: re-written totals, pings, warm returns with and without an intervening ping;
  - a `share` sentinel;
  - CLI tests for `keeper plan`, `keeper record` and `meter record` with breaks.
- **Mod tests:** the test harness can't fire `$.clock` callbacks (verified), so the ping step is an exported function with injected `fork` and `record` functions, tested directly. The `clock.after` mock checks that timers are armed with the right delays.
  - timers armed after a main turn and cancelled by `turn.start`;
  - no timers for an agent turn or a zero plan;
  - a ping records usage, and a miss stops later pings;
  - `turn.step` break detection (healthy, break, shrink, a different model);
  - the turn record carries breaks, and the under-answer line is returned;
  - every path fails open.
- **Live:** the probe in §2, plus one isolated end-to-end run of the real mod with `interval_ms` overridden (`GIRTH_KEEPER_INTERVAL_MS`, for tests) to confirm pings land in the ledger.
- **Mutation** ≥ 80% on `keeper` and the new `meter`/`stats` code. `make check` and `make evals` stay green.

## 7. Success criteria
1. The probe shows a warm resume read of at least 90% of the context, against a control rewrite of at least 90%.
2. Mod and Rust tests are green, and mutants are at least 80%.
3. The doctor line and the stats lines appear in an isolated end-to-end run.
4. v0.7.0 is released. The README states the trade-off (a ping costs about 1/40 of a rewrite at API prices and pings stop after 3) and how to turn the keeper off.
   - On a subscription the saving is in plan quota, not dollars. Whether the plan weights cache reads and writes like the API is not verified, so the README says so.
