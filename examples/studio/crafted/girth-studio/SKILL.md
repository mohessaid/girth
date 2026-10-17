---
name: girth-studio
description: Use when making a short motion-design video with HyperFrames in this project, from a brief through spec, sourced data, story beats and timing file to check and render.
metadata:
  girth:
    sources:
      - path-or-repo: /home/mohessaid/.claude/skills/motion-graphics
        sections: [Step 0 — Initialize, Step 1 — Plan, Step 2 — Source, Step 4 — Build, Step 5 — Verify, Step 6 — Approve and render]
      - path-or-repo: /home/mohessaid/.claude/skills/explainer-beat-sheet
        sections: [Each station written as, Rules that make it work, Deliverable checklist]
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-creative
        sections: [Design Spec, Data in Motion]
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-cli
        sections: [Preview before render, Render choices]
    crafted_for: short motion-design videos with HyperFrames - labelled steps lit one at a time, spec, sourced data, story beats, timing file, check, render
    created: 2026-10-17
    last_used: 2026-10-17
---
## Steps (announce each by label; do one at a time) <!-- src: motion-graphics#flow -->
1. **Init** - only if `hyperframes.json` is absent: `npx hyperframes init "$PROJECT_DIR" --non-interactive --example=blank --skill=motion-graphics`. `PROJECT_DIR=videos/<subject>-motion`. Never init in the workspace root or nest a project; run commands as `(cd "$PROJECT_DIR" && ...)`.
2. **Spec** - write/read the design spec. Order: `frame.md` > `design.md` > `DESIGN.md` (`frame.md` always lowercase). Frontmatter is normative: quote hex, fonts, weights verbatim, never invent. Prose is intent. Brand, not layout.
3. **Data** - source every number; write `FACTS.md` (claim -> source; computed values -> the script that computes them). Verify current facts by search, never memory. No superlatives. Illustrative diagrams are marked so.
4. **Beats** - write `STORYBOARD.md`, one block per step: `id · eyebrow`, figure (what is drawn, what it does), title (<=6 words/line, 2-3 lines), gloss (one line), verdict (1-3 words or a number, lands 1-2 s after the title is readable, never together), footnote (one grey line). Get user approval before building.
5. **Timing** - write the timing file (`film.json`: every step `id` and `beats`, plus `lines` if narrated). Silent: read on-screen text aloud, ~2.6 words/s must fit. Narrated: on-screen text is the short form; key each verdict to its word. Never change a fixed `data-duration` to hide a defect.
6. **Build** - reuse-first: `npx hyperframes add <block>`, customize in place. Output `compositions/index.html`: paused GSAP timeline on `window.__timelines`, `class="clip"` with stable ids, `tl.seek(0)`, deterministic. Steps are labelled and lit one at a time (the active step lit, others dimmed), one accent per meaning.
7. **Check** - `npx hyperframes check .` then `npx hyperframes snapshot --at <opening, signature move, final hold>`; inspect the sheet. One in-place repair pass on failure, then rerun the failed gate.
8. **Render** - `npx hyperframes preview --background`, ask once: "render now, or what changes?" Render only on explicit yes: `npx hyperframes render . -q high -o ./renders/video.mp4` (`--format webm|mov` for transparent). Verify the file exists, is non-empty, has intended duration; hand off artifact, duration, proof times.

## Data in motion <!-- src: hyperframes-creative#data-in-motion -->
- Successive stats of one concept keep the same visual space and look; only the value changes.
- Pair every number with a visual (fill bar, ring, shape). No pie charts, multi-axis charts, 6-panel dashboards, gridlines/ticks/legends, or chart libraries; build with GSAP + SVG/CSS.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
