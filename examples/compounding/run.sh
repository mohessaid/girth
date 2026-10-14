#!/usr/bin/env bash
# Usage: run.sh <bulk|jit|girth> <workdir>
# Runs the five tasks in tasks/ as five separate headless sessions in one project.
#   bulk  = every skill in ~/.claude/skills symlinked into the project (status quo)
#   jit   = girth only; crafted skills are wiped before every session (pay-per-session)
#   girth = girth only; crafted skills persist across sessions (the compounding cache)
# User-level settings, skills and plugins are hidden in every arm (--setting-sources
# project,local), and GIRTH_HOME is isolated, so the real ~/.girth is never touched.
set -euo pipefail
arm=$1 work=$2
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
case "$arm" in bulk|jit|girth) ;; *) echo "arm must be bulk, jit or girth" >&2; exit 2 ;; esac
project="$work/$arm/project"
out="$work/$arm/usage"
rm -rf "$work/$arm" && mkdir -p "$project/.claude/skills" "$out"
export GIRTH_HOME="$work/$arm/girth-home"
if [ "$arm" = bulk ]; then
  for s in "$HOME"/.claude/skills/*/; do ln -s "${s%/}" "$project/.claude/skills/"; done
else
  mkdir -p "$project/.girth"
  printf '{"roots": ["%s/.claude/skills"]}\n' "$HOME" > "$project/.girth/config.json"
fi
cd "$project"
git init -q .
for n in 1 2 3 4 5; do
  if [ "$arm" = jit ]; then rm -rf .claude/skills/girth-*; fi
  claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
    --plugin-dir "$plugin" --permission-mode acceptEdits --allowedTools Bash Skill \
    --max-turns 40 --output-format json "$(cat "$here/tasks/$n.md")" < /dev/null > "$out/$n.json" || true
  ls -d .claude/skills/girth-* 2>/dev/null > "$out/$n.crafted" || true
  python3 "$here/summarize.py" "$out/$n.json" "$arm s$n"
done
