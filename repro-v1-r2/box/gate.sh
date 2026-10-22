#!/bin/sh
# usage: gate.sh <tree> <arm>  runs gate-probe/<arm>, a crate outside the workspace with no patch, against src-<tree>/pallas-utxorpc by path
# expected on the gated head: stable prints "GATE stable" and locks utxorpc-spec 0.19.2 from crates.io, unstable fails with "failed to select a version"
D=/opt/build/v1-r2-1022
T=$1; A=$2
W=$D/gate-probe/$A
rm -rf $W/target $W/Cargo.lock
echo "GATE PROBE $A on src-$T at $(git -C $D/src-$T rev-parse --short HEAD)"
docker run --rm -v $W:/w -v $D/src-$T:/pallas:ro -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/w/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_TERM_COLOR=never -w /w pallas-ci:1 cargo run -q > $D/logs/gate-$T-$A.log 2>&1
echo "exit=$?"
grep -E '^error|failed to select|does not have|^GATE|feature' $D/logs/gate-$T-$A.log | head -8
if [ -f $W/Cargo.lock ]; then grep -A2 'name = "utxorpc-spec"' $W/Cargo.lock; fi
rm -rf $W/target
