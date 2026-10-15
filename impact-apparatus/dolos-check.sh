#!/bin/sh
# usage: dolos-check.sh <cargo args...>
# Runs one cargo command in the Dolos scratch clone with every pallas crate patched to the pallas-tip checkout.
D=/opt/build/impact-2026-10-15
mkdir -p $D/logs $D/target-dolos
log=$D/logs/dolos.$(echo "$*" | tr " /" "__").log
docker run --rm -v $D/dolos:/dolos -v $D/pallas-tip:/pallas-tip -v $D/target-dolos:/target -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -w /dolos pallas-ci:1 cargo "$@" > $log 2>&1
rc=$?
echo "DOLOS pallas $(git -C $D/pallas-tip rev-parse --short HEAD) dolos $(git -C $D/dolos rev-parse --short HEAD) CMD cargo $* EXIT $rc"
grep -E '^test result:' $log | sort | uniq -c
grep -E "^(error(\[E[0-9]+\])?:|warning: unused|warning: patch)" -A8 $log | head -60
grep -E "panicked at|^thread '" -A4 $log | head -30
