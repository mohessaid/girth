#!/bin/sh
# Craft the girth arm: one girth-<domain> skill per task, distilled ONLY from
# that task's own curated skills (environment/skills), for the task's need.
#   examples/skillsbench/craft.sh <skillsbench checkout> <outdir>
# Runs headless Claude Code with the dist plugin (make dist) and an isolated
# GIRTH_HOME. The crafted skill lands in <outdir>/<task>/skills/girth-*/.
set -eu
sb=$1 out=$2
here=$(cd "$(dirname "$0")" && pwd)
dist="$here/../../target/plugin-dist"
mkdir -p "$out"
while read -r task; do
  [ -n "$task" ] || continue
  src="$sb/tasks/$task/environment/skills"
  dest="$out/$task"
  mkdir -p "$dest/skills" "$dest/work"
  need=$(python3 -c "import sys; print(open(sys.argv[1]).read().split('---', 2)[2].strip())" "$sb/tasks/$task/task.md")
  (cd "$dest/work" && GIRTH_HOME="$dest/girth-home" GIRTH_SKILL_ROOTS="$src" GIRTH_OUTPUT_DIR="$dest/skills" \
    claude -p "Use girth to craft (or reuse) the project skill this task needs. Do NOT do the task itself: only craft and validate the skill, then stop.

Task:
$need" \
    --plugin-dir "$dist" \
    --settings '{"enabledPlugins":{"girth@girth":false}}' \
    --model "${MODEL:-claude-sonnet-5-5}" \
    --max-budget-usd "${CRAFT_MAX_USD:-1}" \
    --add-dir "$src" --add-dir "$dest/skills" \
    --allowedTools "Read Write Edit Skill Bash(*/skills/girth/scripts/girth *)" \
    --output-format json > "$dest/craft.json" 2>"$dest/craft.err" || true)
  rm -rf "$dest/work"
  echo "$task: $(ls "$dest/skills") $(python3 -c "import json,sys;print(round(json.load(open(sys.argv[1]))['total_cost_usd'],3))" "$dest/craft.json" 2>/dev/null)"
done < "$here/tasks.txt"
