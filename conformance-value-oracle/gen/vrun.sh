#!/bin/sh
# Generates N values per kind and source, decodes them with the ledger and with pallas, and compares the values.
# valuecmp.py compares the pallas value and re-encoding with the ledger's, schemacmp.py each utxorpc schema.
# GEN_FROM names an earlier output dir whose generated values and ledger results are reused.
# The source `given` generates nothing and reads the values already written to OUT/KIND.given.gen.jsonl.
# Exits 1 when any comparison finds a disagreement or a kind and source has other than N generated values.
# usage: [PROBE=bin/other-probe] [OUT=dir] [GEN_FROM=dir] ./vrun.sh N SEED [KINDS] [SOURCES]
set -u
W=$(cd "$(dirname "$0")" && pwd)
N=${1:-100}
SEED=${2:-1}
KINDS=${3:-"tx body wits ppu block"}
SOURCES=${4:-"arb cddl-plain cddl mutate"}
OUT=${OUT:-$W/vout}
PROBE=${PROBE:-$W/bin/pallas-probe-value}
GEN_FROM=${GEN_FROM:-}
mkdir -p "$OUT"
HS="docker run --rm -i --log-driver none --user $(id -u):$(id -g) -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen"
hexes() {
  python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l).get(sys.argv[1]) or "00")' "$1"
}
status=0
for kind in $KINDS; do
  for src in $SOURCES; do
    base=$OUT/$kind.$src
    if [ -n "$GEN_FROM" ]; then
      ln -f "$GEN_FROM/$kind.$src.gen.jsonl" "$base.gen.jsonl"
      ln -f "$GEN_FROM/$kind.$src.oracle.jsonl" "$base.oracle.jsonl"
    else
      case $src in
        given) ;;
        mutate)
          $HS gen "$kind" arb "$N" "$SEED" > "$base.seed.jsonl"
          python3 -I "$W/mutate.py" "$SEED" < "$base.seed.jsonl" > "$base.gen.jsonl"
          ;;
        *) $HS gen "$kind" "$src" "$N" "$SEED" > "$base.gen.jsonl" ;;
      esac
      hexes hex < "$base.gen.jsonl" | $HS oracle "$kind" > "$base.oracle.jsonl"
    fi
    have=$(grep -c . "$base.gen.jsonl")
    if [ "$have" -ne "$N" ]; then
      echo "$kind $src: $have generated values where $N were asked"
      status=1
    fi
    "$PROBE" "$kind" < "$base.gen.jsonl" > "$base.probe.jsonl"
    hexes reencode_hex < "$base.probe.jsonl" | $HS oracle "$kind" > "$base.reoracle.jsonl"
    python3 -I "$W/valuecmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base.reoracle.jsonl" "$base" || status=1
    python3 -I "$W/schemacmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base" || status=1
  done
done
exit $status
