#!/usr/bin/env bash
set -euo pipefail
mkdir -p .girth vendor-skills/video-mega vendor-skills/copy-mega
cat > .girth/config.json <<'JSON'
{"roots": ["vendor-skills"], "output": "crafted-skills"}
JSON
cat > vendor-skills/video-mega/SKILL.md <<'MD'
---
name: video-mega
description: Everything about producing videos - editing, captions, color grading, audio, export.
---
# Video mega skill
## Color grading
Use a LUT per scene. Lift shadows slightly for social platforms.
## Caption timing
Never show more than 2 lines of captions at once.
Each caption stays on screen for at least 1.2 seconds.
Always break lines at natural phrase boundaries, never mid-phrase.
## Audio mixing
Duck music by 12 dB under voiceover.
## Export settings
H.264, 1080p, 30 fps, -14 LUFS for social.
MD
cat > vendor-skills/copy-mega/SKILL.md <<'MD'
---
name: copy-mega
description: Marketing playbook - landing page copy, email cadence, SEO, ads.
---
# Copy mega skill
## Tone of voice
Write in plain, warm, confident language.
Never use slang, acronyms like lol or btw, or emoji in product copy.
Address the reader as "you" and lead with the outcome they get.
## Email cadence
Send onboarding emails on day 0, 2, 5 and 9.
## SEO keywords
Put the primary keyword in the H1 and the first 100 words.
## Ad bidding
Start with manual CPC for the first two weeks.
MD
python3 - <<'PY'
p = "vendor-skills/video-mega/SKILL.md"
s = open(p).read().replace("## Audio mixing", "## Caption styling\nWhite text on a 60% black box.\nNever exceed 32 characters per caption line.\n## Audio mixing")
open(p, "w").write(s)
PY
mkdir -p crafted-skills/girth-captions
cat > crafted-skills/girth-captions/SKILL.md <<'MD'
---
name: girth-captions
description: Use when timing captions or subtitles for this project's promo videos.
metadata:
  girth:
    sources:
      - path-or-repo: vendor-skills/video-mega
        sections: [Caption timing]
    crafted_for: caption timing rules
    created: 2026-10-01
    last_used: 2026-10-01
---
## Caption timing <!-- src: video-mega#caption-timing -->
- Never show more than 2 lines of captions at once.
- Each caption stays on screen for at least 1.2 seconds.
- Always break lines at natural phrase boundaries, never mid-phrase.
MD
