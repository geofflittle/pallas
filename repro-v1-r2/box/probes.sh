#!/bin/sh
# usage: probes.sh <tree> <unstable|none> <files...>  copies the probe files into src-<tree>/pallas-utxorpc/tests, runs them, removes them
# expected for v1_probe.rs: base with unstable "error[E0433]", head with unstable "2 passed", head with none "error[E0433]"
D=/opt/build/v1-r2-1022
N=$1; FE=$2; shift 2
F=""; if [ "$FE" = unstable ]; then F="--features unstable"; fi
mkdir -p $D/src-$N/pallas-utxorpc/tests
for f in "$@"; do cp $D/arms/$f $D/src-$N/pallas-utxorpc/tests/; done
T=""; for f in "$@"; do T="$T --test ${f%.rs}"; done
echo "PROBES on src-$N at $(git -C $D/src-$N rev-parse --short HEAD) features=$FE: $*"
$D/run.sh $N cargo test -p pallas-utxorpc $F $T -- --nocapture > $D/logs/probe-$N-$FE.log 2>&1
echo "exit=$?"
grep -E '^error(\[E[0-9]+\])?:|^test result|^test .* (ok|FAILED)$' $D/logs/probe-$N-$FE.log | head -20
rm -rf $D/src-$N/pallas-utxorpc/tests
