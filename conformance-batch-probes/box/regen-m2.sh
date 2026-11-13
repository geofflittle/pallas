#!/bin/sh
# Rebuilds the trailing null fixtures from the generated records in out-base, into the directory named by the first argument.
# GEN names the harness dir conformance-value-oracle/gen whose build-image.sh names the image, needed when this script runs from a copy.
set -eu
IMAGE=$(sh "${GEN:-$(dirname "$0")/../../conformance-value-oracle/gen}/build-image.sh" conformance-gen-hs)
cd /opt/build/pallas-conformance-1024
DEST=$1
W=/opt/build/conformance-gen-1024
rm -rf out-framed-m2
python3 -I compose.py out-base compose-m2.json out-framed-m2
python3 -I -c 'import json,sys
for l in sys.stdin:
    if l.strip(): print(json.loads(l)["hex"])' < out-framed-m2/body.framed.gen.jsonl \
  | docker run --rm -i --user "$(id -u):$(id -g)" -v $W:/work "$IMAGE" /work/bin/conformance-gen oracle body \
  > out-framed-m2/body.framed.oracle.jsonl
python3 -I regen_fixtures.py . "$DEST" manifest-m2.txt
