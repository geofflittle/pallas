#!/bin/sh
# usage: arms.sh <ref>...  checks out each ref in src, touches the tree, runs the pallas-utxorpc and pallas-traverse tests with unstable, prints counts and failures
D=/opt/build/v1-panel-fixes
mkdir -p $D/logs
for R in "$@"; do
  git -C $D/src checkout -q --detach "$R" || { echo "REF $R missing"; continue; }
  find $D/src -path $D/src/.git -prune -o -type f -exec touch {} +
  N=$(echo "$R" | tr / _)
  L=$D/logs/$N.log
  echo "REF $R $(git -C $D/src rev-parse --short HEAD) $(git -C $D/src log -1 --format=%s)"
  $D/run.sh cargo test -p pallas-utxorpc -p pallas-traverse --features unstable --no-fail-fast > $L 2>&1
  echo "  exit=$? $(grep -cE '^error(\[|:)' $L) compile errors"
  grep -E '^error(\[|:)' $L | head -3 | sed 's/^/  /'
  awk '/Running unittests src\/lib.rs .*pallas_(utxorpc|traverse)-/{n=$0;sub(/.*deps\//,"",n);sub(/-.*/,"",n);p=1;next} p&&/^test result/{print "  " n ": " $0; p=0}' $L
  grep -E '^test .* FAILED$' $L | sed 's/^/  /'
done
