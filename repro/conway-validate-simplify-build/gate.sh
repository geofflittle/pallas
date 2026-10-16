#!/bin/sh
# gate.sh W  runs the five gate jobs against tree W in order and prints one verdict line per job
A=/opt/build/cvsimplify-build-1016
export W=$1
cat $A/$W/HEAD_SHA
F=0
$A/job.sh $W-fmt cargo fmt --all --check || F=1
$A/job.sh $W-clippy cargo clippy -p pallas-validate --all-targets -- -D warnings || F=1
$A/job.sh $W-clippy-phase2 cargo clippy -p pallas-validate --all-targets --features phase2 -- -D warnings || F=1
$A/job.sh $W-test cargo test -p pallas-validate || F=1
$A/job.sh $W-test-phase2 cargo test -p pallas-validate --features phase2 || F=1
echo "gate $W failed=$F"
