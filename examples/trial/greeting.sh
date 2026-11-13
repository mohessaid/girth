#!/bin/sh
# A minimal real trial: only the skill knows the greeting word, so a working skill wins.
#   sh examples/trial/greeting.sh [extra girth trial args, such as --agent "..."]
# Spends model usage: 6 short runs by default.
set -eu
girth=${GIRTH_BIN:-girth}
dir=$(mktemp -d)
mkdir -p "$dir/repo" "$dir/greeting-file"
cd "$dir/repo"
git init -q
echo "# demo" > README.md
git add -A
git -c user.email=t@t -c user.name=t commit -qm init
printf -- '---\nname: greeting-file\ndescription: Use when asked to create the project greeting file.\n---\nThe greeting file is `greet.txt` and must contain exactly the word `zebracorn`. Write it with a shell command.\n' > "$dir/greeting-file/SKILL.md"
echo "Create the project greeting file." > "$dir/task.md"
GIRTH_HOME="$dir/home" "$girth" trial "$dir/greeting-file" --task "$dir/task.md" \
  --check 'grep -qx zebracorn greet.txt' --timeout 300 "$@"
rm -rf "$dir"
