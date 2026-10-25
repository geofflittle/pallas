#!/bin/sh
# Runs every command of the tree's ci.yml, then the same tests and lints with the unstable features on.
# usage: [W=dir] sh ci.sh TREE BLUEPRINT_DIR [STEP...]
# Prints one "STEP exit N passed P failed F" line per step, logs under W/logs/TREE.
T=$1
BP=$2
shift 2
W=${W:-/opt/build/value-oracle-1025}
export W
cd "$W"
rm -rf "$T/cardano-blueprint"
cp -r "$BP" "$T/cardano-blueprint"
find "$T" -name '*.rs' -exec touch {} +
L=logs/$T
mkdir -p "$L"
U=pallas/unstable,pallas-primitives/unstable,pallas-traverse/unstable,pallas-utxorpc/unstable,pallas-validate/unstable,pallas-network2/unstable
ONLY="$*"
want() {
  [ -z "$ONLY" ] && return 0
  for s in $ONLY; do [ "$s" = "$1" ] && return 0; done
  return 1
}
step() {
  name=$1
  shift
  want "$name" || return 0
  "$@" > "$L/$name.log" 2>&1
  code=$?
  counts=$(/usr/bin/grep -E '^test result' "$L/$name.log" | awk '{p+=$4; f+=$6} END {if (NR) printf "passed %d failed %d", p, f}')
  echo "$name exit $code $counts"
}
R="sh $W/run.sh $T"
step fmt $R cargo fmt --all --check
step clippy $R cargo clippy --workspace --all-targets -- -D warnings
step doc $R cargo doc --workspace --no-deps
step build $R cargo build --workspace --all-targets
step test $R cargo test --workspace
step blueprint $R cargo test -p pallas-network -p pallas-network2 --features blueprint
step unstable $R cargo test -p pallas-primitives -p pallas-traverse -p pallas-utxorpc -p pallas-validate --features unstable
step unstable-blueprint $R cargo test -p pallas-primitives --features unstable,blueprint
step phase2 $R cargo test -p pallas-validate --features phase2
step phase2-unstable $R cargo test -p pallas-validate --features phase2,unstable
step no-default $R cargo check --workspace --all-targets --no-default-features
step all-features $R cargo check --workspace --all-targets --all-features
step isolated $R cargo check -p pallas-primitives --no-default-features
TARGET=target-msrv step msrv $R cargo +1.88 check --workspace --all-targets
step test-u $R cargo test --workspace --features $U
step clippy-u $R cargo clippy --workspace --all-targets --features $U -- -D warnings
step clippy-all $R cargo clippy --workspace --all-targets --all-features -- -D warnings
TARGET=target-msrv step msrv-u $R cargo +1.88 check --workspace --all-targets --features $U
