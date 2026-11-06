# Portability: do other agents load a Girth-crafted skill?

**Date:** 2026-10-23 · **Question:** the Agent Skills standard is now read by 70+ platforms. Is a skill Girth crafts discovered outside Claude Code?

**Setup:** a fresh folder containing two skills in `.agents/skills/`. Each agent was asked headless, read-only, to name every skill it has that starts with `girth-`.
- `girth-captions`, crafted live by the current template: flat, string-only `metadata` (`girth-sources`, `girth-crafted-for`, `girth-created`, `girth-last-used`).
- `girth-hyperframes`, crafted on 2026-10-14: the legacy nested `metadata.girth` block with lists.

| Agent | Version | Flat (spec) form | Legacy nested form | Notes |
|---|---|---|---|---|
| Codex CLI (`codex exec --sandbox read-only`) | 0.160.1 | **listed** | **listed** | Codex listed both in its skill inventory from `.agents/skills` and reported no load failure. It never read a body: Its shell couldn't run here: the host blocks bubblewrap (see `docs/eval-log.md`). |
| Gemini CLI | 0.61.0 | not tested | not tested | The account's tier no longer serves Gemini CLI ("migrate to Antigravity"). |
| Cursor agent | 2026.04.17 | not tested | not tested | Needs `agent login`. |

**Reading it:**
- On the one agent tested, both forms are discovered, so skills crafted before the change keep working.
- The flat form is the one the spec defines (`metadata`: string keys to string values). It is what new crafts write, and what a stricter reader will expect.
- The `## Record` step in a crafted skill calls Girth's `made` command, which only Claude Code with Girth has. Since 2026-10-23 the template says "If Girth is installed", so other agents skip it instead of failing.

## Every harness's folders and Codex spend (2026-11-06)

**Discovery.** Same machine, a folder with no `.girth/config.json`, `girth measure`:

| Build | Default roots | Skills found |
|---|---|---|
| v0.9.0 (installed plugin) | `~/.claude/skills`, `~/.agents/skills`, `.claude/skills`, plugin skill dirs | 124 |
| this change | the above, plus each harness folder that exists | **146** |

The 22 skills that were missing live only in other harnesses' folders. They are counted once even where `npx skills add` linked them from `~/.agents/skills`. The folders that exist here: `~/.codex/skills` (8 entries), `~/.gemini/skills` (11), `~/.cursor/skills` (8), `~/.copilot/skills` (8), `~/.config/opencode/skills` (14), `~/.grok/skills` (2) and `~/.config/goose/skills` (15).

**Codex spend.** `girth import` on the author's `~/.codex` (into a scratch copy of `~/.girth`):

```
$ girth import
imported 140 turns from codex
$ girth import
imported 0 turns from codex
$ girth advise --since 60d --harness codex
girth advise (60d): $173.00 estimated at list prices; shares overlap
  81%  turns 8h+ into a session
       fix: /new when you switch tasks: a fresh session re-reads only what the new task needs
  44%  large conversations
       fix: /compact <what to keep> at a natural break, or /new when you switch tasks
```

- An independent Python sum over all 4,544 `token_usage_record` lines, at the same OpenAI list prices, gives **$173.00**.
- The first build gave $170.14. The $2.87 gap was 4 turns that never logged an end event; they are now recorded as aborted once a later turn ends.
- Across both harnesses over 60 days, `advise --json` splits spend as `claude-code` $3,549 and `codex` $173.
