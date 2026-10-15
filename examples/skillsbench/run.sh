#!/bin/sh
# SkillsBench proof: each task in tasks.txt, three arms, with BenchFlow's
# claude-agent-acp in local Docker.
#   none     --skill-mode no-skill
#   curated  the task's own environment/skills
#   girth    only the girth-<domain> skill crafted from them (crafted/<task>)
#
#   examples/skillsbench/run.sh <skillsbench checkout> <outdir> [arms...]
# TASKS="a\nb" runs a subset (one task per line) instead of tasks.txt.
#
# Credentials: export ANTHROPIC_API_KEY. (BenchFlow can instead copy your
# Claude Code login into each container; that is your call, not this script's.)
set -eu
# BenchFlow silently falls back to copying ~/.claude/.credentials.json into
# each task container. Require an explicit choice instead.
if [ -z "${ANTHROPIC_API_KEY:-}" ] && [ "${SKILLSBENCH_USE_CLAUDE_LOGIN:-}" != 1 ]; then
  echo "run.sh: export ANTHROPIC_API_KEY, or set SKILLSBENCH_USE_CLAUDE_LOGIN=1 to let BenchFlow copy your Claude Code login into the task containers." >&2
  exit 2
fi
sb=$(cd "$1" && pwd) out=$2
shift 2
arms=${*:-none curated girth}
here=$(cd "$(dirname "$0")" && pwd)
bench="$sb/.venv/bin/bench"
mkdir -p "$out"
out=$(cd "$out" && pwd)
while read -r task; do
  [ -n "$task" ] || continue
  for arm in $arms; do
    case $arm in
      none) skills="--skill-mode no-skill" ;;
      curated) skills="--skill-mode with-skill --skills-dir $sb/tasks/$task/environment/skills" ;;
      girth) skills="--skill-mode with-skill --skills-dir $here/crafted/$task" ;;
      *) echo "unknown arm $arm" >&2; exit 2 ;;
    esac
    # shellcheck disable=SC2086
    (cd "$sb" && "$bench" eval run --tasks-dir "tasks/$task" --agent claude-agent-acp \
      --model "${MODEL:-claude-sonnet-5-5}" --sandbox docker $skills \
      --jobs-dir "$out/$arm/$task" > "$out/$arm-$task.log" 2>&1 || true)
    echo "$arm $task: $(tail -n 4 "$out/$arm-$task.log" | grep -m1 'Score' || echo 'see log')"
  done
done <<EOF
${TASKS:-$(cat "$here/tasks.txt")}
EOF
