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
