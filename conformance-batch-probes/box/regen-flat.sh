#!/bin/sh
# Rebuilds the indefinite flat enum fixtures from the generated records in out-base, into the directory named by the first argument.
set -eu
cd /opt/build/pallas-conformance-1024
DEST=$1
W=/opt/build/conformance-gen-1024
rm -rf out-framed-flat
python3 -I compose.py out-base compose-flat.json out-framed-flat
for k in body wits; do
  python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])' < out-framed-flat/$k.framed.gen.jsonl \
    | docker run --rm -i --user "$(id -u):$(id -g)" -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen oracle $k \
    > out-framed-flat/$k.framed.oracle.jsonl
done
python3 -I regen_fixtures.py . "$DEST" manifest-flat.txt
