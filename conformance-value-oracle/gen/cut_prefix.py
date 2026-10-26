"""Writes the first N blocks of an e2e_values.py walk, with their endorser transactions, as a walk of its own.

usage: python3 -I cut_prefix.py SRC DEST N
"""

import json
import os
import sys

src, dest, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
os.makedirs(dest, exist_ok=True)
ebs, kept, equal = 0, 0, 0
with open(f"{src}/served.jsonl") as served, open(f"{src}/block.hex") as blocks, open(f"{dest}/served.jsonl", "w") as so, open(
    f"{dest}/block.hex", "w"
) as bo:
    for i, (rec, hexline) in enumerate(zip(served, blocks)):
        if i >= n:
            break
        so.write(rec)
        bo.write(hexline)
        r = json.loads(rec)
        kept += 1
        ebs += len(r["eb"])
        equal += r["served_txs"] == r["elements"]
with open(f"{src}/tx.hex") as txs, open(f"{dest}/tx.hex", "w") as to:
    for i, line in enumerate(txs):
        if i >= ebs:
            break
        to.write(line)
with open(f"{src}/walk.json") as fh:
    sample = json.load(fh)["eb_sample"]
# The records hold the served and element counts, so the prefix carries that one walk check.
with open(f"{dest}/walk.json", "w") as fh:
    json.dump(
        dict(
            counts={"blocks": kept, "endorser transactions compared": ebs},
            checks={"served transactions equal the body elements": [equal, kept]},
            eb_sample=sample,
        ),
        fh,
    )
print(f"prefix blocks {kept}, endorser transactions {ebs}, served transactions equal the body elements in {equal} of {kept}")
