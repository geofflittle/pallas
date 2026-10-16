#!/bin/sh
# sync.sh REV W  exports commit REV of the pallas repository to the box as $A/W (with the blueprint submodule), writes its head and touches it
REV=$1; W=$2
A=/opt/build/v3dep-1016
R=${R:-/Users/geofflittle/code/pallas}
H=$(/usr/bin/git -C "$R" rev-parse "$REV") || exit 2
T=$(mktemp -d)
/usr/bin/git -C "$R" archive "$H" | tar -x -C "$T"
rsync -a --delete "$T/" root@u5c.musashi.sundae.fi:$A/$W/
rsync -a --delete --exclude .git "/Users/geofflittle/code/pallas/cardano-blueprint/" root@u5c.musashi.sundae.fi:$A/$W/cardano-blueprint/
rm -rf "$T"
ssh root@u5c.musashi.sundae.fi "echo $H > $A/$W/HEAD_SHA && find $A/$W -exec touch {} + && echo synced $W at \$(cat $A/$W/HEAD_SHA)"
