#!/bin/sh
# usage: gate.sh <sha> <label> [jobs] [tree]   pallas-validate gates on one commit, one job at a time
A=/opt/build/final-fixes-1017
SHA=$1; LBL=$2; JOBS=${3:-"fmt clippy clippy-p2 test test-p2"}; TREE=${4:-w}
cd $A
mkdir -p logs
$A/export.sh "$SHA" "$TREE" >/dev/null || { echo "$LBL export failed"; exit 2; }
H=$(cut -c1-8 "$A/$TREE/HEAD_SHA")
for job in $JOBS; do
  case $job in
    fmt) T=np; set -- cargo fmt --all -- --check;;
    clippy) T=np; set -- cargo clippy -p pallas-validate --all-targets -- -D warnings;;
    clippy-p2) T=p2; set -- cargo clippy -p pallas-validate --all-targets --features phase2 -- -D warnings;;
    test) T=np; set -- cargo test -p pallas-validate;;
    test-p2) T=p2; set -- cargo test -p pallas-validate --features phase2;;
    wclippy) T=ws; set -- cargo clippy --workspace --all-targets -- -D warnings;;
    wtest) T=ws; set -- cargo test --workspace;;
  esac
  L=logs/$LBL-$job.log
  W=$TREE T=$T $A/run.sh "$@" > $L 2>&1; rc=$?
  pass=$(grep -E '^test result' $L | awk '{s+=$4} END{print s+0}')
  fail=$(grep -E '^test result' $L | awk '{s+=$6} END{print s+0}')
  comp=$(grep -cE '^ *(Compiling|Checking) ' $L)
  echo "$LBL $H $job exit=$rc passed=$pass failed=$fail crates=$comp"
done
