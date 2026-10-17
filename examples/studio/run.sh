#!/usr/bin/env bash
# Usage: run.sh <bulk|girth> <workdir> <first-run> <last-run>
# Each run is a fresh project and a fresh headless session on task.md, graded by grade.py.
#   bulk  = every skill in ~/.claude/skills symlinked into the project (status quo)
#   girth = only the crafted girth-studio skill (crafted/, made once by craft.md)
# Both arms hide user-level settings, skills and plugins (--setting-sources project,local),
# load the girth plugin, and get an isolated GIRTH_HOME, so the real ~/.girth is never touched.
# Stops before a run once the arm's spend reaches STUDIO_MAX_USD (default 6).
set -euo pipefail
arm=$1 work=$2 first=$3 last=$4
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
case "$arm" in bulk|girth) ;; *) echo "arm must be bulk or girth" >&2; exit 2 ;; esac
mkdir -p "$here/runs"
for n in $(seq "$first" "$last"); do
  spent=$(python3 "$here/summarize.py" --spent "$here/runs" "$arm")
  if python3 -c "import sys; sys.exit(float('$spent') < float('${STUDIO_MAX_USD:-6}'))"; then
    echo "$arm: budget reached (\$$spent), stopping before run $n"; break
  fi
  project="$work/$arm-$n"
  rm -rf "$project" && mkdir -p "$project/.claude/skills"
  if [ "$arm" = bulk ]; then
    for s in "$HOME"/.claude/skills/*/; do ln -s "${s%/}" "$project/.claude/skills/"; done
  else
    cp -R "$here/crafted/girth-studio" "$project/.claude/skills/"
  fi
  (cd "$project" && git init -q . && GIRTH_HOME="$project/.girth-home" \
    timeout 1800 claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
      --plugin-dir "$plugin" --permission-mode acceptEdits --allowedTools Bash Skill Read Write Edit \
      --max-turns 60 --output-format json "$(cat "$here/task.md")" < /dev/null > "$here/runs/$arm-$n.json") || true
  python3 "$here/grade.py" "$project" "$here/runs/$arm-$n.grade.json" > /dev/null || true
  python3 "$here/summarize.py" --line "$here/runs" "$arm-$n"
done
