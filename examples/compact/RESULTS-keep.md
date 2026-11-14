# The compaction keep-list: A/B (2026-11-14)

`keep_ab.sh`: one interactive Claude Code 2.1.325 session per arm, in tmux, on `claude-haiku-5-5`.
- **The script:** 8 rules are stated in passing over 5 messages, then 3 source files (~60K tokens) are read, then `/compact`, then "list every rule I gave you".
- **The arms:** `girth` loads the 0.26 plugin, with the `session.compact` keep-list. `none` loads no plugin.

| arm | run 1 | run 2 | run 3 | total |
|---|---|---|---|---|
| girth | 8/8 | 8/8 | 7/8 | 23/24 |
| none | 8/8 | 8/8 | 7/8 | 23/24 |

- **Both run-3 misses were the same fact:** "the staging server is called kestrel-2".
- **No difference was shown.** Claude Code's own summary already kept short, clearly stated rules in a conversation of this size. The keep-list stays because it costs a few hundred characters of summarizer prompt. Whether it helps in longer conversations, with rules buried deeper, is untested.

# Where the rest goes: compaction at commits, or at a size cap? (2026-11-14)

Measured with `idle_sim.py` on the author's last 7 days: $1,825 in total, $937 of it main-loop cache reads.

| policy | compactions | net saving |
|---|---|---|
| idle 55 min (the 0.26 default) | 77 | $445 |
| + after any commit turn (≥ 200K, next turn ≥ 2 min away) | 95 | $527 |
| + between turns at a 400K cap | 81 | $473 |
| + between turns at a 200K cap | 100 | $543 |

- **Why the extra policies add so little:** $720 of the $937 in reads came in turns of 20 or more requests. The context grows *within* one long agentic turn, which no between-turn compaction can reach.
- **What's left for those turns:**
  - Claude Code's own mid-turn compaction, set with `CLAUDE_CODE_AUTO_COMPACT_WINDOW` or the `autoCompactWindow` setting. A mod can't set either (2.1.325 has no `/config` row for it).
  - Smaller tool output.
- **So 0.26 ships idle compaction alone.** Compacting at a commit or a cap would compact while the user is working, for $30–100 a week more.
