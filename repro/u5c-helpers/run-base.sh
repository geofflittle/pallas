#!/bin/sh
# usage: run-base.sh <command...>
# Runs one command in the CI image against the base tree (f574d839) in this
# area, with its own target, debuginfo off. Exits with the command's status.
D=/opt/build/u5c-helpers-2026-10-09
exec docker run --rm \
  -v $D/base:/pallas \
  -v $D/target-base:/target \
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
