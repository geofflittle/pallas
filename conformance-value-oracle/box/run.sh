#!/bin/sh
# Runs one command in the pallas CI image against a tree under W.
# usage: [W=dir] [TARGET=name] run.sh TREE COMMAND...
W=${W:-/opt/build/value-oracle-1025}
TREE=$1
shift
exec docker run --rm \
  --user "$(id -u):$(id -g)" \
  -v "$W:/w" \
  -e HOME=/w/home \
  -e CARGO_HOME=/w/cargo-home \
  -e RUSTUP_HOME=/usr/local/rustup \
  -e CARGO_TARGET_DIR="/w/${TARGET:-target}" \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_BUILD_JOBS=3 \
  -e CARGO_TERM_COLOR=never \
  -e RUST_BACKTRACE=short \
  -w "/w/$TREE" \
  pallas-ci:1 \
  "$@"
