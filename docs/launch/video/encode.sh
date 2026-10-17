#!/bin/sh
# Encodes the rendered PNG sequence into the silent intro: greyscale, a fixed chalkboard grain (moving grain made the file 480 MB), no audio.
#   ./encode.sh [frames/film] [girth-intro.mp4]
set -eu
in=${1:-frames/film}
out=${2:-girth-intro.mp4}
ffmpeg -y -loglevel error -framerate 24 -i "$in/%04d.png" \
  -vf "format=gray,noise=c0s=5:c0f=p,format=yuv420p" \
  -c:v libx264 -preset slow -crf 18 -movflags +faststart -an "$out"
echo "wrote $out"
