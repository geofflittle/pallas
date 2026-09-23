#!/bin/sh
# usage: sync.sh <laptop worktree> <box area name>
# Copies the worktree to /opt/build/<area>/src without target and .git,
# keeps the box copy of the cardano-blueprint submodule, touches every file
# and writes HEAD_SHA with a dirty marker when the tree has local edits.
set -e
wt="$1"
area="$2"
test -d "$wt" || { echo "no worktree $wt"; exit 2; }
test -n "$area" || { echo "no area"; exit 2; }
head=$(/usr/bin/git -C "$wt" rev-parse HEAD)
dirty=$(/usr/bin/git -C "$wt" status --porcelain | wc -l | tr -d ' ')
rsync -a --delete --exclude target --exclude .git --exclude cardano-blueprint "$wt/" "musashi-aws:/opt/build/$area/src/"
ssh musashi-aws "find /opt/build/$area/src -type f -exec touch {} + && echo $head > /opt/build/$area/src/HEAD_SHA && echo synced $area head=$head dirty_files=$dirty && ls /opt/build/$area/src/cardano-blueprint | wc -l"
