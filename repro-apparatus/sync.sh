#!/bin/sh
# Copy the rewrite worktree to the box clone and touch it so cargo rebuilds.
set -e
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad
rsync -a --delete --exclude .git --exclude Cargo.lock --exclude target \
  -e /usr/bin/ssh "$S/wt-rewrite/" musashi-aws:/opt/build/u5c-v1/pallas/
/usr/bin/ssh musashi-aws 'find /opt/build/u5c-v1/pallas -path /opt/build/u5c-v1/pallas/.git -prune -o -type f -exec touch {} +'
echo synced
