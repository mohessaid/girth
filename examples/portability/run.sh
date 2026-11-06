#!/bin/sh
# Does each installed harness load a Girth-crafted skill from .agents/skills, and can
# it run girth through the shipped skill? One read-only headless prompt per harness,
# run one at a time. Spends each harness's own provider credits, not the Claude plan.
#   sh examples/portability/run.sh [codex copilot opencode agy cursor-agent gemini]
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
work=${TMPDIR:-/tmp}/girth-portability
out="$here/runs/$(date -u +%F)"
rm -rf "$work" && mkdir -p "$work/.agents/skills" "$out"
cp -R "$repo/examples/hyperframes/crafted-girth-hyperframes" "$work/.agents/skills/girth-hyperframes"
cp -R "$repo/skills/girth" "$work/.agents/skills/girth"
mkdir -p "$work/.agents/skills/girth/bin" && cp "$repo/target/release/girth" "$work/.agents/skills/girth/bin/girth"
(cd "$work" && git init -q)

ask='List the names of every agent skill available to you whose name starts with "girth". Then use the girth skill to run its search command for the need "video captions" and print the line that starts with output_dir. Do not modify any files.'

run() { # name, command...
  name=$1; shift
  command -v "$1" >/dev/null 2>&1 || { echo "$name: not installed" | tee "$out/$name.txt"; return; }
  echo "== $name"
  # A harness started from inside Claude Code inherits CLAUDECODE=1; a normal shell has none.
  (cd "$work" && env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT timeout 300 "$@" "$ask") > "$out/$name.txt" 2>&1
  echo "exit $?" >> "$out/$name.txt"
  grep -o 'girth-hyperframes\|output_dir: [^ ]*' "$out/$name.txt" | sort -u
}

for h in ${*:-codex copilot opencode agy cursor-agent gemini}; do
  case $h in
    codex) run codex codex exec --sandbox read-only --skip-git-repo-check ;;
    copilot) run copilot copilot --allow-all-tools -p ;;
    opencode) run opencode opencode run ;;
    agy) run agy agy --dangerously-skip-permissions -p ;; # throwaway dir; headless agy cannot ask
    cursor-agent) run cursor-agent cursor-agent -p ;;
    gemini) GEMINI_CLI_TRUST_WORKSPACE=true run gemini gemini -p ;; # the throwaway dir is untrusted
  esac
done
