# Girth v0.9: cost per outcome (design)

**Status:** the user picked "cost per outcome" among the v0.6+ directions (2026-10-15) and authorised nonstop work. This written spec is pending the author's review.
**Evidence:** `docs/research/2026-10-15-where-the-money-goes.md` §6, `examples/outcomes/analyze-outcomes.py`.

## Why
- **Meter says what a turn cost, not what it bought.** The industry is moving from cost per token to cost per task. Girth can measure it locally, from outcomes the repo already has.
- **On the author's history,** a commit cost $3.33 on average and from $0.95 to $6.41 per project. The median spend between commits was $1.51, p95 $11.43. The number tells projects and weeks apart, so it can show whether a lever (Lens, a crafted skill, idle compaction, a cheaper subagent model) changed what work costs.
- **A test run's exit status is useless for this.** 99% of agent test commands are piped, and by exit status 98% passed; by the runners' own summaries 35% failed. Girth reads the summary.

## What it measures
For each Bash call in a turn (main or subagent), from the command and the end of its output:

| Outcome | When |
|---|---|
| `commit` | a `git commit` (not `--dry-run`) whose call did not error |
| `pr` | a `gh pr create` or `gh pr merge` whose call did not error |
| `test_pass` | a test command whose output ends with a runner's passing summary and no failing one |
| `test_fail` | a test command whose call errored, or whose output has a failing summary |
| `test_untold` | a test command with neither (output filtered away, or no runner summary) |

- **Test commands:** `cargo test|nextest|llvm-cov`, `make check|test`, `npm|pnpm|yarn|bun [run] test`, `npx vitest|jest|playwright test`, `vitest`, `jest`, `pytest`, `python -m pytest`, `go test`, `deno test`, `flutter test`, `dart test`, `mix test`, `rspec`, `phpunit`, `claude plugin test`.
- **Normalising a command line:**
  - It is split on `&&`, `||`, `;`, `|` and newlines; heredoc bodies are dropped first; `cd …` segments are skipped.
  - Leading wrappers are stripped: `rtk`, `rtk proxy`, `sudo`, `time`, `env`, `timeout N`, `nice -n N`, `mise exec … --`, `uv run`, `NAME=value`, and a `/usr/bin/`-style path.
  - `git -C <dir> commit` counts.
- **Summaries read from the last 4,000 characters:**
  - **Failing:** `test result: FAILED`, a line starting `FAIL` or `FAILED`, `N failed|failure(s)|error(s)` with N ≥ 1, `fail N` (node) or `N fail` (bun) with N ≥ 1, `could not compile`, `make: *** … Error`.
  - **Passing:** `test result: ok.`, `N passed`, a line starting `ok ` (go) or `PASS`, `N tests, 0 failures` (ExUnit), `fail 0`, `N pass`, `Ran N tests across` (bun).
- **Calls that were backgrounded or interrupted** are ignored: their output is not the run's.

## Privacy
- **No command text or output is stored.** The mod hands each call's command and output tail to `girth meter record` on stdin. The binary classifies them and stores only the outcome labels on the turn: `"outcomes": ["test_fail", "commit"]`.
- **`share` never reads turns**, so outcomes never leave the machine. A sentinel property test enforces it.

## Components
- **Rust `src/outcome.rs`:**
  - `Outcome` (serialised snake_case);
  - `classify(command) -> Kinds { test, commit, pr }`;
  - `verdict(output, error) -> Outcome`;
  - `outcomes(calls: &[Call]) -> Vec<Outcome>`, where `Call { command, output, error, background }`.
- **Ledger:** `Turn` gains `outcomes: Vec<Outcome>`, skipped when empty. Old lines parse unchanged.
- **CLI:** `girth meter record` reads `bash: [Call]` from its input, sets `turn.outcomes` and never stores `bash`. At most 64 calls per turn are read.
- **Meter summary:**
  - It gains `commits`, `prs`, `tests_passed`, `tests_failed`, `tests_untold` and `since_commit`. `since_commit` is the cost of the turns after the project's last commit turn, or of all turns when there is none.
  - **A new line** when any outcome was recorded: `meter: outcomes: 12 commits · ≈ $3.40 per commit · ≈ $1.20 since the last commit · test runs: 20 passed, 8 failed, 2 no verdict`. A part with no data is left out.
  - **With `--project`,** the summary covers that project; without it, everything.
- **Mod:**
  - **`tool.call` for `Bash`:** awaits `next(e)`; waiting on `next` doesn't count against the hook budget. It records `{command, output: last 4,000 chars of text, error: isError === true, background}` under the loop's key (`e.agentId ?? ''`) and returns the result unchanged.
  - **`turn.complete`:** attaches and clears the list for its loop (`e.agentId ?? ''`).
  - **Failure handling:** every step is inside the existing fail-open `.catch`. A Bash call's result is never altered.

## Review focus
1. **A Bash call's result is returned unchanged,** including errors and denials, even when recording fails.
2. **No command text or output reaches the ledger.**
3. **Piped test runs** are judged by their output, not by the exit status.
4. **A subagent's Bash calls land on the subagent's turn,** not the main one.
5. **Old ledgers** without `outcomes` parse, and the stats for them are unchanged.

## Testing
- **Rust:** a unit table for `classify` (each runner, each wrapper, heredocs, `git -C`, `--dry-run`, `cd` chains) and for `verdict` (each summary format, N = 0 vs N ≥ 1, errors, no summary).
  - **Property tests:**
    - classification never panics on arbitrary input;
    - a test command piped to `tail` with a failing summary is `test_fail`.
  - **Ledger and stats:** a round trip, an old-line parse, the summary and render, `since_commit`, and the share sentinel.
  - **CLI:**
    - `meter record` with `bash` stores outcomes and no command text;
    - more than 64 calls are capped.
- **Mod (`claude plugin test`):**
  - Bash calls are recorded and attached on `turn.complete`;
  - the result passes through unchanged;
  - an agent's calls go on the agent's turn;
  - a non-Bash tool is not recorded.
- **Mutants** scoped to the changed files.
- **Live:**
  - one `claude -p` run of the dist build that runs a failing and then a passing test and commits, in a scratch repo;
  - check the turn's `outcomes` in an isolated `GIRTH_HOME`, and that the ledger holds no command text.
- **No A/B:** this is an instrument, not a lever. Its validation is the ground-truth check in the research (classifier recall against `git log`) and the gates above.
