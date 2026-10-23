# OpenMontage pack: 49 public video skills vs one crafted slice, 8 runs per arm

**Date:** 2026-10-18 · **Model:** claude-sonnet-5-5 · **Runs:** 8 per arm, each a fresh project and a fresh session · **Grader:** a script (`grade.py`), not a model.

## Why this pack

[OpenMontage](https://github.com/calesthio/OpenMontage) (≈ 69K stars, pinned at `c806645`) turns a coding agent into a video studio. Opening a project with it loads its 49 Claude Code skills on every turn: 14,495 B always on (≈ 3.6K tok), with 1.96 MB of markdown behind them. People install it today. This run asks what a project that only needs one slice of it pays.

## Task (`task.md`)

This is the studio brief, in Remotion: a 9-second, 1080×1350, 30 fps video, "6 steps, one video".
- Claude writes `spec.md`, sourced `data.json`, `story.md` and `timing.json` first.
- `npx tsc --noEmit` must pass, and `npx remotion render` must produce `final.mp4`.
- Every run starts from a hard-linked copy of the same installed Remotion 4.0.536 project (`template/`), so neither arm pays for `npm install`.

## Arms (`run.sh`)

User-level settings, skills and plugins are hidden in both arms. Both load the girth plugin and get an isolated `GIRTH_HOME`.
- **bulk:** the 49 OpenMontage skills, **markdown only**. `setup.sh` leaves out the pack's 93 Python files and other scripts, so no arm can run third-party code.
- **girth:** one `girth-studio` skill, crafted once by `craft.sh` from the pack alone ($0.188). Its sections came from `remotion` and `remotion-best-practices`: timing conventions, premounting, `Series`, frame references and springs. It passes `girth validate`.
  - The crafted text is not committed: OpenMontage is AGPL-3.0 and this repo is MIT. `craft.sh` rebuilds it.

## Grade (`grade.py`)

A run passes only if all of these hold:
1. `tsc --noEmit` exits 0.
2. `final.mp4` is 1080×1350 at 30 fps and 9 s (± 0.25 s).
3. The four hand-off files are valid.
4. All six labels are in the source or in a JSON file the source imports.

## Results (`summarize.py --table runs`)

| Arm | Pass | Cost per run, mean ± 95% CI | Median cost | Fresh input tok (mean) | Output tok (mean) |
|---|---|---|---|---|---|
| bulk | 8/8 | $0.114 ± $0.012 | $0.107 | 16,020 | 4,117 |
| girth | 8/8 | $0.106 ± $0.040 | **$0.087 (−19%)** | 13,495 | 3,944 |

**Quality held:** 16/16 runs passed.

**The saving is smaller than in the studio test, and one run dominates the mean.**
- Seven of the eight girth runs ($0.084–$0.095) cost less than *every* bulk run ($0.102–$0.143). One-sided Mann–Whitney U = 56/64, p = 0.007.
- The eighth girth run cost $0.224. Its transcript shows the render running slowly while the other arm rendered at the same time. Claude spent its extra turns polling for the file with `sleep`, `Monitor` and `pgrep`. The cause was the machine, not the skill, but it is reported as measured.
- On the mean, the 95% intervals overlap (−7%). On the median, girth is 19% cheaper.

**Why the gap is smaller than in the studio test (−29%):**
- The bulk arm here pulled in less. Its fresh input averaged 16.0K tokens, against 31.6K for the 39 HyperFrames-centred skills.
- OpenMontage's always-on listing is 15% smaller, and its Remotion skills are shorter than HyperFrames' 21 KB entry skill.
- Girth's saving tracks how much the installed skills cost a session, and this pack is the cheaper case.

## Harness lesson
Don't run arms that render video at the same time on one laptop. `run.sh` still allows it, so run the arms one after the other, as `Reproduce` below does.

## Reproduce
```sh
make dist
examples/openmontage/setup.sh /tmp/om        # clones and pins the pack, builds the Remotion template
examples/openmontage/craft.sh /tmp/om        # once
examples/openmontage/run.sh bulk  /tmp/om 1 8
examples/openmontage/run.sh girth /tmp/om 1 8   # OM_MAX_USD caps each arm (default $8)
python3 examples/openmontage/summarize.py --table examples/openmontage/runs
```
