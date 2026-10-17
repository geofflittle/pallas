#!/bin/sh
# usage: gate.sh <tree>   runs fmt, clippy twice, test twice and prints one line per gate
A=/opt/build/cv-exfix-1016
W=$1
cd $A
test -s $A/$W/HEAD_SHA || { echo "$W refused: no HEAD_SHA"; exit 2; }
echo "$W head $(cat $A/$W/HEAD_SHA)"
find $A/$W -type f -exec touch {} +
mkdir -p logs
r() { name=$1; T=$2; shift 2; W=$W T=$T ./run.sh "$@" > logs/$W-$name.log 2>&1; rc=$?; tot=$(grep -E "^test result" logs/$W-$name.log | awk "{p+=\$4; f+=\$6} END {print \"passed=\" p \" failed=\" f}"); comp=$(grep -cE "^ *(Compiling|Checking) pallas-validate" logs/$W-$name.log); echo "$W $name exit=$rc $tot validate-built=$comp"; }
r fmt np cargo fmt --all -- --check
r clippy np cargo clippy -p pallas-validate --all-targets -- -D warnings
r clippy-p2 p2 cargo clippy -p pallas-validate --all-targets --features phase2 -- -D warnings
r test np cargo test -p pallas-validate
r test-p2 p2 cargo test -p pallas-validate --features phase2
