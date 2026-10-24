"""Counts disagreement records by class and by error text with positions blanked.

usage: python3 -I classes.py OUT_DIR [NAME ...]
"""

import collections
import json
import os
import re
import sys


def main():
    out = sys.argv[1]
    names = sys.argv[2:] or sorted(
        f[: -len(".dis.jsonl")] for f in os.listdir(out) if f.endswith(".dis.jsonl")
    )
    for name in names:
        counts = collections.Counter()
        total = 0
        for line in open(os.path.join(out, name + ".dis.jsonl")):
            d = json.loads(line)
            total += 1
            text = d.get("pallas") if isinstance(d.get("pallas"), str) else json.dumps(d.get("key"))
            text = re.sub(r"\d+", "N", str(text))[:80]
            counts[(d.get("cls"), text)] += 1
        print("==", name, total)
        for (cls, text), n in counts.most_common(40):
            print(f"{n:6} {cls} | {text}")


if __name__ == "__main__":
    main()
