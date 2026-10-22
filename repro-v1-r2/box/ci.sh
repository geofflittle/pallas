#!/bin/sh
# usage: ci.sh <name>  every command of .github/workflows/ci.yml (Linux legs) on src-<name>
N=$1
D=/opt/build/v1-r2-1022
L=$D/ci-logs/$N; mkdir -p $L
echo "head $(git -C $D/src-$N rev-parse HEAD) blueprint $(git -C $D/src-$N/cardano-blueprint rev-parse HEAD)"
find $D/src-$N -type f -not -path "*/.git/*" -exec touch {} +
F=0
run() { J=$1; shift; "$@" > $L/$J.log 2>&1; C=$?; K=$(grep -cE "^ +(Compiling|Checking|Documenting) " $L/$J.log); if [ $C -eq 0 ]; then echo "$J pass crates=$K"; else echo "$J FAIL exit=$C crates=$K"; F=$((F+1)); fi; }
run fmt $D/run.sh $N cargo fmt --all --check
run clippy $D/run.sh $N cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" run docs $D/run.sh $N cargo doc --workspace --no-deps
run build $D/run.sh $N cargo build --workspace --all-targets
run test $D/run.sh $N cargo test --workspace
run test-blueprint $D/run.sh $N cargo test -p pallas-network -p pallas-network2 --features blueprint
run test-unstable $D/run.sh $N cargo test -p pallas-primitives -p pallas-traverse -p pallas-utxorpc --features unstable
run test-unstable-blueprint $D/run.sh $N cargo test -p pallas-primitives --features unstable,blueprint
run features-nodefault $D/run.sh $N cargo check --workspace --all-targets --no-default-features
run features-all $D/run.sh $N cargo check --workspace --all-targets --all-features
run isolated $D/run.sh $N cargo check -p pallas-primitives --no-default-features
run msrv $D/run.sh $N cargo +1.88 check --workspace --all-targets
echo "failed $F of 12"; df -h / | tail -1
