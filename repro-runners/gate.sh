#!/bin/sh
# gate.sh W [FEATURES...]  runs fmt, then clippy and test for the default feature set and each named feature set, against tree W
A=/opt/build/cvmain-1016
export W=$1; shift
cat $A/$W/HEAD_SHA
F=0
$A/job.sh $W-fmt cargo fmt --all --check || F=1
$A/job.sh $W-clippy cargo clippy -p pallas-validate --all-targets -- -D warnings || F=1
$A/job.sh $W-test cargo test -p pallas-validate || F=1
for f in "$@"; do
  $A/job.sh $W-clippy-$f cargo clippy -p pallas-validate --all-targets --features $f -- -D warnings || F=1
  $A/job.sh $W-test-$f cargo test -p pallas-validate --features $f || F=1
done
echo "gate $W features=default,$* failed=$F"
