Make a 9-second HyperFrames video in ./studio/, 1080×1350 (portrait) at 30 fps, titled "6 steps, one video". It shows six labelled panels side by side: Reference, Data, Story, Motion, Direct, Ship. It lights them one at a time, 1.5 s each, with that step's output file under the lit panel: spec.md, data.json, story.md, index.html, timing.json, final.mp4.

Work the way the steps say, and write each file before you animate:
- studio/spec.md: the brief, broken down (format, length, beats, style).
- studio/data.json: every number shown on screen, each with a "source" field.
- studio/story.md: one point and three beats.
- studio/timing.json: every panel's start and end in seconds. It must match the composition.

Then run `npx hyperframes check` in ./studio/ and fix every finding. Render to ./studio/final.mp4 with `--quality draft`. Approval to render is already given, so don't open a preview and don't wait. Finish by reporting the check result and the render's duration.
