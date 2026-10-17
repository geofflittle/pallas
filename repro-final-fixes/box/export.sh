#!/bin/sh
# usage: export.sh <sha> <tree dir name>   exports a commit without .git, writes HEAD_SHA, touches every file
A=/opt/build/final-fixes-1017
set -e
rm -rf "$A/$2"; mkdir -p "$A/$2"
git -C $A/repo archive "$1" | tar -x -C "$A/$2"
git -C $A/repo rev-parse "$1" > "$A/$2/HEAD_SHA"
find "$A/$2" -type f -exec touch {} +
echo "exported $(cat $A/$2/HEAD_SHA) to $2"
