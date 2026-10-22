#!/bin/sh
# usage: run.sh <cargo command...>  runs in src with target, in the pallas-ci image, debug info off
D=/opt/build/v1-panel-fixes
exec docker run --rm -v $D/src:/pallas -v $D/target:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w /pallas pallas-ci:1 "$@"
