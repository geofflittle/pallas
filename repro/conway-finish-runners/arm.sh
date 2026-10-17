#!/bin/sh
# arm.sh SRC PATCH FILTER...  copies synced tree $A/SRC to $A/arm, applies arms/PATCH (none for the unpatched control), runs the phase2 validate tests matching FILTER, prints one verdict line and the failing tests
A=/opt/build/conway-finish-1016
SRC=$1; P=$2; shift 2
rsync -a --delete "$A/$SRC/" "$A/arm/"
if [ "$P" != none ]; then
  (cd "$A/arm" && patch -s -p1 < "$A/arms/$P") || { echo "arm $P on $SRC patch-failed"; exit 9; }
fi
find "$A/arm/pallas-validate" -exec touch {} +
cd "$A" && W=arm JOBNAME=arm ./run.sh cargo test -p pallas-validate --features phase2 -- "$@" > "$A/logs/arm-$SRC-$P.log" 2>&1
C=$?
echo "arm $P on $SRC exit=$C compiled=$(/bin/grep -cE '^ +Compiling ' "$A/logs/arm-$SRC-$P.log")"
/bin/grep -E '^---- |^test .* FAILED|^error(\[|:)' "$A/logs/arm-$SRC-$P.log" | head -20
/bin/grep -E '^test result' "$A/logs/arm-$SRC-$P.log" | awk '{p+=$4; f+=$6} END {print "totals passed=" p " failed=" f}'
exit $C
