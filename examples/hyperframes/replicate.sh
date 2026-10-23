#!/usr/bin/env bash
# Usage: replicate.sh <workdir> <first-run> <last-run>
# Re-runs the 2026-10-14 side-by-side with N runs per arm, alternating bulk and girth one run at a
# time (time-of-day drift hits both arms; never two browsers at once). Each run is a fresh project:
#   bulk  = every skill in ~/.claude/skills symlinked into .claude/skills
#   girth = only the crafted skill in crafted-girth-hyperframes/
# Same claude -p flags as run_arm.sh, plus an isolated GIRTH_HOME. Usage is read from modelUsage
# by ../studio/summarize.py. Stops once total spend reaches HF_MAX_USD (default 5).
set -euo pipefail
work=$1 first=$2 last=$3
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
runs="$here/runs"; mkdir -p "$runs"
for n in $(seq "$first" "$last"); do
  for arm in bulk girth; do
    b=$(python3 "$here/../studio/summarize.py" --spent "$runs" bulk)
    g=$(python3 "$here/../studio/summarize.py" --spent "$runs" girth)
    if python3 -c "import sys; sys.exit($b + $g < ${HF_MAX_USD:-5})"; then echo "budget reached"; exit 0; fi
    project="$work/$arm-$n"
    rm -rf "$project" && mkdir -p "$project/.claude/skills"
    if [ "$arm" = bulk ]; then
      for s in "$HOME"/.claude/skills/*/; do ln -s "${s%/}" "$project/.claude/skills/"; done
    else
      cp -R "$here/crafted-girth-hyperframes" "$project/.claude/skills/girth-hyperframes"
    fi
    (cd "$project" && git init -q . && GIRTH_HOME="$project/.girth-home" \
      timeout 1800 claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
        --plugin-dir "$plugin" --permission-mode acceptEdits --allowedTools Bash \
        --output-format json "$(cat "$here/task.md")" < /dev/null > "$runs/$arm-$n.json") || true
    python3 "$here/grade.py" "$project" "$runs/$arm-$n.grade.json" > /dev/null || true
    python3 -c "import json,sys;g=json.load(open(sys.argv[1]));print(sys.argv[2],g)" "$runs/$arm-$n.grade.json" "$arm-$n"
  done
done
