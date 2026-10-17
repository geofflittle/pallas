#!/bin/sh
# usage: rerun.sh <sha> <label> <features> probes <list>   or   rerun.sh <sha> <label> <features> arm <arm name>
# exports the commit, applies the reviewer probes or one arm, runs the pallas-validate tests with no fail fast
A=/opt/build/final-fixes-1017
R=$A/rprobes
SHA=$1; LBL=$2; FEAT=$3; MODE=$4; WHAT=$5
cd $A
mkdir -p logs
$A/export.sh "$SHA" rr >/dev/null || { echo "$LBL export failed"; exit 2; }
if [ "$MODE" = probes ]; then
  python3 $R/apply.py $A/rr "$WHAT" $R || { echo "$LBL apply failed"; exit 2; }
  FILTER=review_probe
else
  python3 $R/arms.py "$WHAT" $A/rr || { echo "$LBL arm failed"; exit 2; }
  FILTER=
fi
case $FEAT in
  phase2) T=p2; set -- cargo test -p pallas-validate --features phase2 --no-fail-fast;;
  *) T=np; set -- cargo test -p pallas-validate --no-fail-fast;;
esac
L=logs/$LBL.log
if [ -n "$FILTER" ]; then
  W=rr T=$T $A/run.sh "$@" "$FILTER" -- --nocapture > $L 2>&1; rc=$?
else
  W=rr T=$T $A/run.sh "$@" > $L 2>&1; rc=$?
fi
echo "$LBL $(cut -c1-8 $A/rr/HEAD_SHA) $MODE $WHAT exit=$rc"
grep -E '^(PROBE|test result: FAILED|test .* FAILED$)|panicked at|^error(\[|:)' $L
