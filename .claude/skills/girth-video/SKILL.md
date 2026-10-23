---
name: girth-video
description: Use when planning, scripting or building an explainer, launch or intro video for this project, especially a silent one, or choosing scene transitions and on-screen captions.
metadata:
  internal: true
  girth-sources: "~/.claude/skills/explainer-beat-sheet, ~/.claude/skills/product-launch-video, ~/.claude/skills/faceless-explainer, ~/.claude/skills/hyperframes-animation, ~/.claude/skills/embedded-captions"
  girth-crafted-for: "a silent 60–75 s chalk intro video that makes Girth's core idea clear"
  girth-created: "2026-10-17"
  girth-last-used: "2026-10-17"
---
## Story first <!-- src: explainer-beat-sheet#structure-2-min-170-beats-at-84-bpm, product-launch-video#story-design-product-launch-video -->
Decide the film before code: write `STORYBOARD.md` and `FACTS.md`. Build the sequence around the **promise**, not a feature list. Opening 3–5 s: ONE hook in the viewer's words; the promise lands by beat 2, features after that are its evidence. Arc: Hook · Mechanism (one idea per station, dependency order) · Turn · Proof · Close (pays off the hook).

## Each station <!-- src: explainer-beat-sheet#each-station-written-as -->
2 shots, hard cut between: shot A states the idea, shot B shows its consequence and lands the verdict. Title ≤ 6 words per line; gloss one line; verdict 1–3 words or a number; footnote one grey line of fact.

## Rules <!-- src: explainer-beat-sheet#rules-that-make-it-work -->
- **Show the mechanism, not a metaphor.** Draw the actual thing, with real data computed by a script.
- **Readable before impact.** The verdict lands 1–2 s after the title is readable. Never both at once.
- **Every number has a source** in `FACTS.md`. No superlatives.
- Silent films: on-screen text carries the whole argument; read it aloud at speed to check it fits (~2.6 words per second).
- Before rendering in full, run a director's pass on pacing: `references/pacing.md`.

## Transitions <!-- src: product-launch-video#3-cut-the-curve-scene-transitions, faceless-explainer#transitions, hyperframes-animation#scene-transitions -->
- Cut the curve: the outgoing hero accelerates one way, the cut lands mid-motion, the incoming hero continues the **same direction** and decelerates. Mirrored eases (exit `power4.in`, entry `power4.out`), same distance and duration. Partial travel + fade, never full off-screen moves. Gaps where nothing moves read as dead air.
- Pick ONE primary (60–70% of changes) + 1–2 accents; never a different transition per scene. Opening: most distinctive. Topic change: something different. Climax: boldest accent. Outro: slowest, simplest (dip to black, 0.6–1.0 s).
- Exit animations are banned except on the final scene: the transition IS the exit.

## Record
After each deliverable, run the taste context's `made` command with its defining choices.
