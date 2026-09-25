#!/bin/sh
# usage: run.sh <workdir: pallas or dolos> <target dir name> <logname> <command...>
# Runs one cargo command in pallas-ci:1 with the pallas tree at /pallas and
# the Dolos tree at /dolos, writing the log under logs/ and its exit code last.
base=/opt/build/fork-rebuild2
work=$1
target=$2
log=$base/logs/$3.log
shift 3
mkdir -p $base/logs $base/$target
docker run --rm \
  -v $base/src:/pallas \
  -v $base/dolos:/dolos \
  -v $base/$target:/target \
  -v /opt/build/cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -e CARGO_PROFILE_TEST_DEBUG=0 \
  -e CARGO_TERM_COLOR=never \
  -w /$work \
  pallas-ci:1 "$@" > "$log" 2>&1
rc=$?
echo "EXIT $rc" >> "$log"
echo "EXIT $rc"
exit $rc
