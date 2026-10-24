#!/bin/sh
# Runs the brief's check list on one tree and prints one verdict line per step.
# usage: [W=dir] [U=features] sh checks.sh TREE BLUEPRINT_DIR [STEP...]
# TREE and BLUEPRINT_DIR are relative to W, default /opt/build/pallas-conformance-1024.
# BLUEPRINT_DIR is the checkout matching the tree's cardano-blueprint gitlink.
T=$1
BP=$2
shift 2
W=${W:-/opt/build/pallas-conformance-1024}
export W
cd "$W"
rm -rf "$T/cardano-blueprint"
cp -r "$BP" "$T/cardano-blueprint"
find "$T" -name '*.rs' -exec touch {} +
L=logs/$T
mkdir -p "$L"
U=${U:-pallas-primitives/unstable,pallas-traverse/unstable}
HAS_PBP=$(/usr/bin/grep -c '^blueprint' "$T/pallas-primitives/Cargo.toml")
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
ONLY="$*"
step fmt ./run.sh "$T" cargo fmt --all -- --check
step three-unstable ./run.sh "$T" cargo test -p pallas-primitives -p pallas-traverse -p pallas-utxorpc --features $U
step three ./run.sh "$T" cargo test -p pallas-primitives -p pallas-traverse -p pallas-utxorpc
step workspace ./run.sh "$T" cargo test --workspace
step workspace-unstable ./run.sh "$T" cargo test --workspace --features $U
step clippy ./run.sh "$T" cargo clippy --workspace --all-targets -- -D warnings
step clippy-unstable ./run.sh "$T" cargo clippy --workspace --all-targets --features $U -- -D warnings
step blueprint ./run.sh "$T" cargo test -p pallas-network -p pallas-network2 --features blueprint
if [ "$HAS_PBP" != "0" ]; then
  step blueprint-primitives ./run.sh "$T" cargo test -p pallas-primitives --features unstable,blueprint
fi
step nodefault-primitives ./run.sh "$T" cargo check -p pallas-primitives --no-default-features
TARGET=target-msrv step msrv ./run.sh "$T" cargo +1.88 check --workspace --all-targets --features $U
