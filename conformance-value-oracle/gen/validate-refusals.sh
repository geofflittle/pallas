#!/bin/sh
# Builds Dijkstra transactions the ledger refuses at decode from the captured TTL transaction, and requires pallas-validate phase one to refuse each as malformed.
# The cases are the planted values of probe-validate and N mutate.py rewrites of the base transaction, all signed by the probe's key.
# Prints the per kind table and exits 1 when phase one accepts any transaction the ledger refuses, 2 when the base transaction is not accepted by both.
# usage: [PROBE=bin/pallas-probe-validate] [OUT=dir] ./validate-refusals.sh N SEED
set -u
W=$(cd "$(dirname "$0")" && pwd)
N=${1:-1000}
SEED=${2:-1}
OUT=${OUT:-$W/vrefusals}
PROBE=${PROBE:-$W/bin/pallas-probe-validate}
FIXTURE=${FIXTURE:-$W/../conf/test_data/dijkstra-ttl}
mkdir -p "$OUT"
HS="docker run --rm -i --log-driver none --user $(id -u):$(id -g) -v $W:/work $(sh "$W/image-name.sh" conformance-gen-hs) /work/bin/conformance-gen"
hexes() {
  python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])'
}
"$PROBE" plant "$FIXTURE" > "$OUT/cases.jsonl" || exit 2
"$PROBE" base "$FIXTURE" "$N" | python3 -I "$W/mutate.py" "$SEED" mutate | "$PROBE" sign "$FIXTURE" >> "$OUT/cases.jsonl" || exit 2
hexes < "$OUT/cases.jsonl" | $HS oracle tx > "$OUT/ledger.jsonl" || exit 2
"$PROBE" compare "$FIXTURE" "$OUT/cases.jsonl" "$OUT/ledger.jsonl"
