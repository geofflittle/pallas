#!/usr/bin/env python3
"""Prints the entry count and the first few (key, size) entries of an endorser
block body, read with a minimal CBOR walker rather than with pallas.

usage: ebentries.py <ebbody file> [n]
"""
import sys


def head(b: bytes, i: int):
    major, info = b[i] >> 5, b[i] & 0x1F
    if info < 24:
        return major, info, i + 1
    width = {24: 1, 25: 2, 26: 4, 27: 8}[info]
    return major, int.from_bytes(b[i + 1 : i + 1 + width], "big"), i + 1 + width


def main() -> None:
    body = bytes.fromhex(open(sys.argv[1]).read().strip())
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    major, count, i = head(body, 0)
    assert major == 5, "a definite map"
    print(f"entries {count}")
    for k in range(count):
        m, klen, i = head(body, i)
        assert m == 2
        key = body[i : i + klen].hex()
        i += klen
        m, size, i = head(body, i)
        assert m == 0
        if k < n:
            print(f"{k} {key} {size}")
    assert i == len(body), "no bytes after the map"


main()
