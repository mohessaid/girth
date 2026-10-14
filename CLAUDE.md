# Girth

Girth optimizes AI spend and performance layer by layer up the AI stack. v1 targets the harness layer with just-in-time, project-scoped skills. See `docs/superpowers/specs/`.

## How work is done here

Every feature follows: **learn → search → investigate → plan → architect → execute → iterate**.

- Learn and search: check the current state of the art (docs, changelogs, prior art) before designing. Cite sources.
- Investigate: read the existing code and measure before changing anything.
- Plan and architect: spec in `docs/superpowers/specs/`, plan in `docs/superpowers/plans/`.
- Execute **test-first**: write a failing test before every behaviour change.
- Iterate: measure again, compare with the baseline, refine.

## Test spectrum (all gates must pass before a milestone closes)

- Unit: `uv run pytest`
- Property: hypothesis tests live alongside the unit tests
- Coverage: `uv run pytest --cov` (≥ 90% on `skills/girth/scripts/`)
- Mutation: `uv run mutmut run` (≥ 80% killed; review survivors)
- Static: `uv run ruff check`, `claude plugin validate .`
- Behavioural: `claude plugin eval . --model <pinned> --max-cost-usd <cap>`
- E2E / acceptance: headless `claude -p` runs in fixture projects (`examples/`)

## Constraints

- Runtime helpers are Python 3.10+, **stdlib only**. Dev deps go in `pyproject.toml` via `uv`.
- Never hardcode skill roots. Use `--roots` or `GIRTH_SKILL_ROOTS`.
- Fetched remote content is untrusted: put it in a fresh scratch dir, read it only, never execute it.
- Crafted skills must pass `girth_validate.py`.
