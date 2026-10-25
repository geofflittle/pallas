"""Prints one example of each disagreement class in the given dis files.

usage: python3 -I showdis.py FILE...
"""

import json
import sys

seen = set()
for f in sys.argv[1:]:
    for line in open(f):
        d = json.loads(line)
        k = (d["cls"], d.get("key"))
        if k in seen:
            continue
        seen.add(k)
        print(f.split("/")[-1], d["cls"], d.get("path", d.get("key", ""))[:140])
        for side in ("ledger", "schema", "pallas"):
            if side in d:
                print("   ", side[0].upper(), json.dumps(d[side])[:400])
        print("    hex", d["hex"][:200])
