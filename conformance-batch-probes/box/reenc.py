"""Prints, for each re-encoding the ledger refuses, the input path where the re-encoding first departs from the input.

usage: python3 -I reenc.py DIS.jsonl [LIMIT]
"""

import collections
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from where import Walk  # noqa: E402


def main():
    path = sys.argv[1]
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    shown = collections.defaultdict(list)
    for line in open(path):
        d = json.loads(line)
        if d.get("cls") != "ledger refuses the pallas re-encoding":
            continue
        a = bytes.fromhex(d["hex"])
        b = bytes.fromhex(d.get("reencode_hex", ""))
        pos = next((i for i in range(min(len(a), len(b))) if a[i] != b[i]), min(len(a), len(b)))
        # walk back to the start of the innermost item containing pos
        w = Walk(a)
        best = None
        for start in range(pos, -1, -1):
            w = Walk(a)
            try:
                w.item([], start)
            except Exception:  # noqa: BLE001
                continue
            if w.hits:
                best = (start, w.hits[0])
                break
        sig = " ".join(best[1][0]) if best else "?"
        shape = re.sub(r"\[\d+\]", "[i]", sig)
        shown[shape].append(d["i"])
        if len(shown[shape]) <= limit:
            print(d["i"], pos, sig, "in:", a[pos - 4:pos + 8].hex(), "out:", b[pos - 4:pos + 8].hex())
    print("== shapes")
    for k, v in sorted(shown.items(), key=lambda x: -len(x[1])):
        print(len(v), k, v[:8])


if __name__ == "__main__":
    main()
