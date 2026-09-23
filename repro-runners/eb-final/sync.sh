#!/bin/sh
# Copy a pallas tree to a box source dir and touch every file there.
# Usage: sync.sh <local tree> <box dir>
set -eu
src="$1"
dst="$2"
rsync -a --delete --exclude target --exclude .git "$src"/ musashi-aws:"$dst"/
ssh musashi-aws "find $dst -type f -exec touch {} + && df -h / | tail -1"
