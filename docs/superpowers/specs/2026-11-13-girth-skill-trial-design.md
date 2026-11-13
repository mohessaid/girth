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
1. The workspace is the project's committed tree (`git archive HEAD`), unpacked into a fresh temp directory. Both arms therefore start from the same files, and uncommitted work never leaks in.
2. **The with-skill arm** copies `<skill-dir>` into `<work>/.claude/skills/<name>/`.
3. **The without arm** removes any `.claude/skills/<name>/` the tree already has.
4. The agent runs in `<work>` with the prompt on stdin, `GIRTH_HOME` pointed at a temp dir, and `GIRTH_TRIAL=1`.
5. The check runs in `<work>` with a 600 s timeout. Its exit status is the verdict. Its output is never stored.
6. If the agent printed Claude Code's `--output-format json` result, the run's cost comes from its `total_cost_usd`. Otherwise the cost is unknown.

**The agent:**
- The default is `claude -p --output-format json --setting-sources project,local --permission-mode acceptEdits`, plus `--model M` when given. User-level skills, plugins and MCP servers are hidden, so the skill is the only difference between the arms.
- `--agent` replaces the default with any command that reads a prompt on stdin, such as Codex's `codex exec -` or `gemini -p`. Girth targets every harness, so the runner is not tied to one.

**Order and repeats:**
- Arms alternate (with, without, with, …), so drift in the model or the network hits both arms alike.
- Runs are sequential, never in parallel.
- `--runs` defaults to 3 for each arm and task. That is chosen from the pilot below.
- `--runs` must be at least 2. With one run, the margin is the whole range, so no verdict could ever be reached.

### The verdict

`verdict(with, without) -> Verdict` is a pure function. `with` and `without` each hold, per arm, `passed`, `runs` and an optional `cost`.

The **gain** is the mean, over tasks, of `pass_rate(with) − pass_rate(without)`.

The **margin** is one run's worth, `1 / runs`. Unlike 2611.13144, which admits any positive gain, Girth needs the gain to clear this margin. A single flipped run must not decide a verdict (see the pilot).

| Condition | Verdict |
|---|---|
| gain > margin | **admit** |
| gain < −margin | **reject** |
| \|gain\| ≤ margin, every run of both arms passed, and with-skill cost ≤ 0.9 × without | **admit**, as good and cheaper |
| \|gain\| ≤ margin, every run of both arms passed, and with-skill cost ≥ 1.25 × without | **reject**, as good and dearer |
| anything else (both failing, or costs within the band, or unknown) | **undecided** |

**Why the cost rows:**
- 2611.12953 found that the added cost has to be paid for by a gain.
- Girth's own SkillsBench result was equal success with 14–23% fewer output tokens. That is a reason to keep a skill.
- The band is asymmetric on purpose. Admitting needs a 10% saving. Rejecting needs a 25% overspend, because run-to-run spend varies.

**When every run of both arms fails,** the task can't tell the arms apart, so the verdict is undecided and the output says so.

**Reporting:**
- Results print as raw counts: `with 3/3 · without 1/3 · gain +0.67 · $0.41 vs $0.38 per run → admit`.
- There is no significance language. The footer says once that a gain within one run's worth is treated as no difference.

### The ledger

A new event, `trial`, holding:
- `skill` and `project`;
- `tasks`, a count only;
- `runs`;
- `with` and `without`, each `{passed, runs, cost?}`;
- `verdict`.

It stores no prompt text, check command or output, just as outcomes store only labels. `share` never sends it; a sentinel test enforces that. Older Girth versions drop the unknown kind.

### Where the verdict shows

- **`girth stats`**: each crafted skill's row gains `trial: admit (3/3 vs 1/3, 2026-11-13)`, from that skill's latest trial in the project.
- **`girth advise`** gains a line for a skill whose latest trial was **reject** and which is still installed in the project: "trial says it hurts; remove it or re-craft it".

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

**What ran:** the SkillsBench no-skill arm, two more runs each of `edit-pdf` and `software-dependency-audit`. The setup was the same as `examples/skillsbench`: SkillsBench `f860ecf`, `claude-sonnet-5-5`, local Docker.

| task | no skill (10-15, 11-13, 11-13) | girth skill (10-15, 10-23, 10-23) |
|---|---|---|
| edit-pdf | 0/3 | 3/3 |
| software-dependency-audit | 1/3 (pass, fail, fail) | 0/3 |

**What it means for the rule:**
- **`edit-pdf`:** the gain is +1.0, an admit under any rule.
- **`software-dependency-audit`:**
  - The gain is −0.33, a single flipped run.
  - Both of the new no-skill fails were close misses. The agent wrote 22 findings, dev dependencies included, and the grader expected only the production ones.
  - The paper's "positive gain admits" rule, mirrored, would reject this skill on noise. With the one-run margin, it is undecided, which is the honest answer.
- **The default stays at 3 runs** per arm and task, with the margin. A user who wants a sharper verdict raises `--runs`, and the margin shrinks with it.
