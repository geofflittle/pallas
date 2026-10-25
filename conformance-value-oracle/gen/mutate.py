"""Turns generated values into nearby inputs the ledger is expected to refuse.

usage: python3 -I mutate.py SEED < GEN > GEN

Each input line gives one output line with the same `i`, a `hex` holding the
mutated CBOR, `source` set to mutate and `mutation` naming the change. The
mutations change the structure the ledger decoder checks: a dropped or added
element, a duplicated or unknown map key, a swapped major type, a negated or
oversized integer, and trailing bytes.
"""

import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from value import Reader  # noqa: E402


def head(major, n):
    if n < 24:
        return bytes([major << 5 | n])
    for info, size in ((24, 1), (25, 2), (26, 4), (27, 8)):
        if n < 1 << (8 * size):
            return bytes([major << 5 | info]) + n.to_bytes(size, "big")
    raise ValueError("argument too large")


def enc(t):
    """The definite length encoding of one term as `Reader.item` returns it."""
    k = t[0]
    if k == "int":
        v = t[1]
        if v >= 1 << 64 or v < -(1 << 64):
            raw = (v if v >= 0 else -1 - v).to_bytes(((v if v >= 0 else -1 - v).bit_length() + 7) // 8, "big")
            return head(6, 2 if v >= 0 else 3) + head(2, len(raw)) + raw
        return head(0, v) if v >= 0 else head(1, -1 - v)
    if k == "bytes":
        return head(2, len(t[1])) + t[1]
    if k == "text":
        raw = t[1].encode()
        return head(3, len(raw)) + raw
    if k == "list":
        return head(4, len(t[1])) + b"".join(enc(x) for x in t[1])
    if k == "map":
        return head(5, len(t[1])) + b"".join(enc(a) + enc(b) for a, b in t[1])
    if k == "tag":
        return head(6, t[1]) + enc(t[2])
    if k == "bool":
        return bytes([0xF5 if t[1] else 0xF4])
    if k == "null":
        return b"\xf6"
    if k == "undefined":
        return b"\xf7"
    if k == "float":
        raw = bytes.fromhex(t[1])
        return bytes([0xE0 | {2: 25, 4: 26, 8: 27}[len(raw)]]) + raw
    if k == "simple":
        return head(7, t[1])
    raise ValueError(f"unknown term {k}")


def nodes(t, out):
    """Every term in the tree, parents before children."""
    out.append(t)
    if t[0] == "list":
        for x in t[1]:
            nodes(x, out)
    elif t[0] == "map":
        for a, b in t[1]:
            nodes(a, out)
            nodes(b, out)
    elif t[0] == "tag":
        nodes(t[2], out)
    return out


def copy(t):
    k = t[0]
    if k == "list":
        return ["list", [copy(x) for x in t[1]]]
    if k == "map":
        return ["map", [[copy(a), copy(b)] for a, b in t[1]]]
    if k == "tag":
        return ["tag", t[1], copy(t[2])]
    return list(t)


def mutate(t, rng):
    """One mutation of a copy of the term, with its name, or None when no node admits the chosen kind."""
    t = copy(t)
    every = nodes(t, [])
    kinds = ["drop", "extra", "dupkey", "unknownkey", "swaptype", "negate", "oversize", "trailing"]
    rng.shuffle(kinds)
    for kind in kinds:
        if kind == "trailing":
            return enc(t) + b"\x00", kind
        if kind in ("drop", "extra"):
            cands = [n for n in every if n[0] in ("list", "map") and (kind == "extra" or n[1])]
        elif kind in ("dupkey", "unknownkey"):
            cands = [n for n in every if n[0] == "map" and (kind == "unknownkey" or n[1])]
        elif kind == "swaptype":
            cands = [n for n in every if n[0] in ("int", "bytes")]
        else:
            cands = [n for n in every if n[0] == "int"]
        if not cands:
            continue
        n = rng.choice(cands)
        if kind == "drop":
            n[1].pop(rng.randrange(len(n[1])))
        elif kind == "extra":
            if n[0] == "list":
                n[1].append(["int", 0])
            else:
                n[1].append([["int", 1000], ["int", 0]])
        elif kind == "dupkey":
            a, b = n[1][rng.randrange(len(n[1]))]
            n[1].append([copy(a), copy(b)])
        elif kind == "unknownkey":
            n[1].append([["int", 1000], ["int", 0]])
        elif kind == "swaptype":
            if n[0] == "int":
                n[:] = ["bytes", abs(n[1]).to_bytes(8, "big")]
            else:
                n[:] = ["int", int.from_bytes(n[1][:8] or b"\x00", "big")]
        elif kind == "negate":
            n[1] = -1 - n[1]
        elif kind == "oversize":
            n[1] = (1 << 70) + abs(n[1])
        return enc(t), kind
    return None


def main():
    rng = random.Random(int(sys.argv[1]))
    for line in sys.stdin:
        if not line.strip():
            continue
        g = json.loads(line)
        r = Reader(bytes.fromhex(g["hex"]))
        out = mutate(r.item(), rng)
        raw, kind = out if out else (bytes.fromhex(g["hex"]), "none")
        print(json.dumps(dict(i=g["i"], hex=raw.hex(), source="mutate", mutation=kind, of=g["hex"]), sort_keys=True))


if __name__ == "__main__":
    main()
