"""Prints the shortest items, in ledger accepted records, that sit at a matching path, start with an indefinite array or map, and hold no indefinite byte or text string.

usage: python3 -I sub_candidates.py OUT_DIR NAME PATH_REGEX [LIMIT]
"""

import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def main():
    out, name, pattern = sys.argv[1], sys.argv[2], sys.argv[3]
    limit = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    gen = [json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()]
    oracle = [json.loads(x) for x in open(os.path.join(out, name + ".oracle.jsonl")) if x.strip()]
    hits = []
    for k, g in enumerate(gen):
        if not oracle[k].get("ok"):
            continue
        b = bytes.fromhex(g["hex"])
        s = Spans(b)
        s.item([])
        strings = [st for st in [x for _, x, _ in s.items] + s.keys if b[st] in (0x5F, 0x7F)]
        for p, st, en in s.items:
            if not re.fullmatch(pattern, p) or b[st] not in (0x9F, 0xBF):
                continue
            if any(st <= x < en for x in strings):
                continue
            hits.append((en - st, g["i"], p, b[st:en].hex()[:80]))
    hits.sort()
    print(name, pattern, "hits", len(hits))
    for h in hits[:limit]:
        print(h)


if __name__ == "__main__":
    main()
