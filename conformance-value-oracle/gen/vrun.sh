#!/bin/sh
# Generates N values per kind and source, decodes them with the ledger and with pallas, and compares the values.
# valuecmp.py compares the pallas value and re-encoding with the ledger's, schemacmp.py each utxorpc schema.
# GEN_FROM names an earlier output dir whose generated values and ledger results are reused.
# The source `given` generates nothing and reads the values already written to OUT/KIND.given.gen.jsonl.
# The sources `mutate`, `empty14` and `subkeyhash` are `arb` values rewritten by mutate.py in that mode, and these and `arb` read the `arb` values of their kind, generated once into OUT/KIND.arb.N.SEED.seed.jsonl.
# A probe line naming a planted defect it reached goes to OUT/KIND.SOURCE.reached, and OUT/KIND.SOURCE.compared holds the value count when every compare of the pair finished.
# Exits 1 when any comparison finds a disagreement or a generated kind and source has other than N values.
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
HS="docker run --rm -i --log-driver none --user $(id -u):$(id -g) -v $W:/work $(sh "$W/image-name.sh" conformance-gen-hs) /work/bin/conformance-gen"
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
      arb=$OUT/$kind.arb.$N.$SEED.seed.jsonl
      case $src in
        given) ;;
        arb | mutate | empty14 | subkeyhash)
          if [ ! -s "$arb" ]; then
            $HS gen "$kind" arb "$N" "$SEED" > "$arb.$$"
            mv "$arb.$$" "$arb"
          fi
          if [ "$src" = arb ]; then
            cp "$arb" "$base.gen.jsonl"
          else
            python3 -I "$W/mutate.py" "$SEED" "$src" < "$arb" > "$base.gen.jsonl"
          fi
          ;;
        *) $HS gen "$kind" "$src" "$N" "$SEED" > "$base.gen.jsonl" ;;
      esac
      hexes hex < "$base.gen.jsonl" | $HS oracle "$kind" > "$base.oracle.jsonl"
    fi
    have=$(grep -c . "$base.gen.jsonl")
    if [ "$src" != given ] && [ "$have" -ne "$N" ]; then
      echo "$kind $src: $have generated values where $N were asked"
      status=1
    fi
    "$PROBE" "$kind" < "$base.gen.jsonl" > "$base.probe.out"
    grep '^ORACLE_PLANT reached ' "$base.probe.out" > "$base.reached"
    grep -v '^ORACLE_PLANT reached ' "$base.probe.out" > "$base.probe.jsonl"
    rm -f "$base.probe.out"
    hexes reencode_hex < "$base.probe.jsonl" | $HS oracle "$kind" > "$base.reoracle.jsonl"
    rm -f "$base.counts.json" "$base.schema.counts.json" "$base.compared"
    python3 -I "$W/valuecmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base.reoracle.jsonl" "$base" || status=1
    counts=$base.counts.json
    # No schema maps an endorser block's references.
    if [ "$kind" != eb ]; then
      python3 -I "$W/schemacmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base" || status=1
      counts="$counts $base.schema.counts.json"
    fi
    # The compare finished, a pass or a disagreement, when each compare wrote its counts, which it writes only after reading an aligned row of every file for each value.
    if ls $counts >/dev/null 2>&1; then
      echo "$have" > "$base.compared"
    fi
  done
done
exit $status
