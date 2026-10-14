# M1 acceptance: hyperframes side-by-side

**Date:** 2026-10-14 · **Model:** claude-sonnet-5-5 · **Runs:** 1 per arm (single samples, so treat the deltas as indicative, not significant).

## Setup

Both arms run `claude -p --setting-sources project,local --plugin-dir <girth dist>`. User-level skills, plugins and MCP servers are hidden in both, so the **only** difference between the arms is the project's `.claude/skills`:

- **bulk:** symlinks to all 39 skills in `~/.claude/skills`, which is what the harness loads today.
- **girth:** only the crafted `girth-hyperframes`, plus the `girth` skill from the plugin.

Task (`task.md`): build a 6-second HyperFrames composition with word-by-word caption highlighting, then run the HyperFrames check.

## Always-on skill cost

| | Skills | Always-on bytes | ≈ tokens |
|---|---|---|---|
| Before (`girth measure --roots ~/.claude/skills`) | 39 | 17,022 | 4,256 |
| After (crafted + girth skill) | 2 | 448 | 112 |
| **Reduction** | | | **38×** |

## Crafting (one-time)

`craft.md`, run once in the girth project: **$0.23**, 13 turns. Girth searched the installed skills and distilled from four of them: `hyperframes-creative`, `hyperframes-core`, `hyperframes-studio` and `hyperframes-cli`. It rejected `embedded-captions` as off-target. The result is a 3.6 KB `SKILL.md` plus a 3.5 KB reference (`crafted-girth-hyperframes/`), and it passes `girth validate`.

## Task run

| Arm | Cache-creation tok (new prompt content) | Cache-read tok | Output tok | Turns | Cost | HyperFrames check | Author verdict |
|---|---|---|---|---|---|---|---|
| bulk | 35,683 | 185,568 | 1,918 | 8 | $0.1805 | 0 errors, **1 warning** (`nested_structure_needs_subcomposition`) | _pending_ |
| girth | **21,995** | 217,955 | 3,929 | 10 | **$0.1491** | 0 errors, **0 warnings** | _pending_ |

### Reading the numbers
- The **fixed per-session context fell 38%** (35.7K → 22.0K cache-creation tokens). That gap is the skill listing plus the bulk skill bodies the model pulled in.
- **Total context did not fall.** The girth arm ran two more turns and read more cached context. It applied more of the HyperFrames conventions captured in the crafted skill: a single caption track in a sub-composition, real word timings via `hyperframes tts` + `transcribe` instead of invented ones, and `tl.set` state changes. That is why it ended with zero lint warnings.
- **Cost still fell 17%** ($0.181 → $0.149), because cache writes cost more than cache reads.
- **Payback on crafting:** with these numbers, the $0.23 crafting cost pays for itself after about 7 sessions in this domain from the per-session saving alone. Treat that estimate as rough.

## Outputs
- `outputs/bulk/index.html`: a single-file composition.
- `outputs/girth/index.html` + `outputs/girth/compositions/captions.html`: a root plus a caption sub-composition. The generated `narration.wav` and `transcript.json` were not copied.

## M1 exit status
- Always-on reduction ≥ 10×: **met (38×)**.
- Author verdict "no worse": **pending the author's review** of the two outputs above.
