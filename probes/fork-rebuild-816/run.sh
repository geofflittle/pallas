#!/bin/sh
# usage: run.sh <src dir name> <logname> <command...>
base=/opt/build/fork-rebuild-816
src=$1
log=$base/logs/$2.log
shift 2
docker run --rm \
  -v $base/$src:/pallas \
  -v $base/target:/target \
  -v /opt/build/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -w /pallas \
  pallas-ci:1 "$@" > "$log" 2>&1
rc=$?
echo "EXIT $rc" >> "$log"
echo "EXIT $rc"
exit $rc
