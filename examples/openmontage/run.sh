#!/usr/bin/env bash
# Usage: run.sh <bulk|girth> <workdir> <first-run> <last-run>   (after setup.sh and craft.sh)
# Each run is a fresh project (with a hard-linked copy of the Remotion template at ./video) and
# a fresh headless session on task.md, graded by grade.py.
#   bulk  = all 49 OpenMontage skills (markdown only) in the project's .claude/skills
#   girth = only the crafted girth-studio skill
# Both arms hide user-level settings, skills and plugins, load the girth plugin, and get an
# isolated GIRTH_HOME. Stops before a run once the arm's spend reaches OM_MAX_USD (default 8).
set -euo pipefail
arm=$1 work=$2 first=$3 last=$4
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
case "$arm" in bulk|girth) ;; *) echo "arm must be bulk or girth" >&2; exit 2 ;; esac
mkdir -p "$here/runs"
for n in $(seq "$first" "$last"); do
  spent=$(python3 "$here/summarize.py" --spent "$here/runs" "$arm")
  if python3 -c "import sys; sys.exit(float('$spent') < float('${OM_MAX_USD:-8}'))"; then
    echo "$arm: budget reached (\$$spent), stopping before run $n"; break
  fi
  project="$work/runs/$arm-$n"
  rm -rf "$project" && mkdir -p "$project/.claude/skills"
  cp -al "$work/template" "$project/video"
  if [ "$arm" = bulk ]; then
    cp -R "$work/pack/." "$project/.claude/skills/"
  else
    cp -R "$work/crafted/girth-studio" "$project/.claude/skills/"
  fi
  (cd "$project" && git init -q . && printf 'video/node_modules/\n' > .gitignore && GIRTH_HOME="$project/.girth-home" \
    timeout 2400 claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
      --plugin-dir "$plugin" --permission-mode acceptEdits --allowedTools Bash Skill Read Write Edit \
      --max-turns 60 --output-format json "$(cat "$here/task.md")" < /dev/null > "$here/runs/$arm-$n.json") || true
  python3 "$here/grade.py" "$project" "$here/runs/$arm-$n.grade.json" > /dev/null || true
  python3 "$here/summarize.py" --line "$here/runs" "$arm-$n"
done
