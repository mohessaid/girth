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
- The `## Record` step in a crafted skill calls Girth's `made` command, which only Claude Code has. Elsewhere the rest of the skill still applies.
