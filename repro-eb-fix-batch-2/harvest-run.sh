#!/bin/sh
# Runs one command in the pallas CI image against the harvester tree under
# /opt/build/pallas-eb-harvest, with a target directory of its own.
exec docker run --rm \
  -v /opt/build/pallas-eb-harvest/src:/pallas \
  -v /opt/build/pallas-eb-harvest/target:/target \
  -v /opt/build/pallas-eb-harvest/out:/out \
  -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -e RUST_LOG="${RUST_LOG:-info}" \
  -w /pallas \
  pallas-ci:1 \
  "$@"
