#!/bin/sh
# sync.sh SRC_TREE [W]  copies a pallas tree (without .git and target) to the box as $A/$W and touches it
SRC=$1; W=${2:-w}
A=/opt/build/cvfix-1015
rsync -a --delete --exclude .git --exclude target --exclude cardano-blueprint "$SRC/" root@u5c.musashi.sundae.fi:$A/$W/
rsync -a --delete /Users/geofflittle/code/pallas/cardano-blueprint/ root@u5c.musashi.sundae.fi:$A/$W/cardano-blueprint/ --exclude .git
ssh root@u5c.musashi.sundae.fi "find $A/$W -exec touch {} +"
