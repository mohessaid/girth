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
