# Spike: can a mod compact a conversation, and when? (2026-10-15, Claude Code 2.1.300, Haiku 5.5)

**Question:** compact-on-return (`docs/research/2026-10-15-where-the-money-goes.md`) assumed a mod could compact just before the returning prompt is sent. Can it?

## Results
| Where `$.session.compact()` was called | Outcome |
|---|---|
| A `prompt.submit` hook, interactive or `-p` | **Refused by the host:** "called from a prompt.submit hook, it would compact under the turn this hook is holding; call it from a later event (turn.complete)". The prompt still went through, because the hook fails open. |
| A `turn.complete` hook in `claude -p` | **Refused:** "not available in a headless (-p / SDK) session yet: compaction here runs inside a turn (a /compact prompt)". |
| A `$.clock.after` timer armed at `turn.complete`, interactive (tmux) | **Works.** 84,286 → 9,576 tokens (2 messages), recorded as `compact_boundary` with `trigger: manual`. |

- **After the timer compaction,** the model answered "HELIOTROPE 4127 — lens.rs shrinks a project's skill listing…".
  - It kept the codename and the number given three turns earlier, as `instructions` asked, and still knew the file it had read.
- **The first request after compaction read 0 and wrote 48,159 tokens:** system prompt, tools and summary. That is close to the 60K base the cost model assumed.
- **Lost Enter: seen once, not reproduced.**
  - In the first run, a prompt typed more than a minute after the compaction had finished stayed in the input box through two Enters. The prompt and Enter were sent in one `tmux send-keys` call.
  - The prompt went through on a later `C-m`, which Claude Code then followed with a second compaction: the spike re-arms its timer after every turn.
  - **Retest** (a fresh run of the same session; prompt sent with `send-keys -l`, then Enter 1 s later, 90 s after a 58,988 → 8,600 compaction): the prompt went through on the first Enter. The answer "HELIOTROPE 4127 PAPAYA" came 24 s later.
  - The first failure was probably how tmux delivered the keys, but that is not proven. Watch for it in the dogfood run.

## What this changes
- **"Compact on return" can't compact before the return.** Doing it after the first returning turn would still pay the full rewrite once.
- **Compacting while the conversation sits idle works, and it is cheaper.**
  - Done before the 1-hour cache expires, the compaction reads the warm cache.
  - The return then re-writes about 50–60K tokens instead of the whole context, so both the at-return saving and the later-reads saving apply.
- **Headless sessions (`claude -p`, SDK) can't use it yet.** The feature is interactive-only, and evals can't exercise it end to end.
