#!/bin/sh
# Rebuilds the indefinite array fixtures from the generated records in out-base, into the directory named by the first argument.
set -eu
cd /opt/build/pallas-conformance-1024
DEST=$1
W=/opt/build/conformance-gen-1024
rm -rf out-framed
python3 -I frame.py out-base frame-m1.txt out-framed
for k in body tx wits; do
  python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])' < out-framed/$k.framed.gen.jsonl \
    | docker run --rm -i --user "$(id -u):$(id -g)" -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen oracle $k \
    > out-framed/$k.framed.oracle.jsonl
done
python3 -I regen_fixtures.py . "$DEST" manifest-m1.txt
