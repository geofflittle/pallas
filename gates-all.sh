#!/bin/sh
A=/opt/build/cv-exfix-1016
cd $A
for t in c1 c2 c3 c4 c5 c6; do ./gate.sh $t; done
W=c6 T=ws ./run.sh cargo clippy --workspace --all-targets -- -D warnings > logs/c6-ws-clippy.log 2>&1; echo "c6 ws-clippy exit=$?"
W=c6 T=ws ./run.sh cargo test --workspace > logs/c6-ws-test.log 2>&1; rc=$?; echo "c6 ws-test exit=$rc $(grep -E '^test result' logs/c6-ws-test.log | awk '{p+=$4; f+=$6} END {print "passed=" p " failed=" f}')"
echo GATES-DONE
