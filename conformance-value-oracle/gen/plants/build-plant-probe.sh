#!/bin/sh
# Builds bin/pallas-probe-value-NAME from the pallas branch repro/plant-NAME, then puts the conf tree back on its branch.
# usage: plants/build-plant-probe.sh NAME
set -eu
B=/opt/build/value-oracle-1025
NAME=$1
cd "$B/conf"
HEAD=$(git rev-parse --abbrev-ref HEAD)
[ "$HEAD" = HEAD ] && HEAD=$(git rev-parse HEAD)
git fetch -q ../pallas.git "repro/plant-$NAME"
git checkout -q FETCH_HEAD
git diff --name-only "$HEAD" HEAD | xargs touch
TARGET=target-probe-value timeout 590 sh "$B/run.sh" gen/probe-value cargo build --release 2>&1 | tail -1
cp "$B/target-probe-value/release/pallas-probe-value" "$B/gen/bin/pallas-probe-value-$NAME"
git checkout -q "$HEAD"
git diff --name-only FETCH_HEAD HEAD | xargs touch
echo "built bin/pallas-probe-value-$NAME from $(git rev-parse --short FETCH_HEAD), conf back on $(git rev-parse --short HEAD)"
