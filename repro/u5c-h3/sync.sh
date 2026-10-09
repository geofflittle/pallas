#!/bin/sh
# usage: sync.sh <worktree>
# Copies a worktree (without .git) and the blueprint submodule to the box area
# src, records the worktree head in HEAD_SHA, and touches every file so cargo
# rebuilds. Working tree changes are copied too.
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/u5c-h3
W=$1
D=/opt/build/u5c-h3-2026-10-09
[ -d "$W" ] || { echo "no worktree $W"; exit 1; }
/usr/bin/git -C "$W" rev-parse HEAD > "$S/box/HEAD_SHA" || exit 1
rsync -rlpc --delete --exclude .git --exclude target --exclude cardano-blueprint \
  "$W/" musashi-aws:$D/src/ || exit 1
rsync -rlpc --delete --exclude .git /Users/geofflittle/code/pallas/cardano-blueprint/ musashi-aws:$D/src/cardano-blueprint/ || exit 1
rsync -rlpc "$S/box/HEAD_SHA" musashi-aws:$D/src/HEAD_SHA || exit 1
rsync -rlpc "$S/box/run.sh" "$S/box/verify.sh" musashi-aws:$D/ || exit 1
ssh musashi-aws "find $D/src -type f -exec touch {} +" || exit 1
echo "synced $(cat "$S/box/HEAD_SHA") from $W"
echo "status: $(/usr/bin/git -C "$W" status --short | wc -l) changed paths"
