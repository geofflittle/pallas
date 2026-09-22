#!/bin/sh
# Run one cargo command over $SRC (default /opt/build/pallas-eb/src) in the pallas CI image,
# with a target directory of its own. Debug info is off to keep the target
# small on a disk with under 10 GiB free.
exec docker run --rm \
  -v "${SRC:-/opt/build/pallas-eb/src}:/pallas" \
  -v /opt/build/pallas-eb/target:/target \
  -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -w /pallas \
  pallas-ci:1 \
  "$@"
