#!/bin/sh
# Writes the two rc-2026w42 block fixtures as lowercase hex with no trailing newline.
# Usage: regen-w42-blocks.sh SAMPLES_RB_DIR PALLAS_TREE
set -eu
rb=$1
tree=$2
for pair in "00007_slot245 7" "00011_slot413 11"; do
  set -- $pair
  xxd -p "$rb/rb_$1.cbor" | tr -d '\n' > "$tree/test_data/dijkstra-rc-2026w42-$2.block"
done
