---
name: girth-skill-authoring
description: Use when writing or editing a SKILL.md in this repo, Girth's own skill or a crafted girth-* skill, especially its description or trigger wording.
metadata:
  girth:
    sources:
      - path-or-repo: ~/.claude/plugins/cache/claude-plugins-official/superpowers/6.4.1/skills/writing-skills
        sections: [SKILL.md Structure, Rich Description Field, Keyword Coverage, Token Efficiency (Critical)]
      - path-or-repo: ~/.claude/plugins/marketplaces/claude-plugins-official/plugins/skill-creator/skills/skill-creator
        sections: [How skill triggering works, Writing Style, Writing Patterns]
    crafted_for: writing Girth's own SKILL.md so it triggers reliably and stays small
    created: 2026-10-14
    last_used: 2026-10-14
---

# Skill authoring for Girth

## Frontmatter
- Two required fields: `name` and `description`. Max 1024 characters total.
- `name`: letters, numbers and hyphens only.
- `description`: third person, describes ONLY when to use (NOT what it does). Start with "Use when..." and include specific symptoms, situations and contexts. Keep it under 500 characters if possible.

## The description is the trigger
**NEVER summarize the skill's process or workflow in the description.** When a description summarizes the workflow, agents may follow the description instead of reading the full skill content. The skill body becomes documentation agents skip.

- BAD: `Use for TDD - write test first, watch it fail, write minimal code, refactor`
- GOOD: `Use when implementing any feature or bugfix, before writing implementation code`

Use the words an agent would search for: symptoms, synonyms, tool names, file types. Keep triggers technology-agnostic unless the skill is technology-specific.

## How triggering works
Skills appear in Claude's `available_skills` list with their name + description. Claude only consults skills for tasks it can't easily handle on its own. Simple one-step queries may not trigger a skill even when the description matches perfectly. Complex, multi-step or specialized queries reliably trigger skills when the description matches. Trigger evals therefore need substantive prompts, and their should-not-trigger cases should be near misses, not obviously irrelevant prompts.

## Body
- Frequently-loaded skills: aim for < 200 words. Others: < 500 words.
- Move details into `--help` or `references/` instead of repeating them in SKILL.md.
- Prefer the imperative form. Explain why something matters rather than piling up MUSTs.
- Draft, then reread it with fresh eyes and tighten.
