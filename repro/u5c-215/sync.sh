#!/bin/sh
# usage: sync.sh
# Copies the worktree (without .git), the blueprint submodule and the apparatus
# tests to the box area, and records the worktree head in HEAD_SHA.
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/u5c-215
W=$S/wt
D=/opt/build/u5c-215-2026-10-09
/usr/bin/git -C "$W" rev-parse HEAD > "$S/box/HEAD_SHA"
rsync -rlpc --delete --exclude .git --exclude target --exclude cardano-blueprint \
  --exclude pallas-utxorpc/tests/audit_blocks.rs --exclude pallas-utxorpc/tests/probe_pre_dijkstra.rs \
  "$W/" musashi-aws:$D/src/ || exit 1
rsync -rlpc --delete --exclude .git /Users/geofflittle/code/pallas/cardano-blueprint/ musashi-aws:$D/src/cardano-blueprint/ || exit 1
rsync -rlpc "$S/box/HEAD_SHA" musashi-aws:$D/src/HEAD_SHA || exit 1
rsync -rlpc "$S/box/apparatus/" musashi-aws:$D/src/pallas-utxorpc/tests/ || exit 1
rsync -rlpc "$S/box/run.sh" "$S/box/compare.py" musashi-aws:$D/ || exit 1
echo "synced $(cat "$S/box/HEAD_SHA") with working tree changes"
