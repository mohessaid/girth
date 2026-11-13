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
