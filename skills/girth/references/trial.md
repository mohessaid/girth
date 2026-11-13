# Check whether a crafted skill helps

A trial runs the same task with and without the skill and grades each run with a command. Runs cost model usage, so start one only when the user asks.

1. Ask for, or agree on, two things:
   - **A task prompt.** Write it to a file outside the repo.
   - **A check command.** It runs in the work tree after the agent and exits 0 when the work is right, such as `cargo test -q` or `test -f out.pdf`.
2. Make sure the repo has a commit: each run starts from a clone at HEAD, and uncommitted work isn't included.
3. Run `scripts/girth trial <output_dir>/girth-<domain> --task <file> --check "<cmd>"`.
   - It defaults to 3 runs per arm.
   - Add `--model` to pick the model, or `--agent "<cmd>"` for another harness's command-line agent that reads the prompt on stdin.
4. Report the verdict line and the per-task counts as printed.
   - **Admit:** the skill earns its place.
   - **Reject:** offer to remove it, or to review and re-craft it (`references/review.md`).
   - **Undecided:** a 1-pass difference isn't enough. Suggest more `--runs`, or a task where the skill matters more.
   - **If the skill was used in 0 runs,** fix its description first; nothing was tested.

A task counts only with a 2-pass difference, and cost is reported, not judged. `scripts/girth stats` shows each crafted skill's latest verdict.
