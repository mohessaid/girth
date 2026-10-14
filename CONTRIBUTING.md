# Contributing to Girth

Thanks for helping. Girth is small on purpose; contributions that keep it small, fast and measured are the most welcome.

## Setup

You need Rust (stable, MSRV 1.85). Optional tools for the full gate: `cargo-nextest`, `cargo-llvm-cov`, `cargo-mutants`, `cargo-deny`.

```bash
git clone https://github.com/mohessaid/girth && cd girth
make check     # fmt, clippy -D warnings, tests, coverage >= 90%, plugin validate
```

## How we work

- **Test first.** Every behaviour change starts with a failing test (unit, proptest, CLI in `tests/cli.rs`, or launcher in `tests/launcher.rs`).
- **Measure claims.** Performance or token-saving claims come with numbers and how they were produced (see `examples/`).
- **Frozen interfaces.** The JSON keys of `girth search` and `girth measure`, the strings `VALID <dir>` and `INVALID <dir>: <msg>`, and the `.girth/config.json` keys are relied on by the skill and the evals. Changing them requires updating both, and a CHANGELOG entry.
- **Skill text.** When editing any `SKILL.md`, follow `.claude/skills/girth-skill-authoring`: a description says only *when* to use the skill.
- **Mutation testing.** `make mutate` should stay above 80% caught. Equivalent mutants go in `docs/mutation-notes.md`.
- **Evals** (`make evals`) spend model credits, so maintainers run them before releases.

## Pull requests

Keep PRs focused. Make sure `make check` passes, add a CHANGELOG line under *Unreleased*, and describe what you measured.
