# `girth trial`: evidence

## The rule (`false_calls.py`)

The question: when a skill does nothing, how often does a trial call a task won or lost?
- Both arms pass each run with the same probability p.
- A task is called when Fisher's one-sided exact test, at 0.05, finds the gap in either direction.
- The worst case is taken over p from 0.01 to 0.99, computed exactly.

| runs per arm | worst-case false-call rate |
|---|---|
| 2 | 0% (nothing can be decided) |
| 3 | 3.1% |
| 5 | 2.2% |
| 10 | 4.2% |
| 11 | 5.3% |
| 47 | 7.9% (the largest from 2 to 50) |

So `--runs` stops at 10.

**The fixed rule it replaced.** 0.25.0 called a task won when the with-skill arm had at least 2 more passes. In a 20,000-trial simulation with p = 0.5, that rule made a false call 21% of the time at 3 runs and 50% at 10.

## Real trials (2026-11-13, `greeting.sh`)

**The setup:** a one-commit repo, and a skill that alone knows the greeting word, so a working skill should win.
- **Task:** "Create the project greeting file."
- **Check:** `grep -qx zebracorn greet.txt`.

| agent | runs per arm | with | without | skill used | cost per run (with / without) | verdict | time |
|---|---|---|---|---|---|---|---|
| default (Claude Code, `--model claude-haiku-5-5`) | 3 | 3/3 | 0/3 | 3 of 3 | $0.0044 / $0.0050 | admit | 72 s |
| `codex exec --skip-git-repo-check -s workspace-write -` (Codex 0.164) | 3 | 3/3 | 0/3 | unknown | unknown | admit | — |

- **Codex** found the skill in `.agents/skills`.
- **A first Codex attempt** used a flag Codex 0.164 doesn't have (`--full-auto`). Every run errored, and the report said so: `4 agent runs ended in an error (counted as fails): error: unexpected argument '--full-auto' found`, followed by "every run failed in both arms".

## Dogfood: this repo's `girth-skill-authoring` (2026-11-14)

**Task:** create `.claude/skills/girth-sql-migrations/SKILL.md` (`dogfood/authoring-task.md`).
**Check:** `dogfood/authoring_check.py`. The name must match; the description must start "Use when", stay under 500 characters and not summarise the workflow; the body must stay under 500 words.
**Model:** `claude-sonnet-5-5`, 3 runs per arm.

| agent mode | with | without | skill used | cost per run (with / without) | verdict |
|---|---|---|---|---|---|
| 0.26.0 default (`acceptEdits`, Bash and Skill allowed) | 0/3 | 0/3 | 3 of 3 | $0.111 / $0.110 | undecided: every run failed |
| 0.26.1 default (`bypassPermissions` in the clone) | 3/3 | 3/3 | 3 of 3 | $0.108 / $0.115 | undecided |

- **Why the 0.26.0 runs failed:** Claude Code asked before writing under `.claude/`. Under `acceptEdits` it asks even with `Write` allowed, and even with `Write(.claude/**)`, and nobody is there to answer in a headless run.
  - The trial's "every run failed in both arms" line flagged it.
  - Reproducing one run by hand showed the denial.
  - 0.26.1 reports such runs directly.
- **What the verdict means:** Sonnet 5.5 already writes descriptions that start "Use when", stay short and don't summarise the workflow. On this check the skill adds nothing measurable. A harder check, or an older or smaller model, might show a gap.
