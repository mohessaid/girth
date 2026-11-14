# Cut usage, keep things working

1. Run `scripts/girth advise` (`--since 24h` for today). Costs are list-price estimates; shares overlap.
   Outside Claude Code, run `scripts/girth import` first, then `scripts/girth advise --harness codex` (or `copilot`, `opencode`, `gemini`, `antigravity`). The fixes then name that harness's commands. For Copilot, the dollars are list prices; its bill is the premium requests the report names.
2. Summarise the top three contributors in one line each: the share, what it is, and the fix.
3. Apply **one** fix at a time, and only the one the user picks. Then run its guard:
   - a skill: distil its slice (the craft flow), then `scripts/girth validate` and `scripts/girth drift`;
   - large conversations or turns 8h+ into a session: Girth already compacts idle ones. Suggest `/compact <what to keep>` at a natural break, or `/clear` (Codex: `/new`) on a task switch. Ask before you compact or clear;
   - large tool output: relay the fix it prints. That's `| tail -n 40` or `| head` for commands (in Claude Code, also a shell-output filter), reading large files by line range, or fetching big pages in a subagent. The tail keeps the summary and errors; check that an error still shows before trimming a command for good;
   - cache rewrites, subagents or progress summaries (Claude Code only; these levers are Girth's Claude Code mod): show the exact config line and the file it goes in. Let the user add it.
4. Never uninstall a skill, change settings or end a session yourself.
