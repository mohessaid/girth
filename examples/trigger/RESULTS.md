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
- **The size of it** (`measure_reads.py`, both conditions, crafted `girth-*` files excluded): whole-file reads pulled 184 KB in 20 sessions, about **2.3K tokens per session, ≈ 7% of fresh input**. `girth show` pulled 65 KB.
- **Why:** the files read whole were HyperFrames' own entry skill and its references (`hyperframes/references/*` 8×, `hyperframes` 4×, `hyperframes-cli` 4×). That skill's description begins "Read first to make, edit, animate or render any video with HyperFrames", so the model is following the source skill's instruction, not ignoring Girth's.
- **Decision:** at 7% of one session, a once-per-domain cost, this doesn't justify changing `search`'s frozen JSON. It is recorded here and will be re-measured if a source pack makes it larger.

**Effect on published results:** `examples/compounding` ran its girth arm with `acceptEdits` and `Bash`. Crafts there succeeded by writing through Bash, which the harness allowed. Its numbers stand as measured. A grant would only make session-1 crafting more reliable.
