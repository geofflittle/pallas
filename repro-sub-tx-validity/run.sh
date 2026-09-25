#!/bin/sh
exec docker run --rm -v /opt/build/subtx-validity/src:/pallas -v /opt/build/subtx-validity/target:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_TERM_COLOR=never -w /pallas pallas-ci:1 "$@"
