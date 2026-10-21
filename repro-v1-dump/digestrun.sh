#!/bin/sh
# usage: digestrun.sh <name> <ref>  writes dumpchk/<name>/digest.txt, the v1alpha and v1beta block digests of <ref> over the chain dump
set -e
D=/opt/build/v1-reshape-1022
C=/opt/build/mapper-panics-1022/dump/chain.b64
N=$1; R=$2
W=$D/dumpchk/$N
if [ -d $W/pallas ]; then git -C $D/repo worktree remove --force $W/pallas; fi
mkdir -p $W
git -C $D/repo worktree add -q --detach $W/pallas "$R"
rm -rf $W/digest; mkdir -p $W/digest
sed -e 's/name = "rows-full"/name = "digest-before-v1"/' -e 's/path = "rows-full.rs"/path = "digest-before-v1.rs"/' $D/rows-full/Cargo.toml > $W/digest/Cargo.toml
cp $D/rows-full/digest-before-v1.rs $W/digest/
echo "$N pallas at $(git -C $W/pallas rev-parse HEAD)"
find $W -type f -not -path "*/.git/*" -exec touch {} +
docker run --rm -v $W:/work -v $C:/chain.b64:ro -v $D/target-dump-$N:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -w /work/digest pallas-ci:1 sh -c 'cargo build --release -q && /target/release/digest-before-v1 /chain.b64 > /work/digest.txt'
echo "$N blocks $(wc -l < $W/digest.txt)"
