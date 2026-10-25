#!/bin/sh
# Compares a walk prefix unplanted and under each e2e plant, one output file per arm.
# usage: e2e-plants.sh WALK_DIR PREFIX_DIR N
# Expected: base exits 0, slim, bls and eb each exit 1 with their DISAGREE line.
set -u
G=$(dirname "$0")
SRC=$1
D=$2
N=$3
export SERVED_DIR=${SERVED_DIR:-$G/e2e}
python3 -I "$G/cut_prefix.py" "$SRC" "$D" "$N" > "$D.cut.txt" 2>&1 || exit 1
for p in base slim bls eb; do
  if [ "$p" = base ]; then
    python3 -I "$G/e2e_values.py" compare "$D" > "$D.$p.txt" 2>&1
  else
    PLANT=$p python3 -I "$G/e2e_values.py" compare "$D" > "$D.$p.txt" 2>&1
  fi
  echo "EXIT $?" >> "$D.$p.txt"
done
