"""Turns generated values into nearby inputs, by default ones the ledger is expected to refuse.

usage: python3 -I mutate.py SEED [MODE] < GEN > GEN

Each input line gives one output line with the same `i`, a `hex` holding the
mutated CBOR, `source` set to the mode and `mutation` naming the change. The
default mode `mutate` changes the structure the ledger decoder checks: a
dropped or added element, a duplicated or unknown map key, a swapped major
type, a negated or oversized integer, and trailing bytes. The mode `empty14`
sets the guards (key 14) of a transaction body and of each sub transaction body
to an empty tagged set. The mode `subkeyhash` rewrites each sub transaction's
guards of credentials as the set of their hashes.
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


GUARDS = ["int", 14]
SUB_TXS = ["int", 23]


def bodies(t):
    """The top body map and each sub transaction body map of a transaction or a body term."""
    top = t[1][0] if t[0] == "list" and t[1] and t[1][0][0] == "map" else t
    if top[0] != "map":
        return None, []
    subs = []
    for k, v in top[1]:
        if k == SUB_TXS:
            inner = v[2] if v[0] == "tag" else v
            if inner[0] == "list":
                subs += [s[1][0] for s in inner[1] if s[0] == "list" and s[1] and s[1][0][0] == "map"]
    return top, subs


def set_key(m, key, value):
    for pair in m[1]:
        if pair[0] == key:
            pair[1] = value
            return
    m[1].append([key, value])


def empty14(t):
    """The term with every body's guards an empty tagged set, and whether it changed."""
    t = copy(t)
    top, subs = bodies(t)
    if top is None:
        return t, False
    for b in [top] + subs:
        set_key(b, GUARDS, ["tag", 258, ["list", []]])
    return t, True


def subkeyhash(t):
    """The term with each sub transaction's credential guards rewritten as their hashes, and whether any changed."""
    t = copy(t)
    _, subs = bodies(t)
    changed = False
    for b in subs:
        for pair in b[1]:
            if pair[0] != GUARDS:
                continue
            v = pair[1]
            inner = v[2] if v[0] == "tag" else v
            creds = inner[1] if inner[0] == "list" else []
            if creds and all(c[0] == "list" and len(c[1]) == 2 and c[1][1][0] == "bytes" for c in creds):
                inner[1] = [c[1][1] for c in creds]
                changed = True
    return t, changed


def main():
    rng = random.Random(int(sys.argv[1]))
    mode = sys.argv[2] if len(sys.argv) > 2 else "mutate"
    for line in sys.stdin:
        if not line.strip():
            continue
        g = json.loads(line)
        t = Reader(bytes.fromhex(g["hex"])).item()
        if mode == "mutate":
            out = mutate(t, rng)
            raw, kind = out if out else (bytes.fromhex(g["hex"]), "none")
        elif mode in ("empty14", "subkeyhash"):
            new, changed = (empty14 if mode == "empty14" else subkeyhash)(t)
            raw, kind = enc(new), mode if changed else "none"
        else:
            sys.exit(f"unknown mode {mode}")
        print(json.dumps(dict(i=g["i"], hex=raw.hex(), source=mode, mutation=kind, of=g["hex"]), sort_keys=True))


if __name__ == "__main__":
    main()
