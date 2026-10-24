"""Prints the shortest ledger accepted records whose pallas error text contains a needle.

usage: python3 -I candidates.py OUT_DIR NAME NEEDLE [LIMIT]

Each line gives the record index, the input length in bytes, the pallas error and
the oracle facts, so a record can be picked as a test vector.
"""

import json
import os
import sys


def lines(path):
    with open(path) as f:
        return [json.loads(x) for x in f if x.strip()]


def main():
    out, name, needle = sys.argv[1], sys.argv[2], sys.argv[3]
    limit = int(sys.argv[4]) if len(sys.argv) > 4 else 5
    gen = lines(os.path.join(out, name + ".gen.jsonl"))
    oracle = lines(os.path.join(out, name + ".oracle.jsonl"))
    probe = {d["i"]: d for d in lines(os.path.join(out, name + ".probe.jsonl"))}
    hits = []
    for i, g in enumerate(gen):
        o = oracle[i] if i < len(oracle) else {}
        p = probe.get(g["i"], {})
        err = str(p.get("err") or p.get("mapper_err") or "")
        if needle in err or (needle == "*hex*" and sys.argv[5] in g["hex"]):
            hits.append((len(g["hex"]) // 2, g["i"], err[:110], o.get("ok"), json.dumps(o.get("facts"))[:400]))
    hits.sort()
    print(name, needle, "hits", len(hits))
    for h in hits[:limit]:
        print(h)


if __name__ == "__main__":
    main()
