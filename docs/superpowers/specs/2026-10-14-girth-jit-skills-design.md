# Girth v1 — Just-in-Time Project Skills

**Date:** 2026-10-14
**Status:** Draft for review
**Layer:** Harness (first layer of the Girth stack)

## 1. Problem

Published agent skills bundle a whole domain into one install (e.g. 13 `hyperframes-*` skills, several overlapping video pipelines). A real project needs a small slice from many domains: engineering, marketing, content, video. Users therefore install dozens of large skills, use a fraction of each, and pay for the rest:

- **Always-on cost:** every installed skill's description sits in context on every turn. In the author's setup, 52 skills mean ~29 KB (~7–8K tokens) of descriptions per turn, and near-duplicate descriptions confuse routing.
- **Invocation cost:** `SKILL.md` bodies average ~13 KB; some skills pull references up to ~870 KB.
- **Discovery tools** (find-skills, skills.sh) help locate skills but still install them whole.

## 2. Goal

The harness grows its **own lean, project-specific skills on demand**, at the moment it falls short, by distilling only the needed slice from existing skills. This works the way a model reaches for web search when it lacks a fact.

The mental model is a **cache**: crafted project skills are the cache, the skill ecosystem (local plus remote) is the backing store, and a capability gap is a miss.

**Non-negotiable:** output quality must not drop. Compression applies to the *skill*, never to the *style of the work the skill produces*.

## 3. Scope

**In v1:**
- A Claude Code skill named `girth`, plus small helper scripts.
- Two triggers: model-initiated and user-initiated (`/girth <need>`).
- Local-first sourcing, with remote search as a fallback.
- The crafted-skill format, distillation rules, growth/merge policy.
- A measurement script for before/after token cost.

**Out of v1 (later):**
- **Hooks (plugin form):** a `UserPromptSubmit` hook that detects correction phrases ("that's wrong", "not like that", "too generic") and nudges the model to call Girth. A `Stop`/failure hook could do the same. This makes the trigger automatic rather than relying on self-awareness.
- **MCP server:** expose `girth_craft(need)` as a true tool for cross-harness use (Codex, Cursor, others), reusing the same scripts.
- **Usage-driven pruning:** evict or merge crafted skills based on `last_used` and observed usage across sessions.

## 4. Architecture

```
 task in progress
       │  gap detected (model self-check OR user runs /girth <need>)
       ▼
 ┌─────────────────────────┐
 │ girth SKILL.md          │  tiny always-on description (~40 words)
 │ (procedure for model)   │
 └──────────┬──────────────┘
            │ 1. check cache
            ▼
   .claude/skills/girth-*/        ── hit ──▶ extend if needed, use it
            │ miss
            │ 2. search sources (scripts/search.sh)
            ▼
   ~/.claude/skills, ~/.agents/skills   (local, no network)
            │ nothing relevant
            ▼
   remote: skills.sh / find-skills search, GitHub fetch into scratch dir
            │
            │ 3. model reads candidates, extracts needed slice
            ▼
   write/extend .claude/skills/girth-<domain>/SKILL.md
            │ 4. scripts/measure.sh reports token cost
            ▼
   continue the original task using the crafted skill
```

**Division of labour:** the running model does the distillation. The scripts only search, fetch and measure. That keeps v1 to one `SKILL.md` and a few shell/Python helpers, with no LLM calls of its own.

### Components

| Unit | Purpose | Interface |
|---|---|---|
| `skills/girth/SKILL.md` | Trigger description plus the craft procedure | Loaded by the harness; invoked by the model or `/girth` |
| `scripts/search.sh <query>` | Ranks candidate skills by matching the query against name, description and headings across local roots; falls back to `npx skills find` | Prints ranked list: path, size, matched lines |
| `scripts/fetch.sh <repo-or-url>` | Shallow-fetches a remote skill into a fresh untrusted scratch dir; never into a skills dir | Prints the local path |
| `scripts/measure.sh [dir]` | Reports always-on description bytes/tokens and body sizes for a skills dir | Table plus totals |
| Crafted skills `.claude/skills/girth-<domain>/` | The cache | Standard skill format (below) |

## 5. Triggers

1. **Model-initiated.** The `girth` description tells the model to invoke it when the current task needs domain know-how it lacks or is unsure of (conventions, library APIs, craft rules), and no project skill covers it. The `SKILL.md` adds a short self-check: "Am I about to produce generic output for a specialised domain?"
2. **User-initiated.** `/girth <need>`, e.g. `/girth caption animation in hyperframes`. This covers the case the model can't detect: the user sees the output is falling short.

## 6. Source precedence

1. Project `.claude/skills/girth-*` (already crafted: cache hit)
2. Local installed skills: `~/.claude/skills`, `~/.agents/skills`, project `.claude/skills` (non-girth)
3. Remote: find-skills / skills.sh search, then GitHub fetch into scratch

Girth may *use* find-skills to locate a skill. It **never installs whole skills**. Remote content is treated as untrusted data: it is read and distilled, never executed.

## 7. Crafted-skill contract

```markdown
---
name: girth-<domain>
description: <≤30 words, project-specific: what this project does in this domain>
metadata:
  girth:
    sources:
      - path-or-repo: <path or owner/repo>
        ref: <commit or version, if known>
        sections: [<headings used>]
    crafted_for: <the task/need that triggered crafting>
    created: <YYYY-MM-DD>
    last_used: <YYYY-MM-DD>
---
<body ≤ 3 KB>
```

**Distillation rules** (these protect quality):
- Keep **verbatim**: "never/always/must" constraints, gotchas, exact API signatures, required file structures, version-specific behaviour.
- Keep only what the triggering need (and clearly adjacent needs) touches. Drop examples, motivation prose and unrelated sub-topics.
- **Never compress tone-of-output guidance into terse style rules.** If a source skill governs how output should read (copy, docs, narration), preserve those instructions in full fidelity. The skill is compressed; the work it produces is not.
- Record which source sections were used so the skill can be re-distilled or extended later.
- If the needed slice can't fit in 3 KB, put the overflow in `references/<topic>.md` inside the crafted skill. Loading it stays on demand.

## 8. Growth policy

- On a gap in a domain that already has a `girth-<domain>` skill, **extend it** rather than create a new one, and update `sources` and `crafted_for`.
- At most **one crafted skill per domain**. When the body passes 3 KB, move detail into `references/` instead of splitting.
- If crafted skills in a project exceed **8**, Girth proposes merges to the user before creating another.
- Every use updates `last_used`.

## 9. Error handling

- **No relevant source found:** say so and continue the task unaided. Never fabricate a skill from the model's own guesses and label it as distilled.
- **Fetch fails or no network:** use local sources only and report it.
- **Conflicting sources:** prefer the most specific and most recent source, and note the conflict in the crafted skill.
- **User declines** (in user-visible flows): Girth writes nothing.

## 10. Success criteria

1. **Token cost:** for a demo project, `measure.sh` shows always-on description cost after Girth (girth plus crafted skills only, with bulk installs removed) is **≥10× lower** than with the bulk installs.
2. **Quality demo:** a real captioning or animation task, done with the author's 13-skill hyperframes stack versus one crafted `girth-hyperframes` skill, produces output judged **no worse** by the author. This is a side-by-side and becomes the launch artifact.
3. **Incrementality:** a second, different need in the same domain extends the existing crafted skill rather than creating a new one.
4. **Install:** a single `npx skills add <repo>` (or copy into `.claude/skills`) makes Girth usable.

## 11. Repository layout

```
girth/
  skills/girth/SKILL.md
  skills/girth/scripts/{search.sh,fetch.sh,measure.sh}
  docs/superpowers/specs/
  examples/            # before/after demo artefacts
  README.md
```

## 12. Testing

- Script tests: `search.sh` against a fixture skills dir with known matches; `measure.sh` against fixtures with known byte counts.
- Behavioural evals, run with fresh sessions: (a) model-initiated trigger fires on a specialised-domain task with no matching project skill; (b) it does not fire for generic tasks; (c) `/girth` crafts a valid skill matching the contract; (d) a second need extends rather than duplicates.
- The success-criteria demo (section 10) serves as the acceptance test.
