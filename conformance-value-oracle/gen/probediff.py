"""Counts, per top level key, the lines on which two probe outputs over the same input differ.

usage: python3 -I probediff.py OLD NEW
"""

import json
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from value import rows  # noqa: E402


def main():
    old, new = sys.argv[1], sys.argv[2]
    differ, keys, n = Counter(), Counter(), 0
    for n, (a, b) in enumerate(rows(old, new), 1):
        for k in set(a) | set(b):
            keys[k] += 1
            if json.dumps(a.get(k), sort_keys=True) != json.dumps(b.get(k), sort_keys=True):
                differ[k] += 1
    for k in sorted(keys):
        print(f"{k}: {keys[k]} lines, {differ[k]} differ")
    print(f"probe diff: {n} lines")


if __name__ == "__main__":
    main()
