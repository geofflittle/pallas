#!/bin/sh
# dk.sh <tree> <shell text>, box side with D set: runs the text in pallas-ci:1 with the tree at /w, one shared target dir, debuginfo off.
T=$1
shift
mkdir -p "$D/target"
exec docker run --rm -v "$D/$T":/w -v "$D/target":/target -v /opt/build/cargo:/cargo \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -w /w pallas-ci:1 sh -c "$*"
