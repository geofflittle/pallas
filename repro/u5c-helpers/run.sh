#!/bin/sh
# usage: run.sh <command...>
# Runs one command in the CI image against this area's source tree, with this
# area's own target, cargo home and toolchain, debuginfo off. Exits with the
# command's status. CI_TARGET overrides the target directory (the MSRV job).
D=/opt/build/u5c-helpers-2026-10-09
exec docker run --rm \
  -v $D/src:/pallas \
  -v "${CI_TARGET:-$D/target}:/target" \
  -v $D/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -e RUSTDOCFLAGS="$RUSTDOCFLAGS" \
  -e PROBE_OUT="$PROBE_OUT" \
  -w /pallas \
  pallas-ci:1 \
  "$@"
