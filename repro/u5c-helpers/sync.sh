#!/bin/sh
# usage: sync.sh [worktree]
# Copies a worktree (without .git) and the blueprint submodule to the box area,
# and records the worktree head in HEAD_SHA. Working tree changes are copied
# too, so HEAD_SHA names the commit the tree started from.
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/u5c-helpers
W=${1:-$S/wt}
D=/opt/build/u5c-helpers-2026-10-09
/usr/bin/git -C "$W" rev-parse HEAD > "$S/box/HEAD_SHA"
rsync -rlpc --delete --exclude .git --exclude target --exclude cardano-blueprint \
  --exclude pallas-utxorpc/tests/audit_blocks.rs --exclude pallas-utxorpc/tests/probe_pre_dijkstra.rs \
  "$W/" musashi-aws:$D/src/ || exit 1
rsync -rlpc --delete --exclude .git /Users/geofflittle/code/pallas/cardano-blueprint/ musashi-aws:$D/src/cardano-blueprint/ || exit 1
rsync -rlpc "$S/box/HEAD_SHA" musashi-aws:$D/src/HEAD_SHA || exit 1
rsync -rlpc "$S/box/run.sh" musashi-aws:$D/ || exit 1
ssh musashi-aws "find $D/src -type f -exec touch {} +" || exit 1
echo "synced $(cat "$S/box/HEAD_SHA") from $W with working tree changes"
