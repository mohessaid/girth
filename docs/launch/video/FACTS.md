# Facts behind the intro video

| On screen | Source |
|---|---|
| "dozens of skills"; the names on the shelf | The author's `~/.claude/skills` (40 entries on 2026-10-17); the Lens A/B used 74 skills (`examples/lens/RESULTS.md`). |
| "your AI carries all of them. every turn." | Every request carries the skill listing (each skill's name and description). Lens exists to shrink it: `examples/lens/RESULTS.md`. |
| "the video skills alone: 1.7 MB of manuals." | `find -L ~/.claude/skills/hyperframes* -name '*.md' \| xargs cat \| wc -c` → 1,767,491 bytes on 2026-10-17. |
| "next session, it reads them again." | The bulk arm of `examples/compounding/RESULTS.md` costs the same every session ($0.16–0.21). |
| the four source skills and their headings | `.claude/skills/girth-video/SKILL.md` metadata: explainer-beat-sheet, product-launch-video, faceless-explainer, hyperframes-animation. |
| "3 KB." / "from four skills, 1.1 MB of docs" | `girth-video/SKILL.md` is 3,120 bytes; the four sources' `.md` files total 1,093,246 bytes. |
| five sessions: $0.18 $0.19 $0.17 $0.21 $0.16 vs $0.22 $0.10 $0.09 $0.09 $0.10; totals $0.91 / $0.60; −34% | `examples/compounding/RESULTS.md`, final run (claude-sonnet-5-5, 2026-10-15). Illustrative bar heights use these values. |
| "$3.28" per commit | `docs/research/2026-10-15-where-the-money-goes.md` §6, 1,417 commits across 13 projects on the author's machine. |
| "keeper: your cache stays warm." | README, Keeper section. The prompt cache expires after 5 minutes or 1 hour. |
| "this video was made with girth." | `girth search` picked the sources above, `girth validate` recorded `girth-video`, and `girth taste show video` supplied the house rules (chalk on black, subtle wobble, sparse frames). |

All measurements were taken on one machine, and the video says so.
