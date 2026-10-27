#!/bin/sh
# Runs the w42 coverage over the sample blocks, the synthetic blocks and the generated blocks in coverage/blocks
# on a pallas revision, then compares the values of the same blocks with the ledger in every schema, and requires
# each location coverage saw to have an agreeing value in every compared block that holds it.
# usage: [BLOCKS=dir] sh gen/coverage.sh NAME REV PROBE
# BLOCKS names another directory of generated block hex files.
# REV is a revision of the pallas clone at W/pallas, PROBE the value probe built from the same revision.
# Exits 1 when the coverage test, the value run or the location check fails.
set -u
W=${W:-/opt/build/harness-gaps-1026}
G=$W/gen
SAMPLES=/opt/build/w42-local-1020/samples
SYNTH=$W/synth-coverage
NAME=$1
REV=$2
PROBE=$3
cd "$W" || exit 1
mkdir -p trees logs
rm -rf "$SYNTH"
mkdir -p "$SYNTH"
cp /opt/build/integration-1025/synth/b*.cbor "$SYNTH/"
for f in "${BLOCKS:-$G/coverage/blocks}"/*.hex; do
  [ -e "$f" ] || continue
  printf '8208%s' "$(cat "$f")" | xxd -r -p > "$SYNTH/block_gen_$(basename "$f" .hex).cbor"
done
rm -rf "trees/$NAME"
mkdir -p "trees/$NAME"
git -C pallas archive "$REV" | tar -x -C "trees/$NAME"
rm -rf "trees/$NAME/cardano-blueprint"
cp -r /opt/build/integration-1025/bp-0460 "trees/$NAME/cardano-blueprint"
cp "$G/coverage/w42_coverage.rs" "trees/$NAME/examples/u5c-chain-oracle/tests/w42_coverage.rs"
find "trees/$NAME" -name '*.rs' -exec touch {} +
echo "== $NAME $REV $(git -C pallas rev-parse --short "$REV")"
docker run --rm --log-driver none --user "$(id -u):$(id -g)" -v "$W:/w" \
  -v "$SAMPLES:/samples:ro" -v "$SYNTH:/synth:ro" \
  -e HOME=/w/home -e CARGO_HOME=/w/cargo-home -e RUSTUP_HOME=/usr/local/rustup \
  -e CARGO_TARGET_DIR=/w/target-coverage -e CARGO_INCREMENTAL=0 -e CARGO_BUILD_JOBS=3 \
  -e CARGO_TERM_COLOR=never -e W42_SAMPLES=/samples -e SYNTH=/synth \
  -w "/w/trees/$NAME" pallas-ci:1 \
  cargo test -p u5c-chain-oracle --test w42_coverage -- --nocapture > "logs/coverage-$NAME.log" 2>&1
test_status=$?
echo "coverage test: exit $test_status"
OUT=$G/cov-$NAME
rm -rf "$OUT"
mkdir -p "$OUT"
python3 -I -c '
import json
import sys
samples, synth = sys.argv[1:3]
for line in sys.stdin:
    if not line.startswith("COV "):
        continue
    path = line.split(" ")[1].replace("/samples/", samples + "/", 1).replace("/synth/", synth + "/", 1)
    cbor = open(path, "rb").read()
    assert cbor[0] == 0x82 and cbor[1] < 24, path
    print(json.dumps({"hex": cbor[2:].hex(), "file": path, "cov": line.split(" ")[1]}))
' "$SAMPLES" "$SYNTH" < "logs/coverage-$NAME.log" > "$OUT/block.given.gen.jsonl"
n=$(/usr/bin/grep -c . "$OUT/block.given.gen.jsonl")
echo "coverage blocks: $n"
SCHEMA_HELD=1 OUT=$OUT PROBE=$PROBE sh "$G/vrun.sh" "$n" 0 block given > "logs/coverage-values-$NAME.log" 2>&1
values_status=$?
/usr/bin/grep -E '^schema |DISAGREE|generated values' "logs/coverage-values-$NAME.log"
echo "coverage value run: exit $values_status"
python3 -I "$G/check_coverage_values.py" "logs/coverage-$NAME.log" "$OUT/block.given.gen.jsonl" \
  "$OUT/block.given.schema.counts.json" "$OUT/block.given.schema.held.json" > "logs/coverage-locations-$NAME.log"
locations_status=$?
/usr/bin/grep -E '^FAIL|^coverage values' "logs/coverage-locations-$NAME.log"
/usr/bin/grep -E '^AGREES|^test result|panicked|is failing|^error' "logs/coverage-$NAME.log"
status=0
[ "$test_status" -eq 0 ] && [ "$values_status" -eq 0 ] && [ "$locations_status" -eq 0 ] && [ "$n" -gt 0 ] || status=1
echo "coverage: exit $status"
exit $status
