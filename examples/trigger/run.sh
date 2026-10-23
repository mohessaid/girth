#!/usr/bin/env bash
# Usage: run.sh <workdir> <repeats>
# Trigger rate: does Girth fire by itself? Each of the five compounding tasks (which never mention
# Girth) runs as the FIRST session of a fresh project: no project skills, Girth available, user
# settings hidden, isolated GIRTH_HOME, read access to ~/.claude/skills. One run at a time.
# Claude Code protects .claude/ even in acceptEdits; headless crafting into .claude/skills needs
# MODE=bypassPermissions (the runs already allow unrestricted Bash in a throwaway project).
# A run "fires" if the session invoked the girth skill; "crafted" if a girth-* skill exists after.
set -euo pipefail
work=$1 repeats=$2
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
mkdir -p "$here/${OUT:-runs}"
for r in $(seq "${FIRST:-1}" "$repeats"); do
  for t in ${TASKS:-1 2 3 4 5}; do
    project="$work/t$t-r$r"
    rm -rf "$project" && mkdir -p "$project/.claude/skills" "$project/.girth"
    printf '{"roots": ["%s/.claude/skills"]}\n' "$HOME" > "$project/.girth/config.json"
    (cd "$project" && git init -q . && GIRTH_HOME="$project/.girth-home" \
      timeout 1800 claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
        --plugin-dir "$plugin" --add-dir "$HOME/.claude/skills" --permission-mode "${MODE:-acceptEdits}" \
        --allowedTools Bash Skill Read --max-turns 40 --output-format stream-json --verbose \
        "$(cat "$here/../compounding/tasks/$t.md")" < /dev/null > "$here/${OUT:-runs}/t$t-r$r.jsonl") || true
    fired=$(grep -c '"skill":"girth:girth"' "$here/${OUT:-runs}/t$t-r$r.jsonl" || true)
    crafted=$(find "$project/.claude/skills" -maxdepth 1 -name "girth-*" | wc -l)
    echo "t$t r$r fired=$fired crafted=$crafted"
  done
done
