#!/bin/sh
# sync.sh SRC_TREE W  copies a pallas worktree (without .git and target) to the box as $A/W, writes its head and touches it
SRC=$1; W=$2
A=/opt/build/cvmain-1016
H=$(/usr/bin/git -C "$SRC" rev-parse HEAD)
test -n "$(/usr/bin/git -C "$SRC" status --porcelain)" && echo "dirty tree $SRC" && exit 2
rsync -a --delete --exclude .git --exclude target --exclude cardano-blueprint "$SRC/" root@u5c.musashi.sundae.fi:$A/$W/
rsync -a --delete --exclude .git /Users/geofflittle/code/pallas/cardano-blueprint/ root@u5c.musashi.sundae.fi:$A/$W/cardano-blueprint/
ssh root@u5c.musashi.sundae.fi "echo $H > $A/$W/HEAD_SHA && find $A/$W -exec touch {} + && echo synced $W at \$(cat $A/$W/HEAD_SHA)"
