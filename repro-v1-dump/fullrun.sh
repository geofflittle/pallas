#!/bin/sh
# usage: fullrun.sh <name> <slot>  writes dumpchk/<name>/full-<slot>.json, the whole mapped block at <slot>, from the tree dumprun-full.sh made
set -e
D=/opt/build/v1-reshape-1022
C=/opt/build/mapper-panics-1022/dump/chain.b64
N=$1; S=$2
W=$D/dumpchk/$N
cp $D/rows-full/rows-full.rs $W/rows-full/
touch $W/rows-full/rows-full.rs
docker run --rm -v $W:/work -v $C:/chain.b64:ro -v $D/target-dump-$N:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -e FULL_SLOT=$S -w /work/rows-full pallas-ci:1 sh -c "cargo build --release -q && /target/release/rows-full /chain.b64 > /work/full-$S.json"
echo "$N slot $S lines $(wc -l < $W/full-$S.json)"
