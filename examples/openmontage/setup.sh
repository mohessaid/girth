#!/bin/sh
# Usage: setup.sh <workdir>
# Prepares what both arms share:
#   <workdir>/pack      a markdown-only mirror of OpenMontage's 49 Claude Code skills, pinned.
#                       Its 93 Python and other scripts are left out, so no arm can run them.
#   <workdir>/template  an installed Remotion project (pinned versions, browser downloaded).
#                       Each run gets a hard-linked copy, so neither arm pays for npm install.
set -eu
work=$1
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$work"
if [ ! -d "$work/OpenMontage" ]; then
  git clone -q https://github.com/calesthio/OpenMontage "$work/OpenMontage"
  git -C "$work/OpenMontage" checkout -q c806645
fi
rm -rf "$work/pack" && mkdir -p "$work/pack"
(cd "$work/OpenMontage/.claude/skills" && find . -name '*.md' -exec cp --parents {} "$work/pack/" \;)
if [ ! -d "$work/template/node_modules" ]; then
  mkdir -p "$work/template"
  cp "$here/template/package.json" "$here/template/tsconfig.json" "$work/template/"
  (cd "$work/template" && npm install --no-audit --no-fund --loglevel=error && npx remotion browser ensure)
fi
echo "pack: $(ls "$work/pack" | wc -l) skills; template: $work/template"
