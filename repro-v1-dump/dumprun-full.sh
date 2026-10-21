#!/bin/sh
# usage: dumprun-full.sh <name> <ref>  writes dumpchk/<name>/rows.txt, the rows-full output of <ref> over the chain dump
set -e
D=/opt/build/v1-reshape-1022
C=/opt/build/mapper-panics-1022/dump/chain.b64
N=$1; R=$2
W=$D/dumpchk/$N
if [ -d $W/pallas ]; then git -C $D/repo worktree remove --force $W/pallas; fi
mkdir -p $W
git -C $D/repo worktree add -q --detach $W/pallas "$R"
rm -rf $W/rows-full; mkdir -p $W/rows-full; cp $D/rows-full/Cargo.toml $D/rows-full/rows-full.rs $W/rows-full/
echo "$N pallas at $(git -C $W/pallas rev-parse HEAD)"
find $W -type f -not -path "*/.git/*" -exec touch {} +
docker run --rm -v $W:/work -v $C:/chain.b64:ro -v $D/target-dump-$N:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -w /work/rows-full pallas-ci:1 sh -c 'cargo build --release -q && /target/release/rows-full /chain.b64 > /work/rows.txt'
echo "$N lines $(wc -l < $W/rows.txt) blocks $(grep -c '^B ' $W/rows.txt) subs $(grep -c '^{"sub"' $W/rows.txt)"
