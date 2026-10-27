#!/bin/sh
# Compares the pallas value, re-encoding and schema views of each minimal reproduction with the ledger's, field by field.
# Exits 1 when any reproduction disagrees, including a pallas refusal of an input the ledger accepts.
# usage: [PROBE=bin/other-probe] ./repro/run-repros.sh
set -u
W=$(cd "$(dirname "$0")/.." && pwd)
R=$W/repro
HS="docker run --rm -i --log-driver none --user $(id -u):$(id -g) -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen"
PROBE=${PROBE:-$W/bin/pallas-probe-value}

hexes() {
  python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l).get(sys.argv[1]) or "00")' "$1"
}

status=0
for spec in "ppu ppu-indef-rational" "body voting-null-anchor" "tx golden-tx" "block peras-cert-block"; do
  set -- $spec
  kind=$1
  base=$R/$2
  echo "== $2"
  hexes hex < "$base.jsonl" | $HS oracle "$kind" > "$base.oracle.jsonl"
  "$PROBE" "$kind" < "$base.jsonl" > "$base.probe.jsonl"
  hexes reencode_hex < "$base.probe.jsonl" | $HS oracle "$kind" > "$base.reoracle.jsonl"
  python3 -I "$W/valuecmp.py" "$kind" "$base.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base.reoracle.jsonl" "$base" || status=1
  python3 -I "$W/schemacmp.py" "$kind" "$base.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base" || status=1
done
echo "repros: exit $status"
exit $status
