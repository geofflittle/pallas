# Print the blake2b 256 hash of the raw body bytes of a Dijkstra transaction
# fixture and of each sub transaction body under its body key 23, read from the
# encoded spans, independent of pallas.
# usage: python3 -I subtx_hash.py <path to test_data/dijkstra-subtx.tx>
# expected:
#   top 74e2116ca6e0c809f156aa062a9d4ee8b156322618846f1bdea5d9ad7a206a12
#   sub 0 63ac433eefaed71cf0fa2c7134f82ea38f3c6c338b3d5129c686c47b44162691
import hashlib
import sys


def head(b, i):
    """Return (major type, argument, offset after the head) for the item at i."""
    major, info = b[i] >> 5, b[i] & 0x1F
    i += 1
    if info < 24:
        return major, info, i
    size = {24: 1, 25: 2, 26: 4, 27: 8}.get(info)
    if size is None:
        if info == 31:
            return major, None, i
        raise ValueError(f"unsupported additional info {info} at {i - 1}")
    return major, int.from_bytes(b[i : i + size], "big"), i + size


def skip(b, i):
    """Return the offset just past the item starting at i."""
    major, arg, j = head(b, i)
    if major in (0, 1, 7):
        return j
    if major in (2, 3):
        if arg is None:
            while b[j] != 0xFF:
                j = skip(b, j)
            return j + 1
        return j + arg
    if major == 6:
        return skip(b, j)
    count = arg if major == 4 else (None if arg is None else 2 * arg)
    if count is None:
        while b[j] != 0xFF:
            j = skip(b, j)
        return j + 1
    for _ in range(count):
        j = skip(b, j)
    return j


def items(b, i):
    """Yield the start offsets of the elements of the array at i, past any tag."""
    major, arg, j = head(b, i)
    while major == 6:
        major, arg, j = head(b, j)
    assert major == 4, f"expected an array at {i}"
    n = 0
    while (arg is None and b[j] != 0xFF) or (arg is not None and n < arg):
        yield j
        j = skip(b, j)
        n += 1


def map_entries(b, i):
    """Yield (key, value offset) for the map at i, with unsigned integer keys."""
    major, arg, j = head(b, i)
    assert major == 5, f"expected a map at {i}"
    for _ in range(arg):
        kmajor, key, k = head(b, j)
        assert kmajor == 0
        yield key, k
        j = skip(b, k)


def blake(b):
    return hashlib.blake2b(b, digest_size=32).hexdigest()


def main():
    tx = bytes.fromhex(open(sys.argv[1]).read().strip())
    body = next(items(tx, 0))
    print("top", blake(tx[body : skip(tx, body)]))
    for key, value in map_entries(tx, body):
        if key == 23:
            for n, sub in enumerate(items(tx, value)):
                sub_body = next(items(tx, sub))
                print("sub", n, blake(tx[sub_body : skip(tx, sub_body)]))


main()
