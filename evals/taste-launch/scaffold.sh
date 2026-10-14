#!/usr/bin/env bash
set -euo pipefail
mkdir -p .girth vendor-skills/launch-mega
cat > .girth/config.json <<'JSON'
{"roots": ["vendor-skills"], "output": "crafted-skills"}
JSON
# Project-tier taste: one house rule and three earlier launch videos.
cat > .girth/taste.jsonl <<'JSONL'
{"v":1,"ts":1760000000,"kind":"rule","domain":"launch-video","text":"Always end on an end card that shows girth.dev.","op":"add"}
{"v":1,"ts":1760000100,"kind":"made","domain":"launch-video","project":"/old/a","choices":{"palette":"neon-on-black","music":"synthwave"}}
{"v":1,"ts":1760000200,"kind":"made","domain":"launch-video","project":"/old/b","choices":{"palette":"neon-on-black","music":"synthwave"}}
{"v":1,"ts":1760000300,"kind":"made","domain":"launch-video","project":"/old/c","choices":{"palette":"neon-on-black","music":"synthwave"}}
JSONL
cat > vendor-skills/launch-mega/SKILL.md <<'MD'
---
name: launch-mega
description: Everything about product launch videos - structure, palettes, music, podcasts, thumbnails.
---
# Launch mega skill
## Launch video structure
Hook in the first 2 seconds. Then problem, product, proof, and a call to action.
Keep the whole video under 45 seconds.
## Palette choices
Pick exactly one palette per video: neon-on-black, paper-ink, pastel or mono-bold.
## Music
Pick one bed: synthwave, lo-fi, orchestral or minimal-techno. Duck it under any voiceover.
## Podcast editing
Cut filler words and normalise to -16 LUFS.
## Thumbnail SEO
Put the product name in the first three words of the title.
MD
