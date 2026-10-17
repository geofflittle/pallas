#!/bin/sh
# usage: probe.sh <sha> <label> <features> <test target> <filter>   runs one filtered test with output, prints the marked lines
A=/opt/build/final-fixes-1017
SHA=$1; LBL=$2; FEAT=$3; TGT=$4; FILTER=$5
cd $A
mkdir -p logs
$A/export.sh "$SHA" pr >/dev/null || { echo "$LBL export failed"; exit 2; }
case $FEAT in
  phase2) T=p2; set -- cargo test -p pallas-validate --features phase2;;
  *) T=np; set -- cargo test -p pallas-validate;;
esac
if [ "$TGT" = lib ]; then set -- "$@" --lib; else set -- "$@" --test "$TGT"; fi
L=logs/$LBL.log
W=pr T=$T $A/run.sh "$@" "$FILTER" -- --nocapture > $L 2>&1; rc=$?
echo "$LBL $(cut -c1-8 $A/pr/HEAD_SHA) exit=$rc"
grep -E '^(F3REACH|PROBE|ARM|test result|test .* (ok|FAILED)$)|panicked at' $L
