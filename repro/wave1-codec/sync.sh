#!/bin/sh
# Sync the codec worktree and the blueprint checkout to the box, then touch.
set -e
WT=/Users/geofflittle/code/pallas-wt-codec-util
ssh musashi-aws 'mkdir -p /opt/build/wave1-codec/src/cardano-blueprint'
rsync -a --delete --exclude target --exclude .git --exclude cardano-blueprint "$WT/" musashi-aws:/opt/build/wave1-codec/src/
rsync -a --delete --exclude .git /Users/geofflittle/code/pallas/cardano-blueprint/ musashi-aws:/opt/build/wave1-codec/src/cardano-blueprint/
/usr/bin/git -C "$WT" rev-parse HEAD | ssh musashi-aws 'cat > /opt/build/wave1-codec/src/HEAD_SHA'
ssh musashi-aws 'find /opt/build/wave1-codec/src -type f -exec touch {} + && echo touched $(find /opt/build/wave1-codec/src -type f | wc -l)'
