#!/bin/sh
# usage: W=<tree dir name> T=<target name> run.sh cargo ...
A=/opt/build/final-fixes-1017
mkdir -p "$A/target-$T"
exec docker run --rm --name "ffx-${W}-${T}-$$" -v "$A/$W:/src" -v "$A/target-$T:/target" -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e RUST_BACKTRACE=short -e RUSTDOCFLAGS="$RUSTDOCFLAGS" \
  -w /src pallas-ci:1 "$@"
