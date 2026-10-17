#!/bin/sh
# Renders frames START..END into OUT in 48-frame chunks, each in a fresh Blender.
# A lost GPU context keeps writing black frames without an error; when a chunk ends
# in 12+ black frames, those frames are deleted and the chunk is retried (up to 3 times).
#   ./render.sh frames/film 1 900 & ./render.sh frames/film 901 1800
cd "$(dirname "$0")" || exit 1
out=$1; s=$2; e=$3
while [ "$s" -le "$e" ]; do
  ce=$((s + 47)); [ "$ce" -gt "$e" ] && ce=$e
  for try in 1 2 3; do
    blender -b -P blender/film.py -- --out "$out" --render 1 --start "$s" --end "$ce" > /dev/null 2>&1
    bad=$(python3 - "$out" "$s" "$ce" <<'PY'
import sys
import numpy as np
from PIL import Image
out, s, e = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
run = 0
for i in range(e, s - 1, -1):
    try:
        lit = np.asarray(Image.open(f'{out}/{i:04d}.png').convert('L')).mean() >= 0.3
    except Exception:
        lit = False
    if lit:
        break
    run += 1
if run >= 12:
    print(*range(e - run + 1, e + 1))
PY
)
    [ -z "$bad" ] && break
    echo "frames $s-$ce: black from ${bad%% *}, retry $try" >&2
    for i in $bad; do rm -f "$out/$(printf %04d "$i").png"; done
  done
  s=$((ce + 1))
done
