"""Counts indefinite length items in one CBOR file, given as its path."""
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


def item(b, i, count):
    major, arg, i = head(b, i)
    if major in (0, 1, 7):
        return i
    if major == 6:
        return item(b, i, count)
    if arg is None:
        count[major] = count.get(major, 0) + 1
        while b[i] != 0xFF:
            i = item(b, i, count) if major in (4, 5) else item(b, i, count)
            if major == 5:
                i = item(b, i, count)
        return i + 1
    if major in (2, 3):
        return i + arg
    for _ in range(arg * (2 if major == 5 else 1)):
        i = item(b, i, count)
    return i


data = open(sys.argv[1], "rb").read()
count = {}
end = item(data, 0, count)
print(sys.argv[1], "consumed", end, "of", len(data), "indefinite by major type", count)
