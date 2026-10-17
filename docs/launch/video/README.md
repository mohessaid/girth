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
# the whole film as a PNG sequence (resumable; run ranges in parallel)
blender -b -P blender/film.py -- --out frames/film --render 1 --start 1 --end 900
blender -b -P blender/film.py -- --out frames/film --render 1 --start 901 --end 1800
./encode.sh frames/film girth-intro.mp4
```

Tested with Blender 5.2.2 LTS and ffmpeg 6.1. Run at most two render processes on a 4 GB GPU; with four, one lost its GPU context and wrote black frames without an error. A per-frame brightness scan catches that before encoding. The shelf shows the skills installed on the machine that renders it, from `~/.claude/skills`.

## How it was made

The video was made with Girth:
- `girth search` picked the video skills it learns from: explainer-beat-sheet, product-launch-video, faceless-explainer and hyperframes-animation.
- Girth distilled them into `.claude/skills/girth-video` (3 KB), which covers story first, one idea per scene, cutting on motion, and readable captions.
- `girth taste show video` supplied the house rules from the first round of feedback: chalk on black, subtle wobble, sparse frames.
- The film shows that same card in scenes 5 and 11.
