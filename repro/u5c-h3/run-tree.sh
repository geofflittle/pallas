#!/bin/sh
# usage: TREE=<name> run-tree.sh <command...>
# Runs one command in the CI image against the tree $D/$TREE, with the target
# $D/target-$TREE and this area's cargo home, debuginfo off. Exits with the
# command's status.
D=/opt/build/u5c-h3-2026-10-09
[ -n "$TREE" ] || { echo "TREE unset"; exit 2; }
exec docker run --rm \
  -v $D/$TREE:/pallas \
  -v $D/target-$TREE:/target \
  -v $D/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -e PROBE_OUT="$PROBE_OUT" \
  -w /pallas \
  pallas-ci:1 \
  "$@"
