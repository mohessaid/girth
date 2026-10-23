# Trigger: does Girth fire, and carry through, by itself?

**Date:** 2026-10-23 · **Model:** claude-sonnet-5-5 · **Runs:** 10 per condition (the five compounding tasks × 2), each the **first session of a fresh project**.

The tasks never mention Girth. Each run has no project skills, Girth available through the dist plugin, user settings hidden, an isolated `GIRTH_HOME`, and read access to `~/.claude/skills` (`run.sh`). Per-run summaries are in `summary-*/`, extracted by `summarize.py`. The raw transcripts carry third-party skill text, so they aren't committed.

| Condition | Fired | Crafted a skill | Write denied | Read whole source files | Used `girth show` | Cost |
|---|---|---|---|---|---|---|
| `--permission-mode acceptEdits` | 9/10 | 3/10 | 2/10 | 8/10 | 1/10 | $2.04 |
| `--permission-mode bypassPermissions` | **10/10** | **9/10** | 0/10 | 10/10 | 2/10 | $2.16 |

**Finding 1: Claude Code's protection of `.claude/`, not Girth's instructions, held back the headless crafts.**
- Claude Code treats `.claude/` as protected. In headless mode only `--dangerously-skip-permissions` (bypass) writes there; `--allowedTools Write`, `Write(.claude/skills/**)` and `Write(./.claude/skills/**)` were all denied in a direct probe.
- In the first condition:
  - Two runs were refused outright.
  - Four fired, then gave up on crafting and did the task from the sources.
  - One didn't fire.
- With the grant, 9 of 10 crafted. Interactive users just see a permission prompt for the new skill. Girth doesn't route around that protection.
- **For CI or other headless use, craft with bypass in a throwaway checkout, or commit the crafted skills and run without crafting.**

**Finding 2, open: models read whole source files instead of sections.**
- Even when they crafted, models `cat`-ed whole installed skill files in 18 of 20 runs. They used `girth show --heading` (which prints only the matched sections) in 3.
- The cost lands once, in the crafting session, but it is the part of Girth's own flow that isn't lean yet.
- The likely cause is that `girth search --json` hands back file paths the model then opens directly. That's a design question for the search output, not a wording fix, and it's left for the next iteration.

**Effect on published results:** `examples/compounding` ran its girth arm with `acceptEdits` and `Bash`. Crafts there succeeded by writing through Bash, which the harness allowed. Its numbers stand as measured. A grant would only make session-1 crafting more reliable.
