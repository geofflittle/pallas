#!/bin/sh
# Walks a utxorpc v1 server to the node tip, checks the inlined endorser transactions against the announcements,
# and compares every served value with the ledger, failing when any of the three fails.
# usage: NODE_TIP=slot:hash ORACLE=cmd e2e-run.sh GRPC OUT
# Writes OUT.walk.txt, OUT.ebcheck.txt and OUT.compare.txt, and prints their verdict lines and "e2e: exit N".
set -u
G=$(cd "$(dirname "$0")" && pwd)
export SERVED_DIR=${SERVED_DIR:-$G/e2e}
GRPC=$1
OUT=$2
[ -n "${NODE_TIP:-}" ] || { echo "NODE_TIP is not set"; echo "e2e: exit 1"; exit 1; }
status=0
python3 -I "$G/e2e_values.py" walk "$GRPC" "$OUT" > "$OUT.walk.txt" 2>&1
code=$?
echo "walk: exit $code"
[ "$code" -eq 0 ] || status=1
python3 -I "$G/e2e/ebcheck.py" "$GRPC" "$OUT/ebcheck" > "$OUT.ebcheck.txt" 2>&1
code=$?
/usr/bin/grep '^ebcheck:' "$OUT.ebcheck.txt"
echo "ebcheck: exit $code"
[ "$code" -eq 0 ] || status=1
python3 -I "$G/e2e_values.py" compare "$OUT" > "$OUT.compare.txt" 2>&1
code=$?
echo "compare: exit $code"
[ "$code" -eq 0 ] || status=1
/usr/bin/grep -E '^v1 |DISAGREE|^endorser positions|^ledger refuses' "$OUT.compare.txt"
echo "e2e: exit $status"
exit $status
