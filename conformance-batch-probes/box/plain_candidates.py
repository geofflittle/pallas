"""Prints the shortest ledger accepted records that contain no indefinite length byte or text string and match a needle.

usage: python3 -I plain_candidates.py OUT_DIR NAME NEEDLE [LIMIT]

NEEDLE matches the pallas error text, or is "hex:PREFIX" to match an item that starts with PREFIX.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def indefinite_strings(b):
    s = Spans(b)
    s.item([])
    return sum(1 for _, st, _ in s.items if b[st] in (0x5F, 0x7F)), s


def main():
    out, name, needle = sys.argv[1], sys.argv[2], sys.argv[3]
    limit = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    gen = [json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()]
    oracle = [json.loads(x) for x in open(os.path.join(out, name + ".oracle.jsonl")) if x.strip()]
    probe = {}
    for x in open(os.path.join(out, name + ".probe.jsonl")):
        if x.strip():
            d = json.loads(x)
            probe[d["i"]] = d
    hits = []
    for k, g in enumerate(gen):
        if not oracle[k].get("ok"):
            continue
        b = bytes.fromhex(g["hex"])
        n, s = indefinite_strings(b)
        if n:
            continue
        p = probe.get(g["i"], {})
        err = str(p.get("err") or p.get("mapper_err") or "")
        if needle.startswith("hex:"):
            pre = bytes.fromhex(needle[4:])
            if not any(b[st:st + len(pre)] == pre for _, st, _ in s.items):
                continue
        elif needle not in err:
            continue
        hits.append((len(b), g["i"], err[:100]))
    hits.sort()
    print(name, needle, "hits", len(hits))
    for h in hits[:limit]:
        print(h)


if __name__ == "__main__":
    main()
