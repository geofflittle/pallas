#!/bin/sh
# Builds and runs the consumer probe against the pallas tree given as $1.
# Expected on a 0ea294c1 tree:
#   A normal: body_events=1 banned=false
#   B helper: banned=false
#   C on_outbound_msg: banned=false
#   D responder on_outbound_msg: banned=false
#   E responder apply_msg: banned=true
# Expected on the branch tree:
#   A normal: body_events=1 banned=false
#   B helper: banned=true
#   C on_outbound_msg: banned=true
#   D responder on_outbound_msg: banned=true
#   E responder apply_msg: banned=false
set -eu
TREE=$1
HERE=$(cd "$(dirname "$0")" && pwd)
# Cargo fingerprints path dependencies by mtime, so a tree swapped in under
# the same path is rebuilt only if its files are newer than the last build.
find "$TREE" -path "$TREE/target" -prune -o -type f -exec touch {} +
docker run --rm \
  -v "$TREE":/tree:ro \
  -v "$HERE":/probe \
  -v /opt/build/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/probe/target \
  -e CARGO_TERM_COLOR=never \
  -w /probe pallas-ci:1 cargo run -q
