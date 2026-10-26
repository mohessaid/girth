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

## OpenMontage example (2026-10-18)

**Need:** "short motion-design videos in Remotion: labelled steps lit one at a time, a written spec, sourced data, story beats, a timing file, then type check and render"

**`girth search`** ran over the 49 OpenMontage Claude Code skills only (`GIRTH_SKILL_ROOTS`, a markdown-only mirror: 1.96 MB). Girth read matched sections of `remotion` (Toolkit Best Practices, Project Timing Conventions, Transition Duration Guidelines) and `remotion-best-practices` (Premounting, `Series`, Frame References Inside Sequences, Spring animations, Calculate Metadata).

**Crafted (headless, `examples/openmontage/craft.sh`):** `girth-studio`, 3.7 KB, `VALID`, $0.19. It wasn't committed because the source pack is AGPL-3.0.

**Bug found while building it.** The first craft quietly drew from the author's own user-level skills (`paper-ink-explainer`, `explainer-beat-sheet`), because `claude -p` loaded user settings even though `GIRTH_SKILL_ROOTS` pointed only at the pack. Both craft scripts (`openmontage`, `skillsbench`) now pass `--setting-sources project,local`, so a craft can only see the sources the experiment names.

## `girth drift`, first run on this repo (2026-10-23)

```
$ girth drift .claude/skills/girth-video .claude/skills/girth-skill-authoring
girth-video: unavailable product-launch-video
girth-video: unavailable faceless-explainer
girth-video: unavailable hyperframes-animation
girth-skill-authoring: up to date
```

Between 2026-10-17 and 2026-10-23, the author's user skills went from 40 to 26. Three of `girth-video`'s five sources were uninstalled. The crafted skill still carries their distilled slice: that is the point of the cache, and the intro video it guided was finished. But nothing can confirm that slice is still current, and `drift` now says so instead of staying silent.

**Bug found while building it.** `cargo clippy` in the full gate caught a `src.clone()` slice in the new tests. Separately, the gate's run hit a real launcher race: `an_interrupted_download_leaves_nothing_behind` failed under load because `mktemp` ran before the cleanup traps were set. Fixed in the launcher; the test passes 8 of 8 with the CPU saturated.

## `girth advise` against `/usage` (2026-10-26)

| Cause | `/usage` | `girth advise` (author's ledger) |
|---|---|---|
| Large conversations (`/clear`, `/compact`) | 95% (24 h) | 90–98% |
| Subagents on the main model | 61% (24 h) | 30% (7 d, on 2026-10-25); 21% on 2026-10-26 |
| `/playwright-cli` | 16% | 0.04% |

**Why the skill row differed (Kestrelle #18).** It was not a hook gap. Every Skill call since the ledger began (2026-10-13), in the main loop and in subagents, has a matching `use` line; the one subagent call after that date matches to the second. The difference is attribution. A loaded skill stays in context for the rest of the session and is re-injected after each compaction: one `playwright-cli` call in a long session appeared 72 times in its transcript. `/usage` charges the skill for that residency. `advise` charged only the turn after the call.

**Change.** The mod now records the session id on every turn, and the Skill hook records it, plus whether a subagent ran the skill, on every use. `advise` charges a skill for every later main turn of its session, and adds *turns 8 h+ into a session* (fix: `/clear` on a task switch).

**Pending.** Ledger lines from before this change have no session id, so they keep the one-turn charge. Re-run the comparison once a week of new sessions has accumulated.
