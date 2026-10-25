#!/bin/sh
# Reruns the value and schema comparisons over an earlier vrun.sh output dir without generating or decoding again.
# usage: ./recmp.sh DIR [KINDS] [SOURCES]
set -u
W=$(dirname "$0")
D=$1
KINDS=${2:-"tx body wits ppu block"}
SOURCES=${3:-"arb cddl-plain cddl mutate"}
status=0
for kind in $KINDS; do
  for src in $SOURCES; do
    base=$D/$kind.$src
    [ -f "$base.probe.jsonl" ] || continue
    python3 -I "$W/valuecmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base.reoracle.jsonl" "$base" || status=1
    python3 -I "$W/schemacmp.py" "$kind" "$base.gen.jsonl" "$base.oracle.jsonl" "$base.probe.jsonl" "$base" || status=1
  done
done
exit $status
