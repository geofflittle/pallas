#!/bin/sh
# sync.sh <rev>: puts the tree of <rev> from the pallas-wt-eb worktree into the
# box's /opt/build/pallas-eb/src, keeping the box's cardano-blueprint copy, and
# touches every file so cargo rebuilds from it.
set -e
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/eb-fix
rm -rf "$S/tree"
mkdir -p "$S/tree"
/usr/bin/git -C /Users/geofflittle/code/pallas-wt-eb archive "$1" | tar -x -C "$S/tree"
rsync -a --delete --exclude cardano-blueprint --exclude target --exclude Cargo.lock "$S/tree/" musashi-aws:/opt/build/pallas-eb/src/
ssh musashi-aws 'cd /opt/build/pallas-eb/src && find . -exec touch {} + && echo synced $(wc -l < pallas-traverse/src/leios.rs) leios lines'
echo "rev $(/usr/bin/git -C /Users/geofflittle/code/pallas-wt-eb rev-parse --short "$1")"
