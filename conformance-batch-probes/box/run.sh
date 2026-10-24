#!/bin/sh
# Runs one command in the pallas CI image against a tree under this directory.
# usage: [W=dir] run.sh TREE COMMAND...   (TREE is relative to W, default /opt/build/pallas-conformance-1024)
# TARGET picks the target directory name (default target), MSRV builds use their own.
W=${W:-/opt/build/pallas-conformance-1024}
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
