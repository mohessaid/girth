#!/bin/sh
# The README demo: real girth commands on public skills, no model calls.
# Record it with `make demo` (asciinema + agg). Runs in a throwaway dir with
# its own GIRTH_HOME, so it never touches ~/.girth.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
girth="$repo/target/release/girth"
work=${DEMO_DIR:-$(mktemp -d)}
export GIRTH_HOME="$work/.girth"
mkdir -p "$work/.claude/skills"
cd "$work"
girth() { "$girth" "$@"; }

say() { printf '\n\033[1;33m# %s\033[0m\n' "$*"; sleep "${PAUSE:-1}"; }
run() {
  printf '\033[1;32m$\033[0m '
  printf '%s\n' "$*" | fold -w1 | while IFS= read -r c; do printf '%s' "${c:- }"; sleep 0.02; done
  printf '\n'
  eval "$*"
  sleep "${PAUSE:-1}"
}

say "A public skill repo: 21 HyperFrames skills"
run 'girth fetch heygen-com/hyperframes >/dev/null'
ln -s .girth/fetch/heygen-com__hyperframes/skills hf
run 'girth measure --roots hf | tail -1'
say "You need one thing from it. Find the slice, not the skill:"
run 'girth search "word-by-word caption highlighting" --roots hf | sed -n 2,7p | cut -c1-48'
run 'girth show hf/hyperframes-studio --heading "2. One caption track" | head -12'
say "The model distils that slice into a project skill, once (this one is from a real session):"
cp -r "$repo/examples/hyperframes/crafted-girth-hyperframes" .claude/skills/girth-hyperframes
run 'girth validate .claude/skills/girth-hyperframes'
run 'girth measure --roots .claude/skills | tail -1'
say "Taste: house rules and what you made last time, per domain"
run 'girth taste add launch-video "End on the logo with the URL."'
run 'girth made launch-video palette=neon-on-black music=synthwave'
run 'girth taste show launch-video'
say "Measured: 5 sessions, -34% cost at equal quality (examples/compounding)"
sleep 2
