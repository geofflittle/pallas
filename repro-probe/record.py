#!/usr/bin/env python3
"""Records endorser block fixture values without pallas.

For each fixture: the body's own byte length and blake2b-256, the number of
map entries the body declares, the number of lines in the .ebtxs file, and for
the first transaction its entry key, the blake2b-256 of the unwrapped
transaction, and its transaction id, the blake2b-256 of the transaction body
span. Prints one line per fixture.
"""
import hashlib
import sys

ROOT = sys.argv[1]


def head(b, i):
    ib = b[i]
    major, info = ib >> 5, ib & 0x1F
    i += 1
    if info < 24:
        return major, info, i
    if info == 24:
        return major, b[i], i + 1
    if info == 25:
        return major, int.from_bytes(b[i:i + 2], "big"), i + 2
    if info == 26:
        return major, int.from_bytes(b[i:i + 4], "big"), i + 4
    if info == 27:
        return major, int.from_bytes(b[i:i + 8], "big"), i + 8
    if info == 31:
        return major, None, i
    raise ValueError(f"info {info}")


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
    if major == 4:
        if arg is None:
            while b[i] != 0xFF:
                i = skip(b, i)
            return i + 1
        for _ in range(arg):
            i = skip(b, i)
        return i
    if major == 5:
        if arg is None:
            while b[i] != 0xFF:
                i = skip(b, skip(b, i))
            return i + 1
        for _ in range(arg):
            i = skip(b, skip(b, i))
        return i
    if major == 6:
        return skip(b, i)
    raise ValueError(major)


def blake(x):
    return hashlib.blake2b(x, digest_size=32).hexdigest()


for name in ("dijkstra-eb1", "dijkstra-eb2", "dijkstra-eb3"):
    body = bytes.fromhex(open(f"{ROOT}/{name}.ebbody").read().strip())
    lines = open(f"{ROOT}/{name}.ebtxs").read().split()
    major, n, i = head(body, 0)
    assert major == 5 and n is not None
    kmaj, klen, ki = head(body, i)
    assert kmaj == 2 and klen == 32
    first_key = body[ki:ki + 32].hex()
    vmaj, first_size, _ = head(body, ki + 32)
    assert vmaj == 0
    wire = bytes.fromhex(lines[0])
    wmaj, wlen, wi = head(wire, 0)
    assert wmaj == 2 and wi + wlen == len(wire)
    inner = wire[wi:]
    amaj, alen, ai = head(inner, 0)
    assert amaj == 4
    body_end = skip(inner, ai)
    print(
        f"{name} body_bytes={len(body)} body_hash={blake(body)} entries={n} "
        f"ebtxs_lines={len(lines)} first_key={first_key} first_size={first_size} "
        f"first_inner_len={len(inner)} first_inner_hash={blake(inner)} "
        f"first_tx_elements={alen} first_tx_id={blake(inner[ai:body_end])}"
    )
