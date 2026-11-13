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
1. **The workspace** is a fresh local clone of the project (`git clone --quiet <repo> <tmp>`) at `HEAD`.
   - Both arms start from the same files, and uncommitted work never leaks in.
   - Unlike `git archive`, a clone keeps the history, so a task that commits or diffs behaves as it does in the real repo.
2. **The with-skill arm** copies `<skill-dir>` into `<work>/.claude/skills/<name>/`.
3. **The without arm** removes any `.claude/skills/<name>/` the tree already has.
4. **The agent** runs in `<work>`:
   - The prompt goes on stdin.
   - `GIRTH_HOME` points at a temp dir, and `GIRTH_TRIAL=1` is set.
   - `--timeout` (default 1,800 s) kills a run that hangs. A killed run counts as a fail and is marked `timeout`.
5. **The check** runs in `<work>` with a 600 s timeout. Its exit status is the verdict. Its output is never stored.

**The agent:**
- **The default** is `claude -p --output-format stream-json --verbose --setting-sources project,local --permission-mode acceptEdits --allowedTools Bash,Skill`, plus `--model M` when given.
  - User-level skills, plugins and MCP servers are hidden, so the skill is the only difference between the arms.
  - `Bash` is allowed because, without it, a headless run can't do most tasks and both arms would fail. The exact command is smoke-tested on a trivial task before release.
  - From the stream, Girth reads two things:
    - **Whether the skill fired:** a `Skill` tool use naming it.
    - **The run's cost:** the result line's `total_cost_usd`.
- **`--agent`** replaces the default with any command that reads a prompt on stdin, such as Codex's `codex exec -` or `gemini -p`. Girth targets every harness, so the runner is not tied to one. For those agents, firing and cost are reported as unknown.

**Order and repeats:**
- Arms alternate (with, without, with, …), so drift in the model or the network hits both arms alike.
- Runs are sequential, never in parallel.
- `--runs` defaults to 3 for each arm and task, chosen from the pilot below.
- `--runs` must be at least 2. With one run, no task could reach the 2-run difference below.

### The verdict

`verdict(tasks: &[TaskResult], fired: Option<u64>) -> Verdict` is a pure function.
- Each `TaskResult` holds `with_passed`, `without_passed` and `runs`, all integers.
- `fired` is how many with-skill runs used the skill, when Girth could tell.

**Each task is judged on integer passes,** `d = with_passed − without_passed`:
- **won** when d ≥ 2;
- **lost** when d ≤ −2;
- **tied** otherwise.

A single flipped run never decides a task. That is the pilot's lesson: the paper's "any positive gain" rule, mirrored, would have rejected a skill on one lucky no-skill run.

| Condition | Verdict |
|---|---|
| `fired == Some(0)`: the skill never ran | **undecided** (the arms were the same) |
| at least one task won, none lost | **admit** |
| at least one task lost, none won | **reject** |
| none won or lost, or some of each | **undecided** |

**Known limits, stated in `--help`:**
- A skill that adds one pass on every task (+1, +1, +1, +1) is undecided. That is conservative on purpose; more runs settle it.
- When every run of both arms fails, the task can't tell the arms apart, and the output says so.

**Cost is reported, never decisive.**
- In the pilot, output tokens for the same arm and task varied by 17–32% between runs. A cost threshold at n = 3 would decide on noise.
- Each arm's mean cost per run is printed next to the counts, and stored. 2611.12953's point that added cost must be paid for by a gain is left to the reader, with the numbers in front of them.

**Reporting:**
- Each task prints as raw counts: `edit-pdf: with 3/3 · without 0/3 → won`.
- Then a total: `skill used in 3 of 3 runs · $0.41 vs $0.38 per run → admit`.
- There is no significance language. A footer says once that a task needs a 2-run difference to count.

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
- **`edit-pdf`:** a clear win. The same-day difference is 2 runs, so the task is won.
- **`software-dependency-audit`:** noise in both arms.
  - Every run, with or without the skill, either listed the 3 production findings (pass) or 22 findings with dev dependencies included (fail).
  - The first run's "the skill turned a pass into a fail" was this coin flip.
  - The paper's "any positive gain" rule would have admitted on today's runs and rejected on October's. Needing a 2-run difference, the task is tied both times.
- **The skill fired** (a `Skill` call naming it) in every girth run. The trajectories show it; BenchFlow's own counter reads 0. So firing has to be read from the agent's stream, not assumed.
- **Cost noise:** output tokens for the same arm and task varied by 17–35% between runs.
- **The default stays at 3 runs** per arm and task, with a task decided only by a 2-run difference. A user who wants a sharper verdict raises `--runs`.
