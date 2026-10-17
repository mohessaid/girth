# Girth intro video

A silent, 75-second, black-and-white chalk film about what Girth does. It's drawn in 3D by Blender's Line Art and built entirely from one Python script, so every render is identical.

- **`STORYBOARD.md`:** the story, scene by scene, with each scene's transition.
- **`FACTS.md`:** where every number and name on screen comes from.
- **`blender/film.py`:** the whole film: the props, the timeline, the camera and the captions. Every move is a plain function of time, keyed frame by frame.
- **`blender/lib.py`:** scene setup, the Line Art chalk with its subtle wobble, the chalk text, and the animation helpers.
- **`fonts/Caveat.ttf`:** the handwriting font (SIL Open Font License, `fonts/OFL.txt`).

```sh
# stills for review (frame = seconds × 24 + 1; stills are named with 5 digits, the sequence with 4)
blender -b -P blender/film.py -- --out frames/stills --frames 49,301,997
# the whole film as a PNG sequence (resumable; 48-frame chunks, retried if the GPU drops out)
./render.sh frames/film 1 900 & ./render.sh frames/film 901 1800; wait
python3 qa.py frames/film   # glitch frames, black runs, strobing seconds, frozen holds
./encode.sh frames/film girth-intro.mp4
```

Tested with Blender 5.2.2 LTS and ffmpeg 6.1. Run at most two render processes on a 4 GB GPU. A process that loses its GPU context keeps writing black frames without an error (with two long-running processes, both did, 30 s into the film), which is why `render.sh` starts a fresh Blender every 48 frames and retries a chunk that ends in black. A rare single frame still comes out grey or black; `qa.py` lists them (re-render those frame numbers with `--frames`, then rename `0XXXX.png` to `XXXX.png`). It also flags any second whose motion is high enough to strobe on thin lines, and any hold that freezes. The shelf shows the skills installed on the machine that renders it, from `~/.claude/skills`.

## How it was made

The video was made with Girth:
- `girth search` picked the video skills it learns from: explainer-beat-sheet, product-launch-video, faceless-explainer and hyperframes-animation.
- Girth distilled them into `.claude/skills/girth-video` (3 KB), which covers story first, one idea per scene, cutting on motion, and readable captions.
- `girth taste show video` supplied the house rules from the first round of feedback: chalk on black, subtle wobble, sparse frames.
- The film shows that same card in scenes 5 and 11.
