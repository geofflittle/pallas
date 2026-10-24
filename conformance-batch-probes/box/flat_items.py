"""Prints the item pallas refused for each flat enum disagreement, with its path and whether it holds an indefinite string.

usage: python3 -I flat_items.py DIS.jsonl [LIMIT]

A map key is printed with the path of its map and a trailing "key".
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


class KeySpans(Spans):
    def key(self):
        start = self.p
        k = super().key()
        self.items.append(("key@" + str(start), start, self.p))
        return k


def main():
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 50
    shown = 0
    for line in open(sys.argv[1]):
        r = json.loads(line)
        err = str(r.get("pallas_err") or r.get("err") or r)
        if "flat enum" not in err:
            continue
        m = re.search(r"position (\d+)", err)
        if not m:
            continue
        at = int(m.group(1))
        b = bytes.fromhex(r["hex"])
        s = KeySpans(b)
        s.item([])
        hits = [(p, st, en) for p, st, en in s.items if st == at]
        hits.sort(key=lambda x: x[2] - x[1], reverse=True)
        if not hits:
            print(r["i"], at, "no item")
            continue
        path, st, en = hits[0]
        inner = [p for p, a, z in s.items if a >= st and z <= en and p.endswith("chunk")]
        clean = "clean" if not inner else "chunked"
        print(r["i"], at, clean, en - st, path, "|", b[st:en].hex()[:160])
        shown += 1
        if shown >= limit:
            break


if __name__ == "__main__":
    main()
