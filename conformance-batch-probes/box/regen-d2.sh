#!/bin/sh
# Writes the header version info block fixture from the generated records in out-base into the directory named by the first argument.
set -eu
cd /opt/build/pallas-conformance-1024
mkdir -p "$1"
python3 -I -c 'import json,sys
rows=[json.loads(l) for l in open("out-base/block.cddl-plain.gen.jsonl") if l.strip()]
sys.stdout.write(rows[95]["hex"])' > "$1/block-header-version-info.hex"
