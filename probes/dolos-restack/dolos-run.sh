#!/bin/sh
# Runs one command over /opt/build/ldisc-a/dolos in the Dolos build image, to logs/<name>.log.
set -u
W=/opt/build/ldisc-a
NAME=$1
shift
LOG=$W/logs/$NAME.log
OFFLINE=true
[ "${NET:-0}" = 1 ] && OFFLINE=false
echo "CMD $* START $(date -u '+%Y-%m-%dT%H:%M:%SZ') free_kb=$(df --output=avail / | tail -1)" > "$LOG"
docker run --rm --name "ldisc-a-$NAME" \
  --oom-score-adj 1000 \
  -v "$W":/f \
  -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/f/dolos-target \
  -e CARGO_NET_OFFLINE=$OFFLINE \
  -e CARGO_INCREMENTAL=0 \
  -e RUSTUP_TOOLCHAIN=1.98.0-x86_64-unknown-linux-gnu \
  -e CARGO_PROFILE_DEV_DEBUG=0 \
  -w /f/dolos \
  dolos-build:1 \
  sh -c "$*" >> "$LOG" 2>&1
rc=$?
echo "END $(date -u '+%Y-%m-%dT%H:%M:%SZ') free_kb=$(df --output=avail / | tail -1) EXIT=$rc" >> "$LOG"
exit $rc
