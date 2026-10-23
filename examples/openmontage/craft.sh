#!/bin/sh
# Usage: craft.sh <workdir>   (after setup.sh)
# Crafts the girth arm once: girth searches only the OpenMontage pack and distils the slice
# craft.md asks for into <workdir>/crafted/girth-studio. Headless, dist plugin (make dist),
# isolated GIRTH_HOME. The cost lands in craft.json. The crafted skill stays in <workdir>:
# OpenMontage is AGPL-3.0, so its distilled text is not committed to this MIT repo.
set -eu
work=$1
here=$(cd "$(dirname "$0")" && pwd)
dist="$here/../../target/plugin-dist"
tmp=$(mktemp -d)
rm -rf "$work/crafted" && mkdir -p "$work/crafted"
(cd "$tmp" && GIRTH_HOME="$tmp/girth-home" GIRTH_SKILL_ROOTS="$work/pack" GIRTH_OUTPUT_DIR="$work/crafted" \
  claude -p "$(cat "$here/craft.md") Only craft and validate the skill, then stop." \
  --setting-sources "project,local" --plugin-dir "$dist" \
  --settings '{"enabledPlugins":{"girth@girth":false}}' \
  --model "${MODEL:-claude-sonnet-5-5}" \
  --max-budget-usd "${CRAFT_MAX_USD:-1}" \
  --add-dir "$work/pack" --add-dir "$work/crafted" \
  --allowedTools "Read Write Edit Skill Bash(*/skills/girth/scripts/girth *)" \
  --output-format json < /dev/null > "$here/craft.json" 2>"$tmp/craft.err" || cat "$tmp/craft.err" >&2)
rm -rf "$tmp"
python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print('craft: \$%.3f' % d['total_cost_usd'])" "$here/craft.json"
"$dist/skills/girth/bin/girth" validate "$work/crafted/girth-studio"
