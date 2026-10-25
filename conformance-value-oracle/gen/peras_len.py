"""Counts, per output directory, the Peras certificate byte lengths of the blocks with a peras mapper disagreement.

usage: python3 -I peras_len.py OUTDIR
A block is [header, [transactions, leios, peras]], either array in either length form.
"""
import collections
import json
import sys


def head(b, i):
    ib = b[i]
    major, info = ib >> 5, ib & 31
    i += 1
    if info < 24:
        return major, info, i
    if info == 31:
        return major, None, i
    n = {24: 1, 25: 2, 26: 4, 27: 8}[info]
    return major, int.from_bytes(b[i:i + n], "big"), i + n


def skip(b, i):
    major, arg, i = head(b, i)
    if major in (0, 1, 7):
        return i
    if major in (2, 3):
        if arg is None:
            while b[i] != 0xFF:
                i = skip(b, i)
            return i + 1
        return i + arg
    if major == 6:
        return skip(b, i)
    count = arg * (2 if major == 5 and arg is not None else 1) if arg is not None else None
    if count is None:
        while b[i] != 0xFF:
            i = skip(b, i)
        return i + 1
    for _ in range(count):
        i = skip(b, i)
    return i


def bytes_len(b, i):
    major, arg, j = head(b, i)
    if major == 7 and b[i] == 0xF6:
        return "null"
    if major != 2:
        return f"major {major}"
    if arg is not None:
        return arg
    total = 0
    while b[j] != 0xFF:
        _, n, j = head(b, j)
        total += n
        j += n
    return total


def peras(b):
    _, _, i = head(b, 0)
    i = skip(b, i)
    _, _, i = head(b, i)
    i = skip(b, i)
    i = skip(b, i)
    return bytes_len(b, i)


out = sys.argv[1]
for f in ["block.arb", "block.cddl-plain", "block.cddl"]:
    bad = set()
    for line in open(f"{out}/{f}.dis.jsonl"):
        d = json.loads(line)
        if d["cls"] == "mapper fact" and d["key"] == "peras_certificate":
            bad.add(d["i"])
    lengths = collections.Counter()
    others = collections.Counter()
    for line in open(f"{out}/{f}.gen.jsonl"):
        g = json.loads(line)
        n = peras(bytes.fromhex(g["hex"]))
        (lengths if g["i"] in bad else others)[n == 0] += 1
    print(f, "disagreeing, empty:", dict(lengths), "agreeing, empty:", dict(others))
