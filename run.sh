#!/bin/sh
# usage: run.sh <box area name> <log name> <cargo args...>
# Runs one cargo command on the box over /opt/build/<area>/src with the shared
# wave1 target, refuses under 6 GiB free, and keeps the log on the box.
area="$1"
log="$2"
shift 2
ssh musashi-aws "cd /opt/build/$area && df -h / | tail -1 && free=\$(df --output=avail / | tail -1) && if [ \$free -lt 6291456 ]; then echo 'refused: under 6 GiB free'; exit 4; fi; timeout 1500 docker run --rm -v /opt/build/$area/src:/pallas -v /opt/build/wave1-target:/target -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w /pallas pallas-ci:1 cargo $* > /opt/build/$area/$log.log 2>&1; code=\$?; echo exit=\$code > /opt/build/$area/$log.exit; echo exit=\$code; tail -5 /opt/build/$area/$log.log"
