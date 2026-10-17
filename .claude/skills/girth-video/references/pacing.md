# Pacing: the director's pass

Run this on a cut before the final render: contact sheets every 0.5 s, the caption in/out times, and a motion-energy chart (mean frame-to-frame change per second).

## Energy curve
- Thin lines on black strobe at 24 fps when a second's mean change is above about 8. Cap camera moves near 5, and let an object do the travelling (a scan line, a card) instead of the camera.
- Never freeze: a hold that changes nothing for 1.5 s or more reads as a stalled player. Put a slow push or drift (about 1) under every hold.
- Don't front-load the motion. If the first 20 s run hot and the rest goes flat, the film plays as rushed, then dead.

## Holds
- A caption stays up for its words at ~2.6 per second, plus 0.5 s to settle. The verdict lands only after the title is read, never at the same moment.
- A big beat (the turn, the headline number) gets 0.5 s of quiet before it and 3–4 s of hold after it.
- Remove dead frames: an empty set before the first object draws, or black at a cut that isn't a planned dip.

## 14. Break rhythm once per 30s <!-- src: embedded-captions#14-break-rhythm-once-per-30s -->
Every-word-same-way = eye adapts, stops registering motion. Plant a rhythm-break every ~30s: a phrase from the opposite direction, a single word at 2× size, a beat of pure silence with no caption.

## Accelerating cadence <!-- src: hyperframes-animation#accelerating-cadence-geometric-hold-decay -->
For a roll-call (several stations in a row), shrink the holds instead of using equal shot lengths: early entries hold for reading, later ones get shorter, and the last brakes longest so the stop registers as a beat. Equal lengths (3-3-3) sound like a metronome.

## Edit on a clock
Author scenes on one clock and retime them with a piecewise-linear edit list (authored time → final time). Then a station can grow or shrink without touching its animation code.
