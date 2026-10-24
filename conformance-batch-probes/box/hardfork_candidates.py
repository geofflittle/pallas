"""Prints proposals, in ledger accepted records, whose hard fork initiation holds an indefinite protocol version and no indefinite byte or text string.

usage: python3 -I hardfork_candidates.py OUT_DIR NAME [LIMIT]
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
            m = re.fullmatch(r"(.*\{20\}(?: tag258)? \[\d+\]) \[2\] \[2\]", p)
            if not m or b[st] != 0x9F:
                continue
            ga = by_path[m.group(1) + " [2]"]
            tag = by_path[m.group(1) + " [2] [0]"]
            if b[tag[0]:tag[1]] != b"\x01":
                continue
            pst, pen = by_path[m.group(1)]
            if any(pst <= x < pen for x in strings):
                continue
            hits.append((pen - pst, g["i"], m.group(1), b[ga[0]:ga[1]].hex()[:80]))
    hits.sort()
    print(name, "hits", len(hits))
    for h in hits[:limit]:
        print(h)


if __name__ == "__main__":
    main()
