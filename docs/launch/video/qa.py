# Checks a rendered PNG sequence before encoding:
#   python3 qa.py frames/film
# - glitch frames: a grey background (GPU hiccup), a frame far darker than both neighbours,
#   or a black run longer than any dip in the edit
# - strobe: seconds whose mean frame-to-frame change is above STROBE (thin lines judder)
# - frozen: runs of FROZEN_S seconds or longer where nothing changes at all
import os
import sys

import numpy as np
from PIL import Image

STROBE, FROZEN_S, BLACK_S, FPS = 8.0, 1.5, 1.0, 24
d = sys.argv[1]
names = sorted(n for n in os.listdir(d) if n.endswith('.png'))


def load(n):
    return np.asarray(Image.open(os.path.join(d, n)).convert('L').resize((480, 270)), dtype=np.float32)


frames = [load(n) for n in names]
bad = []
for i, a in enumerate(frames):
    if np.median(a) > 12:
        bad.append((names[i], 'grey background'))
    if 0 < i < len(frames) - 1:
        lo = min(frames[i - 1].mean(), frames[i + 1].mean())
        if lo > 4 and a.mean() < 0.3 * lo:
            bad.append((names[i], 'black frame between lit neighbours'))
# a lost GPU context keeps writing black frames without an error; real dips last under BLACK_S
run = 0
for i, a in enumerate(frames + [None]):
    if a is not None and a.mean() < 0.3:
        run += 1
        continue
    if run > BLACK_S * FPS:
        bad.append((f'{names[i - run]}..{names[i - 1]}', f'{run / FPS:.1f}s of black'))
    run = 0
steps = np.array([np.abs(frames[i + 1] - frames[i]).mean() for i in range(len(frames) - 1)])
strobe = [(s, steps[s * FPS:(s + 1) * FPS].mean()) for s in range(len(steps) // FPS)
          if steps[s * FPS:(s + 1) * FPS].mean() > STROBE]
frozen, run = [], 0
for i, v in enumerate(steps):
    run = run + 1 if v < 0.02 else 0
    if run == int(FROZEN_S * FPS):
        frozen.append((i + 1 - run) / FPS)

for n, why in bad:
    print(f'GLITCH {n}: {why}')
for sec, v in strobe:
    print(f'STROBE {sec}s: {v:.1f}')
for t in frozen:
    print(f'FROZEN from {t:.1f}s')
print(f'{len(frames)} frames, {len(bad)} glitches, {len(strobe)} strobing seconds, {len(frozen)} frozen holds')
sys.exit(1 if bad else 0)
