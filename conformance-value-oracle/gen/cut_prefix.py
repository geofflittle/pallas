"""Writes the first N blocks of an e2e_values.py walk, with their endorser transactions, as a walk of its own.

usage: python3 -I cut_prefix.py SRC DEST N
"""

import json
import os
import sys

src, dest, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
os.makedirs(dest, exist_ok=True)
ebs, kept = 0, 0
with open(f"{src}/served.jsonl") as served, open(f"{src}/block.hex") as blocks, open(f"{dest}/served.jsonl", "w") as so, open(
    f"{dest}/block.hex", "w"
) as bo:
    for i, (rec, hexline) in enumerate(zip(served, blocks)):
        if i >= n:
            break
        so.write(rec)
        bo.write(hexline)
        kept += 1
        ebs += len(json.loads(rec)["eb"])
with open(f"{src}/tx.hex") as txs, open(f"{dest}/tx.hex", "w") as to:
    for i, line in enumerate(txs):
        if i >= ebs:
            break
        to.write(line)
print(f"prefix blocks {kept}, endorser transactions {ebs}")
