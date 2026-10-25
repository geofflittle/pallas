#!/bin/sh
# Generates N values of one kind with the standalone cuddle tool over the Dijkstra CDDL text, one JSON line each.
# Runs inside the conformance-gen-hs image with /work mounted.
# usage: gen-cuddle-cli.sh KIND N SEED [--no-twiddle]
set -eu
KIND=$1
N=$2
SEED=$3
FLAG=${4:-}
CDDL=/work/cardano-ledger/eras/dijkstra/impl/cddl/data/dijkstra.cddl
case $KIND in
  tx) RULE=mempool_transaction ;;
  body) RULE=transaction_body ;;
  wits) RULE=transaction_witness_set ;;
  ppu) RULE=protocol_param_update ;;
  block) RULE=block ;;
  *) echo "unknown kind $KIND" >&2; exit 2 ;;
esac
i=0
while [ "$i" -lt "$N" ]; do
  s=$((SEED * 100000 + i))
  h=$(/work/bin-cuddle/cuddle gen -f hex -s "$s" $FLAG "$RULE" "$CDDL" | tail -n 1)
  printf '{"source":"cuddle-cli%s","seed":%s,"hex":"%s"}\n' "$FLAG" "$s" "$h"
  i=$((i + 1))
done
