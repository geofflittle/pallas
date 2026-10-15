#!/bin/sh
# Copy a worktree to the box clone and touch it so cargo rebuilds.
# Usage: syncdir.sh <local worktree>
set -e
rsync -a --delete --exclude .git --exclude Cargo.lock --exclude target \
  -e /usr/bin/ssh "$1/" musashi-aws:/opt/build/u5c-v1/pallas/
/usr/bin/ssh musashi-aws 'find /opt/build/u5c-v1/pallas -path /opt/build/u5c-v1/pallas/.git -prune -o -type f -exec touch {} +'
echo "synced $1"
