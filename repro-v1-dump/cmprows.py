#!/usr/bin/env python3
# Compares two row files line by line as parsed JSON, so key order is ignored.
import json
import sys

a_path, b_path = sys.argv[1], sys.argv[2]
blocks = rows = diffs = 0
shown = 0
with open(a_path) as a, open(b_path) as b:
    for n, (la, lb) in enumerate(zip(a, b), 1):
        if la.startswith("B "):
            blocks += 1
            if la != lb:
                diffs += 1
                if shown < 3:
                    print(f"line {n} block differs: {la.strip()} | {lb.strip()}")
                    shown += 1
            continue
        rows += 1
        ja, jb = json.loads(la), json.loads(lb)
        if ja != jb:
            diffs += 1
            if shown < 3:
                keys = sorted(k for k in set(ja) | set(jb) if ja.get(k) != jb.get(k))
                print(f"line {n} differs in {keys}")
                for k in keys:
                    print(f"  a {k}: {json.dumps(ja.get(k))[:300]}")
                    print(f"  b {k}: {json.dumps(jb.get(k))[:300]}")
                shown += 1
    extra = a.readline() or b.readline()
print(f"blocks {blocks} rows {rows} differing {diffs} trailing_extra {bool(extra)}")
print("IDENTICAL" if diffs == 0 and not extra else "DIFFER")
