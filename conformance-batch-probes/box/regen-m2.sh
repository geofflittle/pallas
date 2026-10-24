#!/bin/sh
# Rebuilds the trailing null fixtures from the generated records in out-base, into the directory named by the first argument.
set -eu
cd /opt/build/pallas-conformance-1024
DEST=$1
W=/opt/build/conformance-gen-1024
rm -rf out-framed-m2
python3 -I compose.py out-base compose-m2.json out-framed-m2
python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])' < out-framed-m2/body.framed.gen.jsonl \
  | docker run --rm -i --user "$(id -u):$(id -g)" -v $W:/work conformance-gen-hs:2 /work/bin/conformance-gen oracle body \
  > out-framed-m2/body.framed.oracle.jsonl
python3 -I regen_fixtures.py . "$DEST" manifest-m2.txt
