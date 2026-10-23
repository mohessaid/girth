./video/ is an installed Remotion project (package.json, tsconfig.json and node_modules are in place; there is no source yet). In it, make a 9-second video, 1080×1350 (portrait) at 30 fps, titled "6 steps, one video". It shows six labelled panels side by side: Reference, Data, Story, Motion, Direct, Ship. It lights them one at a time, 1.5 s each, with that step's output file under the lit panel: spec.md, data.json, story.md, Main.tsx, timing.json, final.mp4.

Work the way the steps say, and write each file before you animate:
- video/spec.md: the brief, broken down (format, length, beats, style).
- video/data.json: every number shown on screen, each with a "source" field.
- video/story.md: one point and three beats.
- video/timing.json: every panel's start and end in seconds. It must match the composition.

Put the source in video/src/ (entry video/src/index.ts, composition id "Main"). `npx tsc --noEmit` must pass in ./video/. Render with `npx remotion render src/index.ts Main final.mp4` in ./video/. Approval to render is already given, so don't open a preview and don't wait. Finish by reporting the type check and the render's duration.
