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
  - They run each case in a bubblewrap sandbox. Ubuntu 24.04 and later block the user namespaces bubblewrap needs (`kernel.apparmor_restrict_unprivileged_userns = 1`), so without a fix every Bash call fails and every crafting case scores near 0.
  - `make evals` checks for this first. The narrow fix is an AppArmor profile that lets only bwrap create user namespaces:
    ```sh
    printf 'abi <abi/4.0>,\ninclude <tunables/global>\nprofile bwrap /usr/bin/bwrap flags=(unconfined) {\n  userns,\n  include if exists <local/bwrap>\n}\n' | sudo tee /etc/apparmor.d/bwrap && sudo apparmor_parser -r /etc/apparmor.d/bwrap
    ```
    Then `make sandbox-ok` should pass.

## Pull requests

Keep PRs focused. Make sure `make check` passes, add a CHANGELOG line under *Unreleased*, and describe what you measured.
