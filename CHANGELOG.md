# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `girth fetch owner/repo[/path]` shallow-clones a public GitHub skill repo into an untrusted scratch dir (`~/.girth/fetch/`).
  - Only plain `owner/repo` specs are accepted, so no URL, ssh, file transport or flag can reach git.
  - Git hooks are disabled, submodules are skipped, and only the https protocol is allowed.
- `girth show <skill-dir> [--heading H]…` prints a skill's frontmatter and table of contents, or only the named sections (with their subsections). Crafting now reads sources through it, so it needs no reads outside the project and never loads whole files.
- Remote fallback (`references/remote.md`): when no installed skill covers a need, Girth finds candidates with `npx skills find`, fetches at most two and distils from them.
- M2 evals: `auto-captions` (Girth fires unprompted on a specialised task: 3 of 3), `no-fire-generic` (it stays silent on a generic task: 0 of 3 fired) and `extend-captions` (a second need extends the existing skill).
- `examples/compounding/`: the 5-session compounding benchmark (bulk vs per-session JIT vs Girth's persistent cache).

## [0.3.0] - 2026-10-14

### Added
- **Taste memory**: house rules and a variation history per domain, for recurring deliverables (launch videos, decks, landing pages).
  - New commands: `girth taste`, `girth taste show|add|drop`, `girth made`.
  - Two tiers: personal (`~/.girth/taste.jsonl`, follows you across projects) and project (`.girth/taste.jsonl`, can be committed). Local only; `share` never reads it.
  - The `PostToolUse` hook now returns the taste as `additionalContext` when a crafted `girth-<domain>` skill, or one of its source skills, is used. Lines are pre-filtered, so a 1,000-event taste file adds about 1 ms.
  - `validate` links a crafted skill's source skills to its domain.
  - `stats` shows taste counts, never rule text.
- The skill gains a taste section and `references/taste.md`, and triggers when the user states a lasting preference. Always-on cost: 74 → 92 tokens.
- New eval `taste-launch`: a house rule is applied, recent choices are varied, and the deliverable is recorded (3 of 3 runs at 1.00).
- The launcher exports `GIRTH_LAUNCHER`, so taste context names the exact command to run.

### Fixed
- A third-party skill whose name happens to start with `girth-` is no longer counted as crafted. A crafted skill must also carry a `metadata.girth` block.
- `search` printed the warning for an invalid `.girth/config.json` twice. It now prints it once.

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

[Unreleased]: https://github.com/mohessaid/girth/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/mohessaid/girth/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mohessaid/girth/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/mohessaid/girth/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mohessaid/girth/releases/tag/v0.1.0
