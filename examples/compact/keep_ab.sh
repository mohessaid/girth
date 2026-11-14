#!/bin/sh
# A/B: does Girth's compaction keep-list carry the user's rules through /compact?
# One interactive Claude Code session per arm, in tmux: 8 rules stated in passing, a
# large middle, /compact, then "list my rules". Arm `girth` loads the staged plugin
# (target/plugin-dist); arm `none` loads no plugin. User-level settings are hidden in both.
#   sh examples/compact/keep_ab.sh <girth|none> <outdir>
# Spends model usage (Haiku by default, a few cents per arm).
set -eu
arm=$1 out=$2
repo=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$out"; out=$(cd "$out" && pwd)
dir=$(mktemp -d)
mkdir -p "$dir/proj" && cd "$dir/proj" && git init -q
cp "$repo/src/advise.rs" "$repo/src/meter.rs" "$repo/src/keeper.rs" .
plugin=""
[ "$arm" = girth ] && plugin="--plugin-dir $repo/target/plugin-dist"
s="keep-$arm-$$"
tmux new -d -s "$s" -x 200 -y 60 "cd $dir/proj && GIRTH_HOME=$dir/home claude --model ${MODEL:-claude-haiku-5-5} --setting-sources project,local $plugin; sleep 5"
idle() { # until the pane stops showing a running turn, twice in a row
  n=0; t=0
  while [ $n -lt 2 ] && [ $t -lt 240 ]; do
    sleep 3; t=$((t + 3))
    if tmux capture-pane -p -t "$s" | grep -q "esc to interrupt"; then n=0; else n=$((n + 1)); fi
  done
}
say() { tmux send-keys -t "$s" -l "$1"; sleep 1; tmux send-keys -t "$s" Enter; idle; }
sleep 6; tmux send-keys -t "$s" Down Enter; sleep 6   # trust the folder
tmux send-keys -t "$s" Escape; sleep 1                 # any first-run dialog
say "Quick background: this is a small invoicing app. Always use pnpm, never npm, for anything JavaScript. Just say ok."
say "Our team names every new branch with the prefix feat/ and a ticket number, and you should never touch the migrations folder. Just say ok."
say "A few style things: write commit messages in the past tense, and use 2-space indentation in YAML files, always. Just say ok."
say "We prefer British spelling in user-facing text. Do not add new dependencies without asking me first. Just say ok."
say "Remember that the staging server is called kestrel-2. Just say ok."
say "Read advise.rs, meter.rs and keeper.rs in full, then describe each file in one sentence."
say "Which function in keeper.rs decides the keeper plan? One line."
say "/compact"
sleep 5; idle
say "List every rule, convention or preference I gave you in this conversation, one per line, nothing else."
tmux capture-pane -p -t "$s" -S -400 > "$out/$arm.txt"
tmux kill-session -t "$s"
rm -rf "$dir"
score=0
for k in pnpm "feat/" migrations "past tense" "2-space" British dependenc kestrel-2; do
  sed -n '/List every rule/,$p' "$out/$arm.txt" | grep -qi -- "$k" && score=$((score + 1))
done
echo "$arm: $score/8 rules recalled after /compact"
