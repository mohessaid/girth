# Where the money goes (2026-10-15)

Measured from the author's own transcripts with `examples/subagents/analyze-subagents.py` and `examples/context/analyze-context.py`. The scripts print aggregates only. Costs are at API prices, with each cache write priced at its real TTL rate.

## 1. Subagents are a small slice
- **Overall:** main conversations made 32,466 requests and cost **$4,408 (93.8%)**. Subagents made 7,436 requests and cost **$291 (6.2%)**.
- **If every subagent request had run on one model** (same tokens): Opus $364, Sonnet $285, Haiku $14.
  - Moving them all to Haiku would save at most about 6% of total spend.
  - Sonnet barely differs from Opus, because cache reads dominate and both read at $0.20.
- **Subagents write the 5-minute cache** (`ephemeral_5m`), not the 1-hour cache.
  - Girth's Meter prices every write at the 1-hour rate (2× input), so it overstates subagent turns by 0.75× input per written token.
- **Verdict:** "cheap subagents" is a weak v0.8 for this usage. It is deferred.

## 2. Cache reads of a large context are the bill
Opus main thread, by component:

| Component | Cost | Share |
|---|---|---|
| cache reads | $2,695 | **61.7%** |
| cache writes | $1,177 | 26.9% |
| output | $498 | 11.4% |
| uncached input | $0.26 | 0.0% |

- **Context per request:** p25 217K, median **387K**, p95 895K tokens.
- **Every request re-reads the whole conversation.** A block's real cost is its size times the number of later requests that carry it.
- **What the 13.8B re-read tokens are,** computed exactly from the growth of each request's context, not estimated from characters:
  - base (system prompt, tools, compaction summary): 33%
  - earlier model output (thinking, tool calls, text): 27%
  - new input (tool results, prompts, attachments): 40%
- **Most of it is old:** about 88% of re-read conversation content was more than 20 requests old when it was read.

## 3. Compacting on return
- **The idea:** when a session comes back after an hour or more idle with a large context, Claude Code is about to re-write that whole context into the cache anyway. Compacting at that moment changes the arithmetic.
- **Cost model:**
  - compaction call: the context as input at $4/Mtok, plus about 5K output at $20/Mtok;
  - the new base: 60K written at $8/Mtok;
  - the rewrite that is avoided: the full context at $8/Mtok;
  - later reads saved: (context − 60K) × the requests until the next idle period or shrink, at $0.20/Mtok.

| Context at return | Returns | Saved at the return | Later reads saved | Total |
|---|---|---|---|---|
| ≥ 100K | 203 | $295 | $1,210 | $1,505 |
| ≥ 150K | 175 | $298 | $1,195 | **$1,492** |
| ≥ 250K | 148 | $292 | $1,103 | $1,395 |

- **Context at return** (175 returns ≥ 150K): p25 356K, median 583K, p75 797K tokens.
- **Lower bound.** The table assumes the compaction call reads its input uncached at $4/Mtok. If that call also writes the cache (likely, since Claude Code's requests carry `cache_control`), the at-return term flips from +$298 to about −$101 (175 × ($0.48 new base + $0.10 summary)).
  - The total is then about **$1,094**; the later-reads term carries the feature either way.
- **The 60K base is consistent with the data.** 80 compactions in these transcripts left a median `postTokens` of 21,574 (p25 17,891, p75 26,817; 76 auto, 4 manual), plus about 40K of system prompt and tools.
- **The keeper's replay saved $416. This is about 2.6–3.6 times that.**
- **With the keeper, only part of it applies.** A warm return has no rewrite to avoid. The at-return term becomes a small cost (the summary plus the new base), and only the later-reads term remains.
- **This is the first Girth feature that changes what the model knows.** Lens, Meter and Keeper never did. It must ask first, or be opt-in, and needs an outcome A/B before any default changes.

## 4. Assumptions that need an A/B before shipping
- **The session after compaction might take more requests,** for example re-reading files the summary dropped. The model above assumes the same number of requests.
- **Answer quality after compaction is untested.** Returns after hours are often new subtasks, which favours compaction, but that is a hypothesis, not a measurement.
- **The 60K post-compaction base is an estimate:** 21.6K median summary plus about 40K of system prompt and tools.
- **No subagent rows appear inside the main transcripts** (`isSidechain` usage rows: 0), so the 6.2% subagent share is not an undercount.

## Mods API facts that make this buildable (code.claude.com/docs/en/plugins/mods/reference)
- **`session.compact` event:** fires when the conversation is about to be compacted; a hook can return `{ skip: reason }`.
- **`$.session.compact`** exists, and `$.session.usage()` returns `context.tokens`.
- **`session.append`:** a mod can rewrite a `tool_result`'s content before it is stored. New content can be trimmed this way without breaking the cache.
- **`agent.spawn`:** a mod can choose a subagent's model, with `next({ ...e, model })`.
- **`turn.step`:** a mod can change a request's model or effort.
- **`prompt.submit` with `$.ui.ask`:** a mod can ask before a prompt is sent. `ask` rejects in `claude -p`.

## 5. Revised after the spike: compact while idle, not on return
`examples/compact/SPIKE.md` showed that a mod cannot compact from `prompt.submit` (the host refuses) and cannot compact in headless sessions. It **can** compact an interactive conversation from a timer between turns.

That gives a better design: at 55 minutes idle, while the 1-hour cache is still warm, compact if the context is large. `examples/compact/analyze-idle-compaction.py`, on the same transcripts (Opus rates, 60K base, 20K summary output, matching the median 21.6K `postTokens`; periods that never returned pay the compaction for nothing):

| Context | Idle periods (returned) | Compactions | Return writes saved | Later reads saved | Net |
|---|---|---|---|---|---|
| ≥ 100K | 262 (243) | $131 | $727 | $1,234 | **$1,831** |
| ≥ 150K | 240 (222) | $121 | $717 | $1,219 | $1,815 |
| ≥ 250K | 201 (187) | $104 | $687 | $1,128 | $1,710 |
| ≥ 400K | 156 (148) | $83 | $634 | $1,009 | $1,560 |

- **About 39% of total spend, measured against no keeper.**
  - The keeper is already running and saves about $416 of the same rewrites, so the extra saving over today is roughly **$1.4K**.
- **The two collide at 55 minutes:** the keeper pings then, and idle compaction would compact then.
  - For a large context they are alternatives, not additions: the compaction replaces the ping.
  - Once the context is small, the keeper's rule (context ≥ 20K) decides as before.
- **Open before building:**
  - the opt-in shape (it changes what the model knows while the user is away);
  - an outcome A/B (does work after an idle compaction go as well as without it?);
  - interactive only.

## 6. What an outcome costs (2026-10-16)
Measured with `examples/outcomes/analyze-outcomes.py`. It pairs each Bash call with its result, classifies the command, and prices the requests between outcomes. Projects are anonymised as P1, P2, … by spend.

- **Commits:** 1,415 across 13 projects, **$3.33 per commit** overall.
  - Per project the cost ranges from $0.95 to $6.41, a 7× spread, so the number tells projects apart.
  - **Spend between commits:** p25 $0.64, median **$1.51**, p75 $3.37, p95 $11.43.
- **The exit status of a test run says almost nothing.** 2,066 of 2,086 test commands (99%) were piped through `tail`, `grep` or `head`, so the exit status was the filter's.
  - By exit status, 98% of runs passed.
  - **The runners' own summaries tell a different story.** Read from the end of the output (`test result: FAILED`, `3 failed`, `1 failure`, `ℹ fail 2`, …), there were 1,553 passing runs, **853 failing (35% of runs with a verdict)** and 548 with no verdict in the output.
  - Girth must therefore read the summary line, never the exit status.
- **Spend while the last test run had failed:** $351, or 7.5% of all spend. Request-level, so a lower bound on the cost of fix loops.
- **The classifier was checked against ground truth.** On this repo it found 117 commits in the transcripts; `git log` has 122 non-merge commits in the same period. The rest are merges, which `git merge` makes, not `git commit`.
- **Wrappers the classifier must strip:** `rtk` and `rtk proxy`, `/usr/bin/`, `timeout N`, `nice -n N`, `mise exec … --`, `uv run`, environment assignments, `cd … &&` chains and heredoc bodies.
  - Without the heredoc rule, a `python3 - <<EOF` script that mentions `pytest` counts as a test run.
- **Verdict:** cost per commit is informative and cheap to compute locally. Cost per passing test run is not a useful unit, because agents rerun tests constantly. Report pass/fail counts instead.
