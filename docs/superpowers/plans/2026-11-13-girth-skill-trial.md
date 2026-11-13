# Skill trial implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** `girth trial` runs a task with and without a skill, grades both with a check command, and records an admit, reject or undecided verdict that `stats` and `advise` show.

**Architecture:**
- A new module `src/trial.rs` holds the pure verdict, the stream reader and the runner, which uses `std::process`, polls with a timeout, and runs in a temp clone.
- A new ledger event, `Event::Trial(Trial)`.
- The CLI subcommand lives in `main.rs`.
- `stats` adds the trial to each crafted row; `advise` gains a contributor.

**Tech stack:** Rust (std only; serde and clap are already dependencies).

**Spec:** `docs/superpowers/specs/2026-11-13-girth-skill-trial-design.md`

## Global constraints

> Superseded in part by 0.25.1: tasks are judged with Fisher's one-sided exact test at 0.05 (not d ≥ 2), `--runs` is 2–10, the check timeout is `--check-timeout`, and scratch lives in `$GIRTH_HOME/trials`. See the spec.

- `--runs` must be at least 2 (default 3). `--timeout` defaults to 1,800 s, and the check timeout is 600 s.
- A task is **won** if d ≥ 2 and **lost** if d ≤ −2, where d = with_passed − without_passed.
  - **admit:** at least one task won and none lost.
  - **reject:** at least one task lost and none won.
  - **undecided:** everything else, and always when `fired == Some(0)`.
- Store no prompt, check or output text, and no task names.
- The default agent, verbatim: `claude -p --output-format stream-json --verbose --setting-sources project,local --permission-mode acceptEdits --allowedTools Bash,Skill`, plus ` --model M` when given.
- `girth validate` is unchanged.

## Review focus
1. **A skill whose name is plugin-namespaced** (`plug:name`) in the `Skill` input still counts as fired.
2. **An agent that prints non-JSON** gives `fired`/cost as unknown and never panics.
3. **An agent that hangs** is killed at the timeout and counted as a fail.
4. **The project is not a git repo, or `HEAD` doesn't exist:** a clear error and exit 2.
5. **Task/check counts differ:** one check serves all tasks, otherwise the counts must match (exit 2).

---

### Task 1: the verdict and the stream reader (pure)

**Files:** create `src/trial.rs` and add `pub mod trial;` to `src/lib.rs`.

**Produces:**
- `TaskResult { with_passed, without_passed, runs: u64 }`, with Serialize and Deserialize.
- `Call { Won, Lost, Tied }`
- `Verdict { Admit, Reject, Undecided }`, serialized snake_case.
- `judge(&TaskResult) -> Call`
- `verdict(&[TaskResult], Option<u64>) -> Verdict`
- `Stream { fired: Option<bool>, cost: Option<f64> }`
- `read_stream(stdout: &str, skill: &str) -> Stream`

**Tests:**
- `judge`: 3/0 is won, 1/3 is tied, 0/2 is lost, 2/1 is tied.
- `verdict`:
  - one won plus three tied → admit;
  - won plus lost → undecided;
  - all tied → undecided;
  - lost only → reject;
  - won with fired `Some(0)` → undecided;
  - won with `None` → admit.
- `read_stream`:
  - the smoke-test stream (a `Skill` with `{"skill":"greeting-file"}`, then a result with `total_cost_usd`) → `(Some(true), Some(cost))`;
  - `plug:greeting-file` counts as fired;
  - another skill's name → `Some(false)`;
  - a single `--output-format json` object → cost only, with `fired` `Some(false)`;
  - plain text → `(None, None)`.

### Task 2: the ledger event

**Files:** `src/ledger.rs`, plus the `Event::Trial` match arms in `src/share.rs`, `src/stats.rs` and `src/main.rs` (`only_harness`: true for every harness).

**Produces:** `Trial { project, skill, tasks: Vec<TaskResult>, fired: Option<u64>, with_cost: Option<f64>, without_cost: Option<f64>, verdict: Verdict }`.

**Tests:**
- It round-trips through JSON.
- The share proptest's sentinel run includes a `Trial` record, and the payload stays free of names.

### Task 3: the runner and the CLI

**Files:** `src/trial.rs` (`run`) and `src/main.rs` (`Command::Trial`), with tests in `tests/cli.rs`.

**Produces:**
- `Options { skill_dir, tasks: Vec<(String prompt, String check)>, runs, agent: String, timeout: Duration }`
- `run(project: &Path, opts: &Options, mut log: impl FnMut(&str)) -> Result<Trial, String>`

**How it runs:**
- Arms alternate within each task.
- Each run clones the project with `git clone --quiet`, places or removes the skill, then runs `sh -c agent` with the prompt on stdin and stdout captured. It uses the same timeout poll for the check (`sh -c check`).
- The skill name is the folder name.
- Cost is the mean over the runs that reported one.

**CLI:**
- `girth trial <SKILL_DIR> --task F... --check C... [--runs N] [--agent CMD] [--model M] [--timeout S]`
- It prints one line per task, then the total and the footer, and appends the event with project = the repo top level.

**Tests** use a fake agent: a shell script that writes `ok` only if `.claude/skills/<name>` exists, and prints a stream-json `Skill` line plus a result line.
- With check `test -f ok` → admit, `fired` 3, and the ledger has a trial.
- An agent that writes `ok` only *without* the skill → reject.
- A hanging agent with `--timeout 1` → both arms fail, undecided.
- Not a git repo → exit 2.
- Two tasks and three checks → exit 2.

### Task 4: stats, advise, docs, release

**Stats:**
- `CraftedRow.trial: Option<TrialRow { verdict, with_passed, without_passed, runs, ts }>`, the latest trial for that skill and project.
- It renders as `    trial: admit (with 3/3 · without 0/3, 2026-11-13)`.
- **Test:** a later trial replaces an earlier one.

**Advise:**
- A skill whose latest trial is reject, if it is still in `<project>/.claude/skills/<name>`, gets "trial says it hurts: remove it or re-craft it".
- Read the advise code before writing this. If it doesn't fit the contributor shape, keep it to stats and note that in the CHANGELOG.

**Docs:**
- `skills/girth/references/trial.md`, plus a one-line pointer in `SKILL.md` (keep it under the word budget).
- README and CHANGELOG `[Unreleased]`.

**Gates:** `make check`, `cargo mutants --in-diff`, then release 0.25.0.
