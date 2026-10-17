#!/bin/sh
# sync.sh W [SRC]  copies worktree SRC (default the conway finish worktree, uncommitted edits included) to the box as $A/W, writes its head and touches it
W=${1:?W names the tree}
SRC=${2:-/Users/geofflittle/code/pallas-wt-conway-finish}
A=/opt/build/conway-finish-1016
H=$(/usr/bin/git -C "$SRC" rev-parse HEAD) || exit 2
ssh musashi-aws "mkdir -p $A/$W"
rsync -a --delete --exclude .git --exclude target --exclude cardano-blueprint "$SRC/" musashi-aws:$A/$W/
rsync -a --delete --exclude .git "/Users/geofflittle/code/pallas/cardano-blueprint/" musashi-aws:$A/$W/cardano-blueprint/
ssh musashi-aws "echo $H > $A/$W/HEAD_SHA && find $A/$W -exec touch {} + && echo synced $W at \$(cat $A/$W/HEAD_SHA) dirty=$(/usr/bin/git -C "$SRC" status --porcelain | wc -l | tr -d ' ')"
