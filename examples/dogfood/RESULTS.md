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
