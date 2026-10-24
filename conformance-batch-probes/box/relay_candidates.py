"""Prints pool registration certificates, in ledger accepted records, that hold an indefinite relay array and no indefinite byte or text string.

usage: python3 -I relay_candidates.py OUT_DIR NAME [LIMIT]
"""

import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def main():
    out, name = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 5
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
        by_path = {p: (st, en) for p, st, en in s.items}
        for p, st, en in s.items:
            m = re.fullmatch(r"(.*\{4\}(?: tag258)? \[\d+\]) \[8\](?: tag258)? \[\d+\]", p)
            if not m or b[st] != 0x9F:
                continue
            cst, cen = by_path[m.group(1)]
            if b[cst + 1] != 0x03 or any(cst <= x < cen for x in strings):
                continue
            hits.append((cen - cst, g["i"], m.group(1), p, b[st:en].hex()[:60]))
    hits.sort()
    print(name, "hits", len(hits))
    for h in hits[:limit]:
        print(h)


if __name__ == "__main__":
    main()
