# M2: 5-session compounding benchmark

**Date:** 2026-10-15 · **Model:** claude-sonnet-5-5 · **Samples:** 1 run per arm per session. These are single samples, so read the per-session numbers as indicative. The totals agree in direction across three full runs (below).

## Question
Over five sessions in the same domain, does Girth's persistent cache spend less than both:
- **bulk:** installing every skill, as the harness does today;
- **per-session JIT:** loading or distilling skills fresh every session, Skillselion-style?

## Setup (`run.sh`)
- **Tasks:** five different short HyperFrames compositions in one project (`tasks/1.md` … `tasks/5.md`): a title card, caption highlighting, a lower third, a stat count-up and an end card. Every task ends with "run its check command and fix any errors".
- **Every arm:**
  - runs `claude -p --setting-sources project,local --plugin-dir <girth dist>`, so user-level skills, plugins and settings are hidden;
  - gets read access to `~/.claude/skills` (`--add-dir`), as an interactive user would grant;
  - uses an isolated `GIRTH_HOME`.
- **The tasks never mention Girth.** Crafting happens only if Girth triggers itself.

| Arm | Project skills |
|---|---|
| bulk | symlinks to all 40 skills in `~/.claude/skills` |
| jit | none; Girth is available, and crafted skills are **wiped before every session** |
| girth | none at first; Girth is available, and crafted skills **persist** |

## Results (final run, `usage/`)

| Arm | s1 | s2 | s3 | s4 | s5 | **Total** | Fresh input tok | Cache-read tok |
|---|---|---|---|---|---|---|---|---|
| bulk | $0.178 | $0.186 | $0.172 | $0.209 | $0.164 | **$0.909** | 170,201 | 1,035,428 |
| jit | $0.251 | $0.185 | $0.184 | $0.224 | $0.215 | **$1.061** | 175,454 | 1,371,060 |
| **girth** | $0.222 | **$0.101** | **$0.090** | **$0.089** | **$0.098** | **$0.599** | **103,933** | **644,111** |

- **Girth versus bulk:** −34% cost, −39% fresh input tokens, −38% cache reads.
- **Girth versus per-session JIT:** −44% cost.
- **Compounding:**
  - Session 1 pays to craft `girth-hyperframes` ($0.22, slightly more than bulk).
  - Sessions 2–5 hit the cache and cost **about half** of a bulk session ($0.09–0.10 vs $0.16–0.21), in 5–6 turns instead of 6–9.
  - The JIT arm re-crafts every session and never gets that discount.
- **Quality:** all 15 sessions report a passing HyperFrames check (lint, runtime, layout, motion, contrast). The only warning in the run was in one JIT session. On this domain and check, Girth's savings come **at equal measured quality**.

### Earlier runs (same tasks)
| Run | bulk | jit | girth | Note |
|---|---|---|---|---|
| 1 | $1.092 | $1.192 | $0.823 | **Invalid for Girth:** headless `Read` outside the project was denied, so Girth never crafted. The arm ran with no skills at all. This led to `girth show` and to `--add-dir` in the harness. |
| 2 | $1.081 | $1.001 | $0.670 | Valid, but crafting happened in s2: before the `show` overview was labelled, s1 read the heading list as the whole skill. |
| 3 (final) | $0.909 | $1.061 | **$0.599** | — |

## What the benchmark found in Girth itself
1. **Permission friction:** source skills live outside the project. Crafting by reading whole files needs permission for each read, and headless runs simply deny them. → `girth show <dir> --heading …` prints only the needed sections through the one launcher command.
2. **An ambiguous overview:** an unlabelled heading list was read as "this skill is a stub". → The overview now says `headings only; print sections with: …`.

## Reproduce
```bash
make dist
examples/compounding/run.sh bulk  /tmp/cmp
examples/compounding/run.sh jit   /tmp/cmp
examples/compounding/run.sh girth /tmp/cmp
python3 examples/compounding/summarize.py --table /tmp/cmp
```
Each arm costs about $1 at current prices.
