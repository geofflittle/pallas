#!/bin/sh
# usage: sync.sh <box tree name> <git rev | WORKTREE>
# writes a commit (git archive) or the current worktree (tar, no .git) to /opt/build/cv-fixes-1016/<name>
WT=/Users/geofflittle/code/pallas-wt-conway-finish
A=/opt/build/cv-fixes-1016
N=$1; R=$2
ssh musashi-aws "rm -rf $A/$N && mkdir -p $A/$N" || exit 1
if [ "$R" = WORKTREE ]; then
  COPYFILE_DISABLE=1 tar -C "$WT" --no-xattrs --exclude=.git --exclude=target -cf - . | ssh musashi-aws "tar -xf - -C $A/$N 2>/dev/null" || exit 1
  H="worktree-on-$(git -C "$WT" rev-parse HEAD)-diff-$(git -C "$WT" diff HEAD | shasum | cut -c1-12)"
else
  git -C "$WT" archive "$R" | ssh musashi-aws "tar -xf - -C $A/$N" || exit 1
  H=$(git -C "$WT" rev-parse "$R")
fi
echo "$H" | ssh musashi-aws "cat > $A/$N/HEAD_SHA"
echo "synced $N $H"
