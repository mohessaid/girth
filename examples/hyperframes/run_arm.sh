#!/usr/bin/env bash
# Usage: run_arm.sh <bulk|girth> <project-dir> <prompt-file> <out-json>
#   Both arms hide user-level settings (skills, plugins, MCP servers) and load the
#   girth plugin, so the ONLY difference is the project's .claude/skills:
#   bulk = symlinks to every skill the harness loads today; girth = crafted girth-* only.
set -euo pipefail
arm=$1 project=$2 prompt_file=$3 out=$4
repo=$(cd "$(dirname "$0")/../.." && pwd)
plugin=${GIRTH_PLUGIN_DIR:-$repo/target/plugin-dist}
case "$arm" in
  bulk|girth) ;;
  *) echo "arm must be bulk or girth" >&2; exit 2 ;;
esac
prompt=$(cat "$prompt_file")
cd "$project"
claude -p --model "${DEMO_MODEL:-claude-sonnet-5-5}" --setting-sources "project,local" \
  --plugin-dir "$plugin" --permission-mode acceptEdits --allowedTools Bash \
  --output-format json "$prompt" < /dev/null > "$out"
python3 - "$out" "$arm" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
u = d["usage"]
ctx = sum(u.get(k, 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
print(f"{sys.argv[2]}: input+cache {ctx:,} tok, output {u.get('output_tokens', 0):,} tok, "
      f"turns {d.get('num_turns')}, cost ${d.get('total_cost_usd', 0):.4f}")
PY
