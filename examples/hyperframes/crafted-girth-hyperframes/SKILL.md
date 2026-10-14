---
name: girth-hyperframes
description: Use when building a short HyperFrames promo with a word-by-word highlighted caption track (karaoke-style), including captions sub-composition wiring, seek-safe state changes and the final check/render gates.
metadata:
  girth:
    sources:
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-creative
        sections: [frame-presets/coral/caption-skin.html]
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-core
        sections: [SKILL.md pitfalls, references/sub-compositions.md, references/minimal-composition.md]
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-studio
        sections: [2. One caption track, safe zones]
      - path-or-repo: /home/mohessaid/.claude/skills/hyperframes-cli
        sections: [development loop]
    crafted_for: short HyperFrames promo with word-by-word caption highlighting
    created: 2026-10-14
    last_used: 2026-10-14
---
# HyperFrames promo + word-highlight captions

Word timings (`{text,start,end}` per word, grouped into short lines) come from TTS/transcription (`npx hyperframes transcribe` / `tts`). Never invent timings.

## Structure
- Root `index.html`: `data-composition-id`, `data-start`, `data-width`, `data-height`, `data-duration`; holds only timed hosts, media and audio. Each scene with nested markup is its own file via `data-composition-src`.
- Captions: ONE sub-composition host, one `data-track-index`, `data-track-kind="captions"`, carrying every caption group. Never one row per group, never mixed with other kinds.
- Host needs `data-composition-id` (must equal the inner file's id), `data-composition-src="compositions/captions.html"`, `data-start`, `data-duration`, `data-track-index`, `data-width`, `data-height`.
- Sub-comp file: everything (markup, style, script) goes INSIDE `<template>`; head is discarded.
- Register `window.__timelines["captions"] = tl` (id must match the root `data-composition-id`), single paused GSAP timeline.

## Rules
- Seek-safe only: change word state with `tl.set(el, {className: ...}, time)`. NEVER `tl.call()` callbacks (seek does not fire them, wrong state on render).
- Exactly one group visible at a time (groups absolutely stacked, opacity set 1/0).
- Three word states: upcoming (faint), `.is-active` (highlight fill), `.is-spoken` (solid). Highlight via `box-shadow` spread, so no layout shift.
- Add a full-span anchor `tl.to({}, {duration: DURATION}, 0)`.
- Captions stay inside the title-safe box (80%, 10% inset per edge); everything else inside action-safe (90%).
- No scene-exit `tl.set(..., {visibility:"hidden"})`; use opacity fades.
- Don't center with CSS `translate(-50%,-50%)` on nodes GSAP animates (`gsap_css_transform_conflict`); use flex/`inset`.
- Full-bleed backgrounds go on a `class="clip"` layer, not `#root`.
- Don't add extra scenes, music or transitions the request didn't ask for.
- Source skills disagree on nothing material here; coral skin is the most specific caption source.

## Build loop
1. Write the brief and storyboard; get word timings.
2. Write `compositions/captions.html` from [references/caption-skin.md](references/caption-skin.md) (fill `GROUPS`, `DURATION`).
3. `npx hyperframes check` (runs lint first; deprecated aliases: validate/inspect/layout). Fix `caption_*` findings.
4. `npx hyperframes preview --background`, give the user the URL, wait for approval.
5. Render only after approval: `--quality draft` while iterating, `npx hyperframes render --quality looks --output out.mp4`, `delivery` for final.
6. Verify the file exists and its duration matches the root `data-duration`.
