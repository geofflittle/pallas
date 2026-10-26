#!/bin/sh
# Builds the value probe once per pallas revision, with the conf tree checked out at each, and returns conf to the first.
# usage: W=dir sh gen/plants/build-probes.sh NAME=REV...
# The first NAME=REV is the unplanted head. Each binary goes to gen/bin/pallas-probe-value-NAME.
set -u
W=${W:-/opt/build/harness-gaps-1026}
cd "$W" || exit 1
first=
for arg in "$@"; do
  name=${arg%%=*}
  rev=${arg#*=}
  [ -n "$first" ] || first=$rev
  git -C conf checkout -q --detach "$rev" || exit 1
  find conf -name '*.rs' -exec touch {} +
  W=$W TARGET=probe-target sh run.sh gen/probe-value cargo build --release > "logs/probe-$name.log" 2>&1
  code=$?
  cp probe-target/release/pallas-probe-value "gen/bin/pallas-probe-value-$name"
  echo "$name $(git -C conf rev-parse --short HEAD) build exit $code"
  [ "$code" -eq 0 ] || exit 1
done
git -C conf checkout -q --detach "$first"
