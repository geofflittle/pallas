"""Writes the given values of one kind: the hex of each listed file and the built values, without repeats.

usage: python3 -I given.py KIND FILE... > OUT/KIND.given.gen.jsonl
A FILE ending in .jsonl.gz is a ledger corpus, one record per line with the transaction hex in `tx`.
Any other FILE holds one hex value. For the kind tx the built values follow the files.
"""

import gzip
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mutate import enc  # noqa: E402


def i(n):
    return ["int", n]


def b(raw):
    return ["bytes", raw]


def tagged_set(items):
    return ["tag", 258, ["list", items]]


KEY_ACCOUNT = bytes([0xE0]) + bytes([0x11] * 28)
SCRIPT_ACCOUNT = bytes([0xF0]) + bytes([0x22] * 28)


def redeemer(purpose, index, datum):
    return [["list", [i(purpose), i(index)]], ["list", [i(datum), ["list", [i(0), i(0)]]]]]


def reward_order_tx():
    """A transaction with a key and a script withdrawal, two inputs and a redeemer for each input and for withdrawal index 0."""
    body = [
        [i(0), tagged_set([["list", [b(bytes([0x33] * 32)), i(0)]], ["list", [b(bytes([0x33] * 32)), i(1)]]])],
        [i(1), ["list", [["list", [b(bytes([0x60]) + bytes([0x44] * 28)), i(1000000)]]]]],
        [i(2), i(200000)],
        [i(5), ["map", [[b(KEY_ACCOUNT), i(0)], [b(SCRIPT_ACCOUNT), i(0)]]]],
    ]
    wits = [[i(5), ["map", [redeemer(0, 0, 10), redeemer(0, 1, 11), redeemer(3, 0, 30)]]]]
    return enc(["list", [["map", body], ["map", wits], ["null"]]]).hex()


BUILT = {"tx": [("built:reward-order", reward_order_tx)]}


def bare(kind, hexstr):
    """The block of a node file `[era, block]`, by its bytes, or the value unchanged for another kind or form."""
    raw = bytes.fromhex(hexstr)
    if kind == "block" and len(raw) > 2 and raw[0] == 0x82 and raw[1] < 0x18:
        return raw[2:].hex()
    return hexstr


def values(kind, files):
    for path in files:
        if path.endswith(".jsonl.gz"):
            with gzip.open(path, "rt") as fh:
                for n, line in enumerate(fh):
                    if line.strip():
                        yield f"{os.path.basename(path)}#{n}", json.loads(line)["tx"]
        else:
            with open(path) as fh:
                yield os.path.basename(path), bare(kind, fh.read().strip())
    for origin, build in BUILT.get(kind, []):
        yield origin, build()


def main():
    kind, files = sys.argv[1], sys.argv[2:]
    seen, n = set(), 0
    for origin, hexstr in values(kind, files):
        if hexstr in seen:
            continue
        seen.add(hexstr)
        print(json.dumps(dict(i=n, source="given", origin=origin, hex=hexstr), sort_keys=True))
        n += 1


if __name__ == "__main__":
    main()
