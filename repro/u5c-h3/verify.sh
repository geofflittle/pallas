#!/bin/sh
# usage: verify.sh <label>
# Runs on the box against the synced tree: the ten job CI rehearsal, the
# workflow's unstable test job by its own command, then clippy -D warnings of
# pallas-traverse and pallas-utxorpc with and without unstable. Each step
# prints its exit code, and the script exits 1 when any step failed.
D=/opt/build/u5c-h3-2026-10-09
L=$1
FAIL=0
CI_PROBE_AREA=$D /opt/build/ci-all.sh > $D/ci-$L.txt 2>&1
C=$?; echo "ci-all exit=$C"; [ $C -eq 0 ] || FAIL=1
cat $D/ci-$L.txt
awk '/test result:/ {p+=$4; f+=$6; n++} END {print "test-run result lines", n, "passed", p, "failed", f}' $D/ci-logs/test-run.log
rm -rf $D/ci-logs-$L
cp -r $D/ci-logs $D/ci-logs-$L
$D/run.sh cargo test -p pallas-primitives -p pallas-traverse -p pallas-utxorpc --features unstable > $D/x-$L-test-unstable.log 2>&1
C=$?; echo "test-unstable (ci.yml:78) exit=$C"; [ $C -eq 0 ] || FAIL=1
awk '/test result:/ {p+=$4; f+=$6; n++} END {print "test-unstable result lines", n, "passed", p, "failed", f}' $D/x-$L-test-unstable.log
$D/run.sh cargo clippy -p pallas-traverse -p pallas-utxorpc --all-targets \
  --features pallas-traverse/unstable,pallas-utxorpc/unstable -- -D warnings > $D/x-$L-clippy-unstable.log 2>&1
C=$?; echo "clippy unstable exit=$C"; [ $C -eq 0 ] || FAIL=1
$D/run.sh cargo clippy -p pallas-traverse -p pallas-utxorpc --all-targets -- -D warnings > $D/x-$L-clippy-stable.log 2>&1
C=$?; echo "clippy stable exit=$C"; [ $C -eq 0 ] || FAIL=1
df -h /opt | tail -1
echo "verify $L exit=$FAIL"
exit $FAIL
