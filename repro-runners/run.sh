#!/bin/sh
# run.sh cmd...  runs cmd in the CI image against tree $A/$W with its own target dir $A/target-$W
A=/opt/build/cvmain-1016
W=${W:?W names the tree}
exec docker run --rm --name "cvmain-${JOBNAME:-job}" -v "$A/$W:/src/$W" -v "$A/target-$W:/target" -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w "/src/$W" pallas-ci:1 "$@"
