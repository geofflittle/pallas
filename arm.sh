#!/bin/sh
# usage: arm.sh <base tree> <mutant name or arms/X.patch> [test filter]
# copies the base tree to arm, applies the mutant with mut.py or patch, runs the phase2 tests
A=/opt/build/cv-exfix-1016
B=$1; M=$2; F=${3:-}
test -s "$A/$B/HEAD_SHA" || { echo "arm $M refused: $B has no HEAD_SHA"; exit 2; }
rsync -a --delete "$A/$B/" "$A/arm/"
case "$M" in
  *.patch) (cd $A/arm && patch -p1 -s < $A/$M) || { echo "arm $M patch-failed"; exit 9; } ;;
  *) python3 $A/mut.py "$M" "$A/arm" || { echo "arm $M mutate-failed"; exit 9; } ;;
esac
find "$A/arm" -type f -exec touch {} +
L="$A/logs/arm-$B-$(basename $M .patch).log"
cd $A && W=arm T=p2 ./run.sh cargo test -p pallas-validate --features phase2 --no-fail-fast $F > "$L" 2>&1
C=$?
echo "arm $M base=$B head=$(cat $A/$B/HEAD_SHA | cut -c1-8) exit=$C"
grep -E "^test .* FAILED|^error(\[|:)" "$L" | head -8
grep -E "^test result" "$L" | awk "{p+=\$4; f+=\$6} END {print \"totals passed=\" p \" failed=\" f}"
