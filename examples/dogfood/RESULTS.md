# Dogfooding: Girth built with Girth

Girth is built with Girth, the way a self-hosting compiler is built with itself.

## Stage 0
`girth search`, `girth measure` and `girth validate` were written in Rust without Girth's help.

## Stage 1 (2026-10-14)
The repo's `.girth/config.json` adds two plugin skill dirs to the default roots:
- superpowers `6.4.1/skills`
- Anthropic `skill-creator`

**Need:** "writing a skill description that triggers reliably, skill frontmatter and body structure"

**`girth search` top hits:**
- `skill-creator` (72 KB of markdown): Description Optimization, Writing Style
- `writing-skills` (97 KB): SKILL.md Structure, Skill structure

Only the matched sections were read, not the full 168 KB.

**Crafted:** `.claude/skills/girth-skill-authoring`, 2.6 KB, `VALID`. Its always-on cost is 166 B (≈ 42 tokens).

**Effect on the product.** The crafted skill carries writing-skills' rule that *a description must say only when to use a skill, never summarise its workflow, or agents follow the summary and skip the body.* The plan's draft description for Girth ("Craft a lean, project-specific skill … Use when …") broke that rule. Girth's shipped description was rewritten to trigger-only, and `tests/skill_md.rs` now enforces it.

**Bug found by dogfooding.** Piping `girth search … | head` panicked on a broken pipe. Fixed, with the `closed_stdout_does_not_panic` regression test.

**The crafted skill is live in this repo.** Claude Code sessions here list `girth-skill-authoring` among the available skills.

## Studio example (2026-10-17)

**Need:** "make a short portrait motion-design video with HyperFrames: six labelled steps lit one at a time, sourced data, story beats, timing, check and render"

**`girth search` top hits** (39 installed skills, 3.66 MB of markdown):
- `hyperframes` (139 KB), `hyperframes-core` (100 KB) and `motion-graphics` (82 KB)
- `hyperframes-animation` (869 KB), `hyperframes-creative` (349 KB) and `pr-to-video` (134 KB)

**Crafted (headless, `examples/studio/craft.sh`):** `girth-studio`, 3.8 KB, `VALID`, $0.19. Its sources:
- `motion-graphics`: the step flow
- `explainer-beat-sheet`: storyboard blocks
- `hyperframes-creative`: data in motion
- `hyperframes-cli`: check and render

It was then used for the 8-run A/B in `examples/studio/RESULTS.md`, which showed −29% cost at equal quality.

**Bug found while building it.** `claude -p` reports only the *last* turn in `usage` and `num_turns`. When a background task finishes after the answer, that last turn is a short follow-up, so per-run token counts would have read near zero. The studio summary uses `modelUsage`, which covers the whole session. `examples/compounding` still reads `usage`, but its 15 runs are unaffected: in each one, `usage` output tokens equal `modelUsage`'s. `examples/hyperframes` was recorded before `modelUsage` existed, and its `num_turns` (8 and 10) cover the whole session.
