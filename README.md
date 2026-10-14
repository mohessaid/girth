<div align="center">

# Girth

**Just-in-time, project-scoped agent skills.**<br>
Distil only the slice your project needs from the skills you already have — 38× lower always-on skill cost.

<p>
  <a href="https://github.com/mohessaid/girth/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/mohessaid/girth/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://codecov.io/gh/mohessaid/girth"><img alt="codecov" src="https://codecov.io/gh/mohessaid/girth/graph/badge.svg"></a>
  <a href="https://github.com/mohessaid/girth/actions/workflows/codeql.yml"><img alt="CodeQL" src="https://github.com/mohessaid/girth/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/mohessaid/girth"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/mohessaid/girth/badge"></a>
  <a href="https://github.com/mohessaid/girth/releases"><img alt="Release" src="https://img.shields.io/github/v/release/mohessaid/girth?sort=semver"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/github/license/mohessaid/girth"></a>
  <a href="Cargo.toml"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85-orange?logo=rust"></a>
  <a href="docs/mutation-notes.md"><img alt="Mutation score" src="https://img.shields.io/badge/mutation-99.5%25-brightgreen"></a>
  <a href="docs/eval-log.md"><img alt="Evals" src="https://img.shields.io/badge/evals-3%2F3%20passing-brightgreen"></a>
  <a href="#install"><img alt="Claude Code plugin" src="https://img.shields.io/badge/Claude%20Code-plugin-d97757"></a>
</p>

<a href="#install">Install</a> · <a href="#how-it-works">How it works</a> · <a href="examples/hyperframes/RESULTS.md">Results</a> · <a href="CONTRIBUTING.md">Contributing</a>

</div>

Instead of installing dozens of large skills and paying for all of them every turn, Girth distils only the slice your project needs into a small `girth-<domain>` skill, at the moment you need it. Later sessions reuse it from the project's cache.

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

## Ledger & feedback

Girth keeps a local, append-only ledger (`~/.girth/ledger.jsonl`) of which skills you use, which sections of which source skills your crafted skills keep or drop, and your 👍/👎. Ask *"girth stats"* to see which installed skills actually earn their place, or *"girth review captions"* to remove sections you don't want. Removals become signal too.

**Sharing is opt-in and anonymous.** Girth asks once. If you allow it, only **content hashes of public skill sections plus counts** (used / kept / removed / up / down) would be shared, never names, paths, text, notes or anything from your project. Nothing is uploaded until the Girth index launches. Run `girth share preview` to see exactly what would be sent.

## CLI

```bash
cargo build --release
target/release/girth measure                      # what your skills cost in context
target/release/girth search "caption timing" --json
target/release/girth validate .claude/skills/girth-captions   # also records to the ledger
target/release/girth stats                                      # what the ledger says
target/release/girth share preview                              # exactly what would be shared
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
