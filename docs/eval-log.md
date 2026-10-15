# Eval log

Behavioural suite run with `make evals` (`claude plugin eval` on the staged dist plugin, with the release binary bundled at `skills/girth/bin/girth`).

| Date | Model | Case | Runs | Score | Cost |
|---|---|---|---|---|---|
| 2026-10-14 | claude-sonnet-5-5 | craft-captions | 3 | 1.00 | $0.20 |
| 2026-10-14 | claude-sonnet-5-5 | tone-copy | 3 | 1.00 | $0.20 |
| 2026-10-14 | claude-sonnet-5-5 | no-source | 3 | 1.00 | $0.15 |

What the graders check:
- **craft-captions:** the skill fired; `girth search` ran; `crafted-skills/girth-captions/SKILL.md` exists; `VALID girth-captions` appears; the "1.2 seconds" rule is kept; colour grading, LUT, export and LUFS content is dropped.
- **tone-copy:** tone guidance is kept verbatim; email cadence and ad bidding are dropped; the skill validates.
- **no-source:** nothing is written under `crafted-skills/`, and the reply says no source was found (LLM judge, 3/3 PASS each run).

Sample crafted output (craft-captions, run 1):

```markdown
---
name: girth-captions
description: Use when timing, splitting or line-breaking captions for promo videos in this project.
metadata:
  girth:
    sources:
      - path-or-repo: <workspace>/vendor-skills/video-mega
        sections: [Caption timing]
    crafted_for: caption timing rules in our promo videos
    created: 2026-10-14
    last_used: 2026-10-14
---
# Caption timing

- Never show more than 2 lines of captions at once.
- Each caption stays on screen for at least 1.2 seconds.
- Always break lines at natural phrase boundaries, never mid-phrase.
```

## 2026-10-14: v0.3 taste memory

`make evals` (claude-sonnet-5-5, 3 runs per case) on the release tree: **4 of 4 cases at 1.00, $0.93.**

| Case | Score | Cost |
|---|---|---|
| craft-captions | 1.00 | $0.23 |
| no-source | 1.00 | $0.16 |
| taste-launch (new) | 1.00 | $0.29 |
| tone-copy | 1.00 | $0.25 |

What `taste-launch` checks:
- The scaffold seeds a project-tier house rule (an end card with `girth.dev`), plus three earlier launch videos that all used `neon-on-black` / `synthwave`.
- In run 1, the model picked paper-ink and minimal-techno, said why ("your last video used neon-on-black and synthwave"), put girth.dev on the end card, and recorded its choices with `girth made`.

**Manual end-to-end check** (claude-haiku-5-5, a plugin-style hook calling the real launcher, an isolated `GIRTH_HOME`):
- The hook injected the taste, and the model applied the house rule.
- The first run refused to execute the `made` line, because it appeared only in hook output. The crafted-skill template gained a `## Record` section for this reason.
- With that section, the model recorded `mood=hopeful` and explicitly varied away from the last recorded `melancholy`.

## 2026-10-15: M2 (self-trigger, fetch, show)

`make evals` on the release tree: **7 of 7 cases at 1.00 (21 runs), $1.66.**
- New: `auto-captions`, `no-fire-generic`, `extend-captions`.
  - The with-only graders show Girth firing unprompted on the specialised task in **3 of 3** runs, and **0 of 3** on the generic one.
  - Ablation (`--ablation with-without`) gives Δ = 0 on output quality for these fixtures. The source skills sit inside the fixture project, so a capable model without Girth reads them directly. Girth's measured value on capable models is cost at equal quality (`examples/compounding/RESULTS.md`), not a quality lift. Hiding sources from the baseline only would manufacture a Δ, so it isn't done.
- Regression caught and fixed: after `girth show` landed, `tone-copy` fell to 1 of 3. The model read the unlabelled heading list as the whole skill. The overview is now labelled, and `tone-copy` is back to 3 of 3.
