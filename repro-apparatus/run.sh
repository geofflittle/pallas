#!/bin/sh
# One cargo command over /opt/build/u5c-v1/pallas in the pallas CI image,
# with its own target dir, resolving the spec git dependency from the local bare clone.
exec docker run --rm -v /opt/build/u5c-v1/pallas:/pallas -v /opt/build/u5c-v1/spec.git:/spec.git:ro -v "${U5C_TARGET:-/opt/build/u5c-v1/target}:/target" -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never -e CARGO_NET_GIT_FETCH_WITH_CLI=true -e GIT_CONFIG_COUNT=2 -e GIT_CONFIG_KEY_0=url.file:///spec.git.insteadOf -e GIT_CONFIG_VALUE_0=https://github.com/SundaeSwap-finance/utxorpc-spec -e GIT_CONFIG_KEY_1=safe.directory -e GIT_CONFIG_VALUE_1="*" -e RUST_BACKTRACE=short -e REGENERATE_SNAPSHOTS -w /pallas pallas-ci:1 "$@"
