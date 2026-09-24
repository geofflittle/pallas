#!/bin/sh
# usage: run.sh <srcdir> <logname> <command...>
# Runs one cargo command in the pallas-ci image over /opt/build/eb-cuts/<srcdir>.
src=/opt/build/eb-cuts/$1
log=/opt/build/eb-cuts/logs/$2.log
shift 2
mkdir -p /opt/build/eb-cuts/logs
docker run --rm \
  -v "$src":/pallas \
  -v /opt/build/eb-cuts/target:/target \
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
df -h /opt/build | tail -1
exit $rc
