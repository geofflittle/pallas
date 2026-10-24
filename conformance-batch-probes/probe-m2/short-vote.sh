#!/bin/sh
# Feeds the ledger oracle the governance body as committed and with its vote written as 8102.
# Expected: line 1 has "ok":true, line 2 has "expected list len or indef" and "ok":false.
# usage: sh short-vote.sh PALLAS_TREE   (run on the box)
set -eu
H=$(cat "$1/test_data/dijkstra-generated/body-governance-ending-in-null.hex")
SHORT="$(printf '%s' "$H" | cut -c1-166)8102$(printf '%s' "$H" | cut -c173-)"
printf '%s\n%s\n' "$H" "$SHORT" \
  | docker run --rm -i --user "$(id -u):$(id -g)" -v /opt/build/conformance-gen-1024:/work \
      conformance-gen-hs:2 /work/bin/conformance-gen oracle body \
  | sed -e 's/"facts":{[^}]*}/"facts":{...}/'
