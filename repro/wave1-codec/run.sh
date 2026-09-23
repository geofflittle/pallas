#!/bin/sh
# Run one cargo command against /opt/build/wave1-codec/src in the pallas CI
# image, sharing the wave1 target directory and the cargo home with the
# other wave1 builders. Prints disk before starting and refuses under 6 GiB.
free_kib=$(df --output=avail / | tail -1)
df -h / | tail -1
if [ "$free_kib" -lt 6291456 ]; then
  echo "refused: under 6 GiB free"
  exit 4
fi
mkdir -p /opt/build/wave1-target
exec timeout 1500 docker run --rm \
  -v /opt/build/wave1-codec/src:/pallas \
  -v /opt/build/wave1-target:/target \
  -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -e RUSTDOCFLAGS="$RUSTDOCFLAGS" \
  -w /pallas \
  pallas-ci:1 \
  "$@"
