# Girth intro video

A silent, black-and-white, hand-drawn 3D intro, rendered by code.

- **`scene.js`:** the scene, its 52-second timeline and the sketch shader. The shader draws outlines where surfaces turn, hatching where they're in shade, paper or chalk grain, and lines that redraw slightly 8 times a second.
- **`render.mjs`:** renders frame by frame in headless Chrome (SwiftShader) and streams the frames into ffmpeg, so every run is identical.

```sh
npm install
node render.mjs chalk 52 girth-intro.mp4            # the intro (also: ink, pencil)
FRAMES=60,300,900 node render.mjs chalk 52 x.mp4 960 540   # stills in frames/chalk/ for review
```

Every number in it comes from the repo's measurements and is labelled as measured on one machine:
- 74% from the Lens A/B;
- $3.28 per commit from `examples/outcomes`;
- 583K → 21K tokens from the idle compaction example.

The video was made with Girth. `girth search` found the three.js approach among the installed skills, and the style feedback ("calm lines, less crowded, chalk") is in Girth's taste memory under `video`.
