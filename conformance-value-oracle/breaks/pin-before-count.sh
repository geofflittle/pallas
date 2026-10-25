#!/bin/sh
# Runs the comparers of two dirs over one vrun.sh output dir and says whether their logs are identical.
# usage: ./pin.sh OLD_DIR NEW_DIR SET
set -u
OLD=$1
NEW=$2
SET=$3
sh "$OLD/recmp.sh" "$SET" > "$SET.old.log" 2>&1
sh "$NEW/recmp.sh" "$SET" > "$SET.new.log" 2>&1
if cmp -s "$SET.old.log" "$SET.new.log"; then
  echo "LOGS IDENTICAL $(grep -c . "$SET.new.log") lines"
else
  echo "LOGS DIFFER"
  exit 1
fi
