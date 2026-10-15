#!/bin/sh
# usage: run.sh <ref> <cargo args...>
# Checks out <ref> from the bare repo into src, touches every file, runs one cargo command in pallas-ci:1
# with the shared cargo home and this dir's target, and prints the exit code and the verdict lines.
# TOOLCHAIN=1.88 runs the command on that toolchain with its own target dir.
D=/opt/build/impact-2026-10-15
ref=$1; shift
[ -d $D/src/.git ] || git clone -q $D/repo.git $D/src || exit 9
cd $D/src || exit 9
git fetch -q origin '+refs/heads/*:refs/remotes/origin/*' || exit 9
if git rev-parse -q --verify "origin/$ref^{commit}" >/dev/null; then
  git checkout -q -f --detach "origin/$ref" || exit 9
else
  git checkout -q -f --detach "$ref" || exit 9
fi
git clean -qfdx
find $D/src -path $D/src/.git -prune -o -type f -exec touch {} +
tc=${TOOLCHAIN:-}
tgt=$D/target${tc:+-$tc}
mkdir -p $D/logs $tgt
log=$D/logs/$(echo "$ref.$tc.$*" | tr ' /' '__').log
docker run --rm -v $D/src:/pallas -v $tgt:/target -v /opt/build/shared-cargo:/cargo \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_TERM_COLOR=never \
  ${tc:+-e RUSTUP_TOOLCHAIN=$tc} \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 -w /pallas pallas-ci:1 cargo "$@" > $log 2>&1
rc=$?
echo "REF $ref HEAD $(git rev-parse --short HEAD) TC ${tc:-default} CMD cargo $* EXIT $rc"
grep -E '^test result:' $log | sort | uniq -c
grep -E '^(error(\[E[0-9]+\])?:|failures:|    [A-Za-z0-9_:]+$|test .* FAILED|warning: )' $log | sort | uniq -c | head -30
grep -E '^error' -A12 $log | head -60
grep -E "panicked at|^thread '" -A4 $log | head -40
