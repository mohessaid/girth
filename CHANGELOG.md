# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-10-14

### Added
- **Local skill ledger** (`~/.girth/ledger.jsonl`, overridable with `GIRTH_HOME`): an append-only record of skill uses, crafted-skill versions and feedback. A truncated line is skipped, never fatal.
- **Automatic use tracking:** a `PostToolUse` plugin hook runs `girth record use`. It takes ~1.5 ms and never fails the session.
- **Section provenance:** crafted sections carry `<!-- src: skill#slug -->` markers. `girth validate` records source coverage (`used/total`) and diffs each version, showing which sections were added and removed.
- `girth feedback <skill> up|down [--note]`. Notes stay local.
- `girth stats [--json]`: crafted skills, uses, removals, ratings, source coverage and most-used skills.
- `girth share status|allow|deny|preview`: consent is asked once by the skill. The preview shows the exact anonymous payload (source-section hashes and counts only). **Nothing is uploaded in 0.2.**
- Skill flows: review (remove sections through a multi-select), feedback, stats and sharing, loaded on demand from `references/`.

### Changed
- The `girth validate` VALID line now reports `(recorded: +A −R sections)`. Use `--no-record` to skip recording.
- Search headings ignore HTML comments, so provenance markers don't affect ranking.

## [0.1.1] - 2026-10-14

### Added
- Signed releases: Sigstore keyless bundles (`*.sigstore.json`) and GitHub build-provenance attestations for every binary and `SHA256SUMS`.
- Continuous fuzzing of the frontmatter parser and validator (`cargo fuzz`, 6.8M local executions, no crashes).
- CI quality signals: Codecov coverage, an MSRV (1.85) check, cargo-deny, CodeQL, OpenSSF Scorecard and Dependabot.
- Community files: CONTRIBUTING, Code of Conduct, SECURITY, issue and PR templates.

## [0.1.0] - 2026-10-14

### Added
- The `girth` CLI (Rust): `search`, `measure` and `validate`.
- The `girth` skill and Claude Code plugin, plus a marketplace manifest.
- A launcher that downloads checksum-verified release binaries (Linux musl x86_64/ARM64, macOS Intel/Apple Silicon).
- A behavioural eval suite (3 cases, all 1.00) and the hyperframes acceptance demo (38× lower always-on skill cost).

[Unreleased]: https://github.com/mohessaid/girth/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/mohessaid/girth/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/mohessaid/girth/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mohessaid/girth/releases/tag/v0.1.0
