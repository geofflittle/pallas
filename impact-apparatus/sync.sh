#!/bin/sh
# usage: sync.sh <ref>...
# Bundles the named local refs of the pallas repo (excluding history the box already has after the first run)
# and fetches them into the box bare repo under their own names.
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/impact
R=/Users/geofflittle/code/pallas
B=root@u5c.musashi.sundae.fi
D=/opt/build/impact-2026-10-15
basis=""
[ -f $S/synced-once ] && basis="^e7ea1726 ^e6c38ac5 ^8643e0fd"
/usr/bin/git -C $R bundle create -q $S/sync.bundle "$@" $basis || exit 1
scp -q $S/sync.bundle $B:$D/sync.bundle || exit 1
specs=""
for r in "$@"; do specs="$specs +refs/heads/$r:refs/heads/$r"; done
ssh $B "cd $D/repo.git && git fetch -q $D/sync.bundle $specs && for r in $*; do echo \"box \$r \$(git rev-parse --short \$r)\"; done" || exit 1
touch $S/synced-once
