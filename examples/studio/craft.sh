#!/bin/sh
# Craft the girth arm once: girth searches every installed skill in ~/.claude/skills and
# distils the slice craft.md asks for into crafted/girth-studio. Headless Claude Code with
# the dist plugin (make dist) and an isolated GIRTH_HOME; the cost lands in craft.json.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
dist="$here/../../target/plugin-dist"
work=$(mktemp -d)
mkdir -p "$here/crafted"
(cd "$work" && GIRTH_HOME="$work/girth-home" GIRTH_SKILL_ROOTS="$HOME/.claude/skills" GIRTH_OUTPUT_DIR="$here/crafted" \
  claude -p "$(cat "$here/craft.md") Only craft and validate the skill, then stop." \
  --plugin-dir "$dist" \
  --settings '{"enabledPlugins":{"girth@girth":false}}' \
  --model "${MODEL:-claude-sonnet-5-5}" \
  --max-budget-usd "${CRAFT_MAX_USD:-1}" \
  --add-dir "$HOME/.claude/skills" --add-dir "$here/crafted" \
  --allowedTools "Read Write Edit Skill Bash(*/skills/girth/scripts/girth *)" \
  --output-format json < /dev/null > "$here/craft.json" 2>"$work/craft.err" || cat "$work/craft.err" >&2)
rm -rf "$work"
python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print('craft: \$%.3f, %s turns' % (d['total_cost_usd'], d['num_turns']))" "$here/craft.json"
ls "$here/crafted"
