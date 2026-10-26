# Cut usage, keep things working

1. Run `scripts/girth advise` (`--since 24h` for today). Costs are list-price estimates; shares overlap.
2. Summarise the top three contributors in one line each: the share, what it is, and the fix.
3. Apply **one** fix at a time, and only the one the user picks. Then run its guard:
   - a skill: distil its slice (the craft flow), then `scripts/girth validate` and `scripts/girth drift`;
   - large conversations: suggest `/compact <what to keep>` at a natural break, or `/clear` on a task switch. Never compact or clear without asking;
   - cache rewrites or subagents: show the exact config line and the file it goes in. Let the user add it.
4. Never uninstall a skill, change settings or end a session yourself.
