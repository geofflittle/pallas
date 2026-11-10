#!/bin/sh
# Runs vrun.sh over each kind and source pair of PAIRS, JOBS at a time, the given values read from the files TX_FILES, BLOCK_FILES and EB_FILES name, then vtally.py over the output.
# A pair is KIND:SOURCE. A pair whose source rewrites the arb values of its kind runs after the other pairs, so it reads the arb values its kind's arb pair wrote. GEN_FROM reuses the values and ledger results of an earlier tier run, so only the probe differs.
# VRUN and TALLY replace vrun.sh and vtally.py.
# Exits 1 when a run or the tally fails.
# usage: [PROBE=bin] [GEN_FROM=dir] [PAIRS="tx:given ..."] [JOBS=4] [TX_FILES=..] [BLOCK_FILES=..] [EB_FILES=..] ./tier.sh OUT N SEED
set -u
W=$(cd "$(dirname "$0")" && pwd)
OUT=$1
N=$2
SEED=$3
PAIRS=${PAIRS:-"tx:given block:given eb:given eb:arb tx:arb tx:cddl-plain tx:cddl tx:mutate tx:empty14 tx:subkeyhash body:arb body:cddl-plain body:cddl body:mutate body:empty14 wits:arb wits:cddl-plain wits:cddl wits:mutate ppu:arb ppu:cddl-plain ppu:cddl ppu:mutate block:arb block:cddl-plain block:cddl block:mutate"}
JOBS=${JOBS:-4}
VRUN=${VRUN:-$W/vrun.sh}
mkdir -p "$OUT"

# Whether the source $1 rewrites the arb values of its kind.
derived() {
  case $1 in
    mutate | empty14 | subkeyhash) ;;
    *) return 1 ;;
  esac
}

# Runs the pair $1 into OUT/KIND.SOURCE.log and writes its exit status to OUT/KIND.SOURCE.status.
pair() {
  kind=${1%%:*}
  src=${1#*:}
  if [ "$src" = given ] && [ -z "${GEN_FROM:-}" ]; then
    case $kind in
      tx) files=${TX_FILES:-} ;;
      block) files=${BLOCK_FILES:-} ;;
      eb) files=${EB_FILES:-} ;;
      *) files= ;;
    esac
    if [ -z "$files" ]; then
      echo "$kind given: no files"
      echo 1 >"$OUT/$kind.$src.status"
      return
    fi
    # shellcheck disable=SC2086
    if ! python3 -I "$W/given.py" "$kind" $files >"$OUT/$kind.given.gen.jsonl"; then
      echo 1 >"$OUT/$kind.$src.status"
      return
    fi
  fi
  st=0
  OUT=$OUT sh "$VRUN" "$N" "$SEED" "$kind" "$src" >"$OUT/$kind.$src.log" 2>&1 || st=1
  echo "$st" >"$OUT/$kind.$src.status"
}

# Runs pair on each argument, JOBS at a time with the arguments dealt to JOBS workers in turn, then prints each run's output in argument order.
pool() {
  worker=0
  while [ "$worker" -lt "$JOBS" ]; do
    (
      i=0
      for item; do
        if [ $((i % JOBS)) -eq "$worker" ]; then
          pair "$item" >"$OUT/${item%%:*}.${item#*:}.pool" 2>&1
        fi
        i=$((i + 1))
      done
    ) &
    worker=$((worker + 1))
  done
  wait
  for item; do
    cat "$OUT/${item%%:*}.${item#*:}.pool"
    rm -f "$OUT/${item%%:*}.${item#*:}.pool"
  done
}

first=
later=
for p in $PAIRS; do
  rm -f "$OUT/${p%%:*}.${p#*:}.status"
  if derived "${p#*:}"; then
    later="$later $p"
  else
    first="$first $p"
  fi
done
# shellcheck disable=SC2086
pool $first
# shellcheck disable=SC2086
pool $later
status=0
for p in $PAIRS; do
  kind=${p%%:*}
  src=${p#*:}
  if [ "$(cat "$OUT/$kind.$src.status" 2>/dev/null)" != 0 ]; then
    status=1
  fi
done
if [ -n "${TALLY:-}" ]; then
  $TALLY "$OUT" || status=1
else
  python3 -I "$W/vtally.py" "$OUT" || status=1
fi
exit $status
