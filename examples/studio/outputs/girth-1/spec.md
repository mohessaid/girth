# Spec: "6 steps, one video"

## Format
- 1080 x 1350 (portrait), 30 fps, silent, MP4 (draft quality render)
- Root composition: `index.html`, id `main`

## Length
- 9 s total = 6 steps x 1.5 s, no intro or outro padding

## Beats
| # | Panel     | Window (s) | Output file |
|---|-----------|-----------|-------------|
| 1 | Reference | 0.0-1.5   | spec.md     |
| 2 | Data      | 1.5-3.0   | data.json   |
| 3 | Story     | 3.0-4.5   | story.md    |
| 4 | Motion    | 4.5-6.0   | index.html  |
| 5 | Direct    | 6.0-7.5   | timing.json |
| 6 | Ship      | 7.5-9.0   | final.mp4   |

Six labelled panels sit side by side. One is lit at a time; the rest are dimmed. The lit panel's output file name appears under it.

## Style
- Dark ground `#0a0a0a`, panels `#161618`, text `#f4f4f5`, dimmed text `#6b6b73`
- One accent, `#ffb020`, meaning "this step is active"
- Inter for labels and title, monospace for file names
- Each panel pairs its step number with a fill bar that fills over its 1.5 s
- No charts, gridlines or legends. Built with GSAP + CSS only
