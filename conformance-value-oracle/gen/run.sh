#!/bin/sh
# Generates N values per kind and source, decodes them with the ledger and with pallas, and compares.
# With a probe that maps through each utxorpc schema, schemas.py also compares each schema.
# GEN_FROM names an earlier output dir whose generated values and ledger results are reused.
# usage: [PROBE=bin/other-probe] [OUT=dir] [GEN_FROM=dir] ./run.sh N SEED [KINDS] [SOURCES]
set -eu
W=/opt/build/conformance-gen-1024
N=${1:-100}
SEED=${2:-1}
KINDS=${3:-"tx body wits ppu block"}
SOURCES=${4:-"arb cddl-plain cddl"}
OUT=${OUT:-$W/out}
PROBE=${PROBE:-$W/bin/pallas-probe}
GEN_FROM=${GEN_FROM:-}
mkdir -p "$OUT"
HS="docker run --rm -i --log-driver none --user $(id -u):$(id -g) -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen"
for kind in $KINDS; do
  for src in $SOURCES; do
    base=$OUT/$kind.$src
    if [ -n "$GEN_FROM" ]; then
      cp "$GEN_FROM/$kind.$src.gen.jsonl" "$base.gen.jsonl"
      cp "$GEN_FROM/$kind.$src.oracle.jsonl" "$base.oracle.jsonl"
    else
    case $src in
      cuddle-cli) docker run --rm --log-driver none --user "$(id -u):$(id -g)" -v "$W:/work" conformance-gen-hs:2 sh /work/gen-cuddle-cli.sh "$kind" "$N" "$SEED" > "$base.gen.jsonl" ;;
      cuddle-cli-plain) docker run --rm --log-driver none --user "$(id -u):$(id -g)" -v "$W:/work" conformance-gen-hs:2 sh /work/gen-cuddle-cli.sh "$kind" "$N" "$SEED" --no-twiddle > "$base.gen.jsonl" ;;
      *) $HS gen "$kind" "$src" "$N" "$SEED" > "$base.gen.jsonl" ;;
    esac
    python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])' < "$base.gen.jsonl" | $HS oracle "$kind" > "$base.oracle.jsonl"
    fi
    "$PROBE" "$kind" < "$base.gen.jsonl" > "$base.probe.jsonl"
    # The ledger decodes each pallas re-encoding that differs from the input, "00" holds the line otherwise.
    python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l).get("reencode_hex") or "00")' < "$base.probe.jsonl" | $HS oracle "$kind" > "$base.reenc.oracle.jsonl"
    python3 -I "$W/compare.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base.dis.jsonl" "$base.reenc.oracle.jsonl" > "$base.summary.txt"
    if head -1 "$base.probe.jsonl" | python3 -I -c 'import json,sys
sys.exit(0 if "schemas" in json.loads(sys.stdin.read() or "{}") else 1)'; then
      python3 -I "$W/schemas.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base" >> "$base.summary.txt"
    fi
    echo "== $kind $src"
    cat "$base.summary.txt"
  done
done
