#!/bin/sh
A=/opt/build/cvfix-1015
W=${W:-w}
exec docker run --rm --name "cvfix-${JOBNAME:-job}" -v "$A/$W:/w" -v "$A/target-$W:/target" -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w /w pallas-ci:1 "$@"
