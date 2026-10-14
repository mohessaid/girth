# Girth

[![CI](https://github.com/mohessaid/girth/actions/workflows/ci.yml/badge.svg)](https://github.com/mohessaid/girth/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/mohessaid/girth/graph/badge.svg)](https://codecov.io/gh/mohessaid/girth)
[![CodeQL](https://github.com/mohessaid/girth/actions/workflows/codeql.yml/badge.svg)](https://github.com/mohessaid/girth/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/mohessaid/girth/badge)](https://scorecard.dev/viewer/?uri=github.com/mohessaid/girth)
[![Release](https://img.shields.io/github/v/release/mohessaid/girth?sort=semver)](https://github.com/mohessaid/girth/releases)
[![License: MIT](https://img.shields.io/github/license/mohessaid/girth)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-orange?logo=rust)](Cargo.toml)
[![Mutation score](https://img.shields.io/badge/mutation-99.2%25-brightgreen)](docs/mutation-notes.md)
[![Evals](https://img.shields.io/badge/evals-3%2F3%20passing-brightgreen)](docs/eval-log.md)
[![Claude Code plugin](https://img.shields.io/badge/Claude%20Code-plugin-d97757)](#install)

**Just-in-time, project-scoped agent skills.** Instead of installing dozens of large skills and paying for all of them every turn, Girth distils only the slice your project needs into a small `girth-<domain>` skill, at the moment you need it. Later sessions reuse it from the project's cache.

Status: pre-release (M1). Design: `docs/superpowers/specs/`.

## Measured so far (2026-10-14, one machine)

| | Before | With Girth (bulk skills removed) |
|---|---|---|
| Always-on skill cost (39 user skills vs crafted) | 17,022 B ≈ 4.3K tok | 448 B ≈ 112 tok (**38× less**) |
| Per-session new prompt tokens, HyperFrames task | 35.7K | 22.0K (**−38%**) |
| Task cost, same task and model | $0.181 | $0.149 (**−17%**) |
| HyperFrames check on the output | 1 warning | 0 warnings |

These are single runs. See `examples/hyperframes/RESULTS.md` for the full numbers and caveats, and `docs/eval-log.md` for the behavioural eval suite (3 cases, 9 runs, all 1.00).

## Install

In Claude Code (2.1.275 or later):

```
/plugin install girth --marketplace mohessaid/girth
```

Or, with the skills CLI: `npx skills add mohessaid/girth`.

You don't need Rust. The first time the skill runs, it downloads the prebuilt `girth` binary for your platform from [Releases](https://github.com/mohessaid/girth/releases) and verifies it against `SHA256SUMS` before running it. Builds exist for Linux x86_64 and ARM64 (static musl) and macOS (Intel and Apple Silicon). To use your own build, set `GIRTH_BIN`.

## How it works

1. During a task, the model (or you: "use girth for …") names a need.
2. `girth search` ranks the skills already on disk and the project's crafted-skill cache. It points at the exact files and headings to read.
3. The model distils only that slice into `.claude/skills/girth-<domain>/SKILL.md`. Rules are kept verbatim, tone guidance is never compressed, and the body is ≤ 3 KB.
4. `girth validate` enforces the contract. The next time the domain comes up, the crafted skill is a cache hit.

## CLI

```bash
cargo build --release
target/release/girth measure                      # what your skills cost in context
target/release/girth search "caption timing" --json
target/release/girth validate .claude/skills/girth-captions
```

Skill roots resolve in this order:
1. `--roots`
2. `GIRTH_SKILL_ROOTS`
3. `.girth/config.json` (`{"roots": [...], "output": "..."}`)
4. The defaults: `~/.claude/skills`, `~/.agents/skills`, `.claude/skills`

## Development

```bash
make check    # fmt, clippy, tests (unit, property, CLI, launcher), coverage ≥ 90%, plugin validate
make mutate   # cargo-mutants
make evals    # claude plugin eval on a staged dist copy (spends model credits, capped)
cargo deny check   # advisories, licenses, sources
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [CHANGELOG](CHANGELOG.md). Report security issues privately ([SECURITY.md](SECURITY.md)).

Girth is built with Girth: see `examples/dogfood/RESULTS.md`.

## License

MIT
