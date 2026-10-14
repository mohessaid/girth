# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/mohessaid/girth/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/mohessaid/girth/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mohessaid/girth/releases/tag/v0.1.0
