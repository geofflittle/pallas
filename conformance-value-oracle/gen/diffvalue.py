"""Prints the first path where the pallas re-encoding of a generated value differs from the input.

usage: python3 -I diffvalue.py GEN.jsonl PROBE.jsonl [LINE]
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compare as c  # noqa: E402


def short(v):
    return repr(v)[:240]


def diff(a, b, path):
    if a == b:
        return None
    if isinstance(a, tuple) and isinstance(b, tuple) and a and b and a[0] == b[0]:
        if a[0] == "map":
            da, db = dict(a[1]), dict(b[1])
            if len(da) != len(a[1]) or len(db) != len(b[1]):
                return (path, f"duplicate keys: input {len(a[1])} entries {len(da)} keys, pallas {len(b[1])} entries {len(db)} keys")
            for k in sorted(set(da) | set(db), key=repr):
                if k not in da or k not in db:
                    return (path + [short(k)], f"key only in {'input' if k in da else 'pallas'}")
                d = diff(da[k], db[k], path + [short(k)])
                if d:
                    return d
        if a[0] == "list":
            if len(a[1]) != len(b[1]):
                return (path, f"list length input {len(a[1])} pallas {len(b[1])}")
            for n, (x, y) in enumerate(zip(a[1], b[1])):
                d = diff(x, y, path + [n])
                if d:
                    return d
    return (path, f"input {short(a)} pallas {short(b)}")


def main():
    gen = [json.loads(x) for x in open(sys.argv[1]) if x.strip()]
    probe = [json.loads(x) for x in open(sys.argv[2]) if x.strip()]
    only = int(sys.argv[3]) if len(sys.argv) > 3 else None
    for n, (g, p) in enumerate(zip(gen, probe)):
        if only is not None and n != only:
            continue
        if not p.get("ok") or p.get("reencode_equal"):
            continue
        d = diff(c.cbor(g["hex"]), c.cbor(p["reencode_hex"]), [])
        print(n, "same value" if d is None else d)


if __name__ == "__main__":
    main()
