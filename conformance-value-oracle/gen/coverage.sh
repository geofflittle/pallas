#!/bin/sh
# Extracts a branch of pallas.git to trees/NAME and runs the w42 coverage test on it.
# usage: sh gen/coverage.sh NAME BRANCH
# The samples, the synthetic blocks and the blueprint are read only mounts of the earlier runs.
B=/opt/build/value-oracle-1025
I=/opt/build/integration-1025
cd "$B" || exit 1
mkdir -p trees logs
rm -rf "trees/$1"
mkdir -p "trees/$1"
git --git-dir=pallas.git archive "$2" | tar -x -C "trees/$1"
rm -rf "trees/$1/cardano-blueprint"
cp -r "$I/bp-0460" "trees/$1/cardano-blueprint"
find "trees/$1" -name '*.rs' -exec touch {} +
echo "== $1 $2 $(git --git-dir=pallas.git rev-parse --short "$2")"
docker run --rm --user "$(id -u):$(id -g)" -v "$B:/w" \
  -v /opt/build/w42-local-1020/samples:/samples:ro \
  -v "$I/synth:/synth:ro" \
  -e HOME=/w/home -e CARGO_HOME=/w/cargo-home -e RUSTUP_HOME=/usr/local/rustup \
  -e CARGO_TARGET_DIR=/w/target-coverage -e CARGO_INCREMENTAL=0 -e CARGO_BUILD_JOBS=3 \
  -e CARGO_TERM_COLOR=never -e W42_SAMPLES=/samples -e SYNTH=/synth \
  -w "/w/trees/$1" pallas-ci:1 \
  cargo test -p u5c-chain-oracle --test w42_coverage -- --nocapture > "logs/coverage-$1.log" 2>&1
echo "exit $?"
/usr/bin/grep -c '^COV ' "logs/coverage-$1.log"
/usr/bin/grep '^COV ' "logs/coverage-$1.log" | /usr/bin/grep -v 'failing \[\]'
/usr/bin/grep -E '^ALL PoolBls|^AGREES|^test result|panicked|is failing|^error' "logs/coverage-$1.log"
