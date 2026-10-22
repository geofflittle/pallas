"""Reads a hex Dijkstra transaction and prints the outer and sub transaction ids and the fields the v1 tests assert, with no pallas code."""
import hashlib
import sys


def head(b, i):
    ib = b[i]
    major, info = ib >> 5, ib & 31
    i += 1
    if info < 24:
        return major, info, i
    n = {24: 1, 25: 2, 26: 4, 27: 8}.get(info)
    if n is None:
        if info == 31:
            return major, None, i
        raise ValueError(f"bad info {info} at {i}")
    return major, int.from_bytes(b[i:i + n], "big"), i + n


def skip(b, i):
    """Returns the offset after the item at i."""
    major, arg, j = head(b, i)
    if major in (0, 1, 7):
        return j
    if major in (2, 3):
        if arg is None:
            while b[j] != 0xFF:
                j = skip(b, j)
            return j + 1
        return j + arg
    if major == 4:
        if arg is None:
            while b[j] != 0xFF:
                j = skip(b, j)
            return j + 1
        for _ in range(arg):
            j = skip(b, j)
        return j
    if major == 5:
        if arg is None:
            while b[j] != 0xFF:
                j = skip(b, skip(b, j))
            return j + 1
        for _ in range(arg):
            j = skip(b, skip(b, j))
        return j
    if major == 6:
        return skip(b, j)
    raise ValueError(major)


def items(b, i):
    """Returns the offsets of the items of the definite array at i, tags removed."""
    major, arg, j = head(b, i)
    while major == 6:
        major, arg, j = head(b, j)
    assert major == 4 and arg is not None, (major, arg)
    out = []
    for _ in range(arg):
        out.append(j)
        j = skip(b, j)
    return out


def map_entries(b, i):
    major, arg, j = head(b, i)
    assert major == 5 and arg is not None
    out = {}
    for _ in range(arg):
        k_major, k, kj = head(b, j)
        assert k_major == 0
        v = kj
        j = skip(b, v)
        out[k] = (v, j)
    return out


def uint(b, i):
    major, arg, _ = head(b, i)
    assert major == 0
    return arg


def bstr(b, i):
    major, arg, j = head(b, i)
    assert major == 2 and arg is not None
    return b[j:j + arg]


def inputs(b, i):
    return [(bstr(b, items(b, y)[0]).hex(), uint(b, items(b, y)[1])) for y in items(b, i)]


def blake(bs):
    return hashlib.blake2b(bs, digest_size=32).hexdigest()


def describe(b, body_start, body_end, label):
    body = map_entries(b, body_start)
    print(f"{label} id {blake(b[body_start:body_end])}")
    print(f"{label} body keys {sorted(body)}")
    if 0 in body:
        print(f"{label} inputs {inputs(b, body[0][0])}")
    if 1 in body:
        coins = []
        for o in items(b, body[1][0]):
            major, arg, j = head(b, o)
            if major == 4:
                coins.append(("array", uint(b, items(b, o)[1]) if head(b, items(b, o)[1])[0] == 0 else "multi"))
            else:
                m = map_entries(b, o)
                v = m[1][0]
                coins.append(uint(b, v) if head(b, v)[0] == 0 else uint(b, items(b, v)[0]))
        print(f"{label} output coins {coins}")
    if 2 in body:
        print(f"{label} fee {uint(b, body[2][0])}")
    if 11 in body:
        print(f"{label} script data hash {bstr(b, body[11][0]).hex()}")
    if 13 in body:
        print(f"{label} collateral {inputs(b, body[13][0])}")
    if 14 in body:
        print(f"{label} guards raw {b[body[14][0]:body[14][1]].hex()}")
    if 18 in body:
        print(f"{label} reference inputs {inputs(b, body[18][0])}")
    return body


def main():
    b = bytes.fromhex(open(sys.argv[1]).read().strip())
    tx = items(b, 0)
    print(f"tx items {len(tx)}")
    body_start = tx[0]
    body_end = skip(b, body_start)
    body = describe(b, body_start, body_end, "outer")
    for k, x in enumerate(tx[1:], 1):
        print(f"tx item {k} raw head {b[x:x + 1].hex()}")
    if 23 in body:
        for n, sub in enumerate(items(b, body[23][0])):
            sub_items = items(b, sub)
            s = sub_items[0]
            describe(b, s, skip(b, s), f"sub{n}")


if __name__ == "__main__":
    main()
