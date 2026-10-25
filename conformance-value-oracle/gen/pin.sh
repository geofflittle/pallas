#!/bin/sh
# Runs the comparers of two dirs over one vrun.sh output dir and says whether their logs are identical.
# Exits 1 when the logs differ and 2 when the new comparers compared no value.
# usage: ./pin.sh OLD_DIR NEW_DIR SET
set -u
OLD=$1
NEW=$2
SET=$3
sh "$OLD/recmp.sh" "$SET" > "$SET.old.log" 2>&1
sh "$NEW/recmp.sh" "$SET" > "$SET.new.log" 2>&1
values=$(grep -c '^value ' "$SET.new.log")
if [ "$values" -eq 0 ]; then
  echo "NO VALUES compared in $SET"
  exit 2
fi
if cmp -s "$SET.old.log" "$SET.new.log"; then
  echo "LOGS IDENTICAL over $values value lines"
else
  echo "LOGS DIFFER"
  exit 1
fi
