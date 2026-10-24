#!/bin/sh
# Extracts each named branch of repo.git into trees/NAME and runs checks.sh on it.
# usage: [W=dir] [U=features] sh ci-branches.sh NAME:BRANCH:BLUEPRINT_DIR...
W=${W:-/opt/build/pallas-schemas-1025}
export W U
cd "$W" || exit 1
for spec in "$@"; do
  name=${spec%%:*}
  rest=${spec#*:}
  branch=${rest%%:*}
  bp=${rest#*:}
  rm -rf "trees/$name"
  mkdir -p "trees/$name"
  git --git-dir=repo.git archive "$branch" | tar -x -C "trees/$name"
  echo "== $name $branch $(git --git-dir=repo.git rev-parse --short "$branch")"
  sh "$W/checks.sh" "trees/$name" "$bp"
done
echo "CI DONE"
