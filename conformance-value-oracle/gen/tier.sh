#!/bin/sh
# Runs vrun.sh over each kind and source pair of PAIRS, the given values read from the files TX_FILES, BLOCK_FILES and EB_FILES name, then vtally.py over the output.
# A pair is KIND:SOURCE. GEN_FROM reuses the values and ledger results of an earlier tier run, so only the probe differs.
# Exits 1 when a run or the tally fails.
# usage: [PROBE=bin] [GEN_FROM=dir] [PAIRS="tx:given ..."] [TX_FILES=..] [BLOCK_FILES=..] [EB_FILES=..] ./tier.sh OUT N SEED
set -u
W=$(cd "$(dirname "$0")" && pwd)
OUT=$1
N=$2
SEED=$3
PAIRS=${PAIRS:-"tx:given block:given eb:given eb:arb tx:arb tx:cddl-plain tx:cddl tx:mutate tx:empty14 tx:subkeyhash body:arb body:cddl-plain body:cddl body:mutate body:empty14 wits:arb wits:cddl-plain wits:cddl wits:mutate ppu:arb ppu:cddl-plain ppu:cddl ppu:mutate block:arb block:cddl-plain block:cddl block:mutate"}
mkdir -p "$OUT"
status=0
for pair in $PAIRS; do
  kind=${pair%%:*}
  src=${pair#*:}
  if [ "$src" = given ] && [ -z "${GEN_FROM:-}" ]; then
    case $kind in
      tx) files=${TX_FILES:-} ;;
      block) files=${BLOCK_FILES:-} ;;
      eb) files=${EB_FILES:-} ;;
      *) files= ;;
    esac
    if [ -z "$files" ]; then
      echo "$kind given: no files"
      status=1
      continue
    fi
    # shellcheck disable=SC2086
    python3 -I "$W/given.py" "$kind" $files >"$OUT/$kind.given.gen.jsonl" || status=1
  fi
  OUT=$OUT sh "$W/vrun.sh" "$N" "$SEED" "$kind" "$src" >"$OUT/$kind.$src.log" 2>&1 || status=1
done
python3 -I "$W/vtally.py" "$OUT" || status=1
exit $status
