"""Recompute the endorser block fixture values with cbor2 and hashlib.

Usage: python3 ebvalues.py <pallas worktree>
"""
import hashlib
import sys

import cbor2

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from parents import head, skip  # noqa: E402


def blake(b):
    return hashlib.blake2b(b, digest_size=32).hexdigest()


root = sys.argv[1]
for n in (1, 2, 3):
    body = bytes.fromhex(open(f"{root}/test_data/dijkstra-eb{n}.ebbody").read().strip())
    entries = list(cbor2.loads(body).items())
    lines = open(f"{root}/test_data/dijkstra-eb{n}.ebtxs").read().split()
    inners = [cbor2.loads(bytes.fromhex(l)) for l in lines]
    ids = []
    for inner in inners:
        _, _, i = head(inner, 0)
        ids.append(blake(inner[i:skip(inner, i)]))
    keys_ok = all(k.hex() == blake(t) for (k, _), t in zip(entries, inners))
    sizes_ok = all(s == len(t) for (_, s), t in zip(entries, inners))
    arity = sorted({head(t, 0)[1] for t in inners})
    print(f"eb{n} bytes={len(body)} blake={blake(body)} count={len(entries)} lines={len(lines)} "
          f"first_key={entries[0][0].hex()} first_size={entries[0][1]} first_id={ids[0]} "
          f"keys_ok={keys_ok} sizes_ok={sizes_ok} arity={arity}")
    if n == 3:
        k, s = entries[391]
        print(f"eb3 entry391 key={k.hex()} size={s} id={ids[391]}")
        change = next(i for i in range(len(entries) - 1) if entries[i][1] != entries[i + 1][1])
        print(f"eb3 first_size_change index={change} sizes={entries[change][1]},{entries[change + 1][1]}")
