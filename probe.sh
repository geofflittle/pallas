#!/bin/sh
# usage: probe.sh <arm>   copies c6, applies probe_r2.py with that arm, runs the probe tests
A=/opt/build/cv-exfix-1016
M=$1
W=pr-$M
rm -rf $A/$W && cp -a $A/c6 $A/$W
python3 $A/probe_r2.py $A/$W $M || exit 9
find $A/$W -type f -exec touch {} +
cd $A && W=$W T=pr ./run.sh cargo test -p pallas-validate --no-fail-fast --test alonzo --test babbage --test conway --test probe_triggers -- probe_ --nocapture > logs/probe-$M.log 2>&1
echo "probe $M head=$(cut -c1-8 $A/c6/HEAD_SHA) exit=$?"
grep -E '^(PROBE|DIFF|FILE|CENSUS)|^error' logs/probe-$M.log
