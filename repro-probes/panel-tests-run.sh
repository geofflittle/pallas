#!/bin/sh
# usage: run.sh <name> <cargo command...>  runs in src-<name> with target-<name>, debug info off
D=/opt/build/panel-tests
N=$1; shift
exec docker run --rm -v $D/src-$N:/pallas -v $D/target-$N:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w /pallas pallas-ci:1 "$@"
