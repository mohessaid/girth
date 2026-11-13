# Girth: skill trials (does a crafted skill earn its place?)

Status: design, 2026-11-13, pending the author's review.

Evidence:
- `docs/radar/2026-11-13.md`: papers 2611.13144 and 2611.12953
- `examples/skillsbench/RESULTS.md`, including the 2026-11-13 pilot of the no-skill arm

## Why

A crafted skill passes `girth validate` when it meets the contract. That says nothing about whether it helps:

- **Skills can hurt.** In 2611.12953, high-quality skills lifted SOP success from 67.4% to 73.1%. Degraded skills fell *below* no skill, and skill-augmented arms spent 1.4–4.7× the tokens.
- **Girth's own run shows the need, and the noise.**
  - In SkillsBench, Girth's `edit-pdf` skill passed 3/3 where no skill failed 3/3.
  - `software-dependency-audit` once looked like a skill that hurts: no skill passed, the skill failed. The pilot below shows that was one lucky no-skill run (1/3 against 0/3).
  - A gate has to tell those two apart.
- **One remedy is published: an admission test.** 2611.13144 (Student-Aware Teaching) admits a skill only if it raises the pass rate on matched dev tasks:
  - It runs 1–4 tasks, once with the candidate and once with an empty library.
  - It admits when the mean gain is positive.
  - Under misleading learner profiles, the test rejected all 48 candidates.
  - The paper does not isolate the gate's own lift. A teacher with the same gate but no learner model reached 81.0%, against 75.0% untaught. Girth cites the gate as a filter, not as a measured gain.

So Girth gets a way to put a skill on trial: the same task, with and without it, graded by a command, with the verdict recorded.

## What

### `girth trial`

```
girth trial <skill-dir> --task <file> --check <cmd> [--task … --check …] [--runs N] [--agent <cmd>] [--model M]
```

**What it needs:**
- `<skill-dir>` is a skill folder with a `SKILL.md`, usually a crafted `girth-*` skill.
- Each `--task` is a prompt file.
- Each `--check` is a shell command that grades a run: exit 0 means pass.
- Tasks and checks pair up in order. A single `--check` may serve every task.

**Each run:**
1. **The workspace** is a fresh local clone of the project (`git clone --quiet`) at `HEAD`, under `$GIRTH_HOME/trials/<pid>-<nanos>/`. A later trial sweeps the dirs of runs whose pid `ps -p` can't find.
   - Both arms start from the same files, and uncommitted work never leaks in.
   - Unlike `git archive`, a clone keeps the history, so a task that commits or diffs behaves as it does in the real repo.
2. **The with-skill arm** copies `<skill-dir>` into `<work>/.claude/skills/<name>/`.
3. **The without arm** removes any `.claude/skills/<name>/` the tree already has.
4. **The agent** runs in `<work>`:
   - The prompt goes on stdin.
   - `GIRTH_HOME` points inside the scratch dir, and `GIRTH_TRIAL=1` is set.
   - The agent stays in girth's process group, so Ctrl-C stops it too.
   - A non-zero exit or an `is_error` result is an agent error. The run fails, and the first stderr line is printed.
   - `--timeout` (default 1,800 s) kills a run that hangs. A killed run counts as a fail and is marked `timeout`.
5. **The check** runs in `<work>` with a `--check-timeout` (default 600 s); a timed-out check fails the run and is counted. Its exit status is the verdict. Its output is never stored.

**The agent:**
- **The default** is `claude -p --output-format stream-json --verbose --setting-sources project,local --permission-mode acceptEdits --allowedTools Bash,Skill`, plus `--model M` when given.
  - User-level skills, plugins and MCP servers are hidden, so the skill is the only difference between the arms.
  - `Bash` is allowed because, without it, a headless run can't do most tasks and both arms would fail. The exact command is smoke-tested on a trivial task before release.
  - From the stream, Girth reads two things:
    - **Whether the skill fired:** a `Skill` tool use naming it.
    - **The run's cost:** the result line's `total_cost_usd`.
- **`--agent`** replaces the default with any command that reads a prompt on stdin. Girth targets every harness, so the runner is not tied to one.
  - The skill goes into both `.claude/skills/<name>` and `.agents/skills/<name>`, and both are cleared in the without arm.
  - For these agents, firing and cost are reported as unknown, and the output says the verdict assumes the skill ran.
  - **Verified 2026-11-13** with Codex 0.164 (`codex exec --skip-git-repo-check -s workspace-write -`): the skill was found in `.agents/skills`, 3/3 vs 0/3.
- **`GIRTH_TRIAL_AGENT`** replaces the default agent. The tests set it, and run with `claude` off the PATH, so no test or mutant can start a real, auto-approved run. Before this, a mutation-testing run started three real `claude -p` runs, about $0.38.

**Safety:** the default agent pre-approves Bash. A trial lets the skill's instructions run shell commands with the user's environment, so `--help` and `references/trial.md` say to trial only skills the user would let run.

**Order and repeats:**
- Arms alternate (with, without, with, …), so drift in the model or the network hits both arms alike.
- Runs are sequential, never in parallel.
- `--runs` defaults to 3 for each arm and task, chosen from the pilot below.
- `--runs` is 2–10. At 2 runs no task can be decided (the best gap, 2/2 vs 0/2, has a 1-in-6 chance). It is allowed only so that a cheap first look can show whether the skill fires at all.

### The verdict

`verdict(tasks: &[TaskResult], fired: Option<u64>) -> Verdict` is a pure function.
- Each `TaskResult` holds `with_passed`, `without_passed` and `runs`, all integers.
- `fired` is how many with-skill runs used the skill, when Girth could tell.

**Each task is judged with Fisher's one-sided exact test** on its pass counts:
- **won** when the chance that identical arms would show a gap at least this large in the skill's favour is at most 0.05;
- **lost** when the same holds against the skill;
- **tied** otherwise.

**Why not a fixed gap.** An earlier draft used "won when the difference is at least 2 passes". A simulation (2026-11-13, 20,000 trials per cell) shows that rule calls a win or a loss on identical coin-flip arms 21% of the time at 3 runs, and 50% at 10. So more runs would make it *worse*.

| runs per arm | fixed 2-pass rule: false calls (p = 0.5) | Fisher at 0.05: false calls (p = 0.5) | Fisher's power, 0.9 vs 0.2 |
|---|---|---|---|
| 3 | 21% | 3.0% | 36% |
| 5 | 34% | 2.2% | 55% |
| 10 | 50% | 4.2% | 94% |

Worked exactly over every pass rate (`examples/trial/false_calls.py`), the chance that identical arms get any call stays at or under 4.22% up to 10 runs. It passes 5% at 11 runs and reaches 7.9% at 47, so `--runs` stops at 10. Within that range, power grows with runs. "Add runs when undecided" is therefore the right advice.
- At 3 runs, only 3/3 vs 0/3 decides.
- With several tasks, the chance that some task gets a false call grows with their number.

The pilot's lesson still holds: one flipped run never decides.

| Condition | Verdict |
|---|---|
| `fired == Some(0)`: the skill never ran | **undecided** (the arms were the same) |
| at least one task won, none lost | **admit** |
| at least one task lost, none won | **reject** |
| none won or lost, or some of each | **undecided** |

**Known limits, stated in `--help`:**
- A small lift is undecided at 3 runs. That is conservative on purpose, and more runs settle it.
- When every run of both arms fails, the task can't tell the arms apart, and the output says so.

**Cost is reported, never decisive.**
- In the pilot, output tokens for the same arm and task varied by 17–32% between runs. A cost threshold at n = 3 would decide on noise.
- Each arm's mean cost per run is printed next to the counts, and stored. 2611.12953's point that added cost must be paid for by a gain is left to the reader, with the numbers in front of them.

**Reporting:**
- Each task prints as raw counts: `edit-pdf: with 3/3 · without 0/3 → won`.
- Then a total: `skill used in 3 of 3 runs · $0.41 vs $0.38 per run → admit`.
- A footer says once what a call means: chance would give the gap at most 1 time in 20.

### The ledger

A new event, `trial`, holding:
- `skill` and `project`;
- `tasks`: one `{with_passed, without_passed, runs}` per task, with no names;
- `fired?`;
- `with_cost?` and `without_cost?`, the mean dollars per run;
- `verdict`.

It stores no prompt text, check command or output, just as outcomes store only labels. `share` never sends it; a sentinel test enforces that. Older Girth versions drop the unknown kind.

### Where the verdict shows

- **`girth stats`**: each crafted skill's row gains `trial: admit (3/3 vs 1/3, 2026-11-13)`, from that skill's latest trial in the project.
- **`girth advise`:** deferred. Its contributors are shares of spend, worked out from the ledger without reading the filesystem. "A rejected skill is still installed" fits neither, and belongs in a later skill-hygiene pass.

### Advisory, not a gate

- A trial costs real runs: minutes, plus dollars or plan usage. Crafting is just-in-time, so the trial never sits in the craft path.
- `girth validate` doesn't change. `VALID` and `INVALID` stay about the contract, and a skill with no trial, or a rejected one, is still valid.
- The shipped `SKILL.md` stays short. The trial is described in `girth trial --help` and in `skills/girth/references/`, and SKILL.md gets at most one line pointing there.

## Non-goals

- **A ledger-only verdict,** comparing turns with and without the skill. Skills fire on harder work, so that comparison is confounded.
- **The source skills as the baseline** ("is the distillation faithful?"). The verdict question is "does it help?", so the baseline is no skill. Faithfulness stays an example-level experiment (`examples/skillsbench`).
- **Statistics:** confidence intervals and p-values. The sample sizes are too small for them.
- **Running trials automatically.**

## The pilot (2026-11-13)

**Setup:** the same as `examples/skillsbench`: SkillsBench `f860ecf`, `claude-sonnet-5-5`, local Docker.
- On 2026-11-13, both arms ran twice on each task, alternating.
- The earlier runs are from 10-15 (both arms) and 10-23 (girth only).

| task | no skill, same day | girth, same day | no skill, all | girth, all |
|---|---|---|---|---|
| edit-pdf | 0/2 | 2/2 | 0/3 | 5/5 |
| software-dependency-audit | 0/2 | 1/2 | 1/3 | 1/5 |

**What it means for the rule:**
- **`edit-pdf`:** a clear win over all runs, 5/5 against 0/3. The same-day 2/2 vs 0/2 alone would be tied under Fisher's test (1 in 6), and the default of 3 runs exists for that reason.
- **`software-dependency-audit`:** noise in both arms.
  - The three same-day audit runs whose output was inspected (both no-skill runs and the failed skill run) listed 22 findings with dev dependencies included, and failed. The passing skill run listed the 3 production findings. The raw runs have since been deleted.
  - The first run's "the skill turned a pass into a fail" was this coin flip.
  - The paper's "any positive gain" rule would have admitted on today's runs and rejected on October's. Under Fisher's exact test, the task is tied both times.
- **The skill fired** (a `Skill` call naming it) in every girth run. The trajectories show it; BenchFlow's own counter reads 0. So firing has to be read from the agent's stream, not assumed.
- **Cost noise:** output tokens for the same arm and task varied by 17–35% between runs.
- **The default stays at 3 runs** per arm and task, judged with Fisher's exact test. A user who wants a sharper verdict raises `--runs`.
