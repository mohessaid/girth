#!/bin/sh
# Lens A/B: the same task with Lens off and on, in your real Claude Code setup.
#   examples/lens/ab.sh <off|on> <outdir> [runs]
# Needs `make dist`. Uses your real skills and plugins, a COPY of your ledger in
# an isolated GIRTH_HOME, and turns the installed girth plugin off for the run
# so only the dist copy (with its mod) loads. Your ~/.girth is never written.
set -eu
arm=$1 out=$2 runs=${3:-3}
repo=$(cd "$(dirname "$0")/../.." && pwd)
dist="$repo/target/plugin-dist"
task=$(cat "$repo/examples/lens/task.md")
mkdir -p "$out"
for i in $(seq 1 "$runs"); do
  work=$(mktemp -d)
  home="$out/$arm-$i.girth"
  mkdir -p "$home"
  cp "${REAL_GIRTH_HOME:-$HOME/.girth}/ledger.jsonl" "$home/" 2>/dev/null || true
  lens=on
  [ "$arm" = off ] && lens=off
  (cd "$work" && GIRTH_HOME="$home" GIRTH_LENS=$lens claude -p "$task" \
    --plugin-dir "$dist" \
    --settings '{"enabledPlugins":{"girth@girth":false}}' \
    --model "${MODEL:-claude-sonnet-5-5}" \
    --max-budget-usd "${RUN_MAX_USD:-1}" \
    --allowedTools "Write Edit Read Skill Bash(*/skills/girth/scripts/girth *)" \
    --output-format stream-json --verbose > "$out/$arm-$i.jsonl" 2>"$out/$arm-$i.err" || true)
  cp "$work/index.html" "$out/$arm-$i.html" 2>/dev/null || true
  rm -rf "$work"
done
