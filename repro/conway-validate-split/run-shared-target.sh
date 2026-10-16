#!/bin/sh
# run-shared-target.sh cmd...  the first runner, one target dir for every tree, which linked a library built from another tree
A=/opt/build/cvsplit-1016
W=${W:?W names the tree}
exec docker run --rm --name "cvsplit-${JOBNAME:-job}" -v "$A/$W:/src/$W" -v "$A/target:/target" -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_PROFILE_RELEASE_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w "/src/$W" pallas-ci:1 "$@"
