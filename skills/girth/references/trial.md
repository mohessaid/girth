# Check whether a crafted skill helps

A trial runs the same task with and without the skill and grades each run with a command. Runs cost model usage, so start one only when the user asks.

The default agent runs with permissions bypassed inside each throwaway clone, since a headless run can't answer a prompt. The skill's instructions then run commands with the user's environment, so trial only skills the user would let run (never a fetched, unreviewed one).

1. Ask for, or agree on, two things:
   - **A task prompt.** Write it to a file outside the repo.
   - **A check command.** It runs in the work tree after the agent and exits 0 when the work is right, such as `cargo test -q` or `test -f out.pdf`.
2. Make sure the repo has a commit: each run starts from a clone at HEAD, and uncommitted work isn't included.
3. A trial takes minutes per run: run it in the background, or with a long command timeout. Run `scripts/girth trial <output_dir>/girth-<domain> --task <file> --check "<cmd>"`.
   - It defaults to 3 runs per arm.
   - Add `--model` to pick the model.
   - For another harness, use `--agent "<cmd>"` with a command-line agent that reads the prompt on stdin, such as `codex exec --skip-git-repo-check -s workspace-write -`. The skill is placed in `.claude/skills` and `.agents/skills`.
4. Report the verdict line and the per-task counts as printed.
   - **Admit:** the skill earns its place.
   - **Reject:** offer to remove it, or to review and re-craft it (`references/review.md`).
   - **Undecided:** the gap could be chance. Suggest `--runs 5` to `10` (the maximum), or a task where the skill matters more.
   - **"Every run failed in both arms" or "ended in an error":** fix the task, the check or the agent's setup first.
   - **If the skill was used in 0 runs,** fix its description first; nothing was tested.

A task counts only when chance would give its gap at most 1 time in 20, so at 3 runs only 3/3 vs 0/3 decides. Cost is reported, not judged. `scripts/girth stats` shows each crafted skill's latest verdict.
