#!/bin/sh
# Runs one command in the pallas CI image against a tree under this directory.
# usage: [W=dir] run.sh TREE COMMAND...   (TREE is relative to W, default /opt/build/pallas-conformance-1024)
# TARGET picks the target directory name (default target), MSRV builds use their own.
# GEN names the harness dir conformance-value-oracle/gen whose build-image.sh names the image, needed when this script runs from a copy.
W=${W:-/opt/build/pallas-conformance-1024}
GEN=${GEN:-$(dirname "$0")/../../conformance-value-oracle/gen}
TREE=$1
shift
IMAGE=$(sh "$GEN/build-image.sh" pallas-ci) || exit 1
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
  "$IMAGE" \
  "$@"
