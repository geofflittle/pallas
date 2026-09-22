#!/usr/bin/env python3
"""Writes a cardano-cli text envelope for chosen transactions of a harvested
endorser block, removing the CBOR byte string envelope each one arrives in.

usage: txid-envelopes.py <ebtxs file> <out dir> <index>...
"""
import json
import os
import sys


def unwrap(raw: bytes) -> bytes:
    major = raw[0] >> 5
    info = raw[0] & 0x1F
    assert major == 2, "a delivered transaction is a byte string"
    if info < 24:
        n, off = info, 1
    elif info == 24:
        n, off = raw[1], 2
    elif info == 25:
        n, off = int.from_bytes(raw[1:3], "big"), 3
    elif info == 26:
        n, off = int.from_bytes(raw[1:5], "big"), 5
    else:
        raise ValueError("unsupported length")
    assert off + n == len(raw), "no bytes after the byte string"
    return raw[off : off + n]


def main() -> None:
    path, out = sys.argv[1], sys.argv[2]
    lines = open(path).read().split()
    os.makedirs(out, exist_ok=True)
    for i in (int(a) for a in sys.argv[3:]):
        inner = unwrap(bytes.fromhex(lines[i]))
        env = {"type": "Tx DijkstraEra", "description": "", "cborHex": inner.hex()}
        with open(os.path.join(out, f"tx-{i}.json"), "w") as f:
            json.dump(env, f)
        print(f"index {i} inner_bytes {len(inner)}")


main()
