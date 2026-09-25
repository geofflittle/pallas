#!/bin/sh
# Run one command in the CI image against one tree of this run.
# Usage: run.sh <green|red> <command...>
TREE=$1
shift
exec docker run --rm \
  -v /opt/build/subtx-pr/$TREE:/pallas \
  -v /opt/build/subtx-pr/target:/target \
  -v /opt/build/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -e RUSTDOCFLAGS="$RUSTDOCFLAGS" \
  -w /pallas \
  pallas-ci:1 \
  "$@"
