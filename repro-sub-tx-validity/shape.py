"""Raw CBOR walk of a block fixture: indefinite headers and tag 258 count."""
import sys


def head(b, i):
    ib = b[i]
    major, ai = ib >> 5, ib & 31
    i += 1
    if ai < 24:
        return major, ai, ai, i
    if ai == 31:
        return major, ai, None, i
    n = {24: 1, 25: 2, 26: 4, 27: 8}[ai]
    return major, ai, int.from_bytes(b[i:i + n], "big"), i + n


def walk(b, i, stats):
    major, ai, val, i = head(b, i)
    if major in (0, 1, 7):
        return i
    if major in (2, 3):
        if val is None:
            stats["indef"] += 1
            while b[i] != 0xFF:
                i = walk(b, i, stats)
            return i + 1
        return i + val
    if major in (4, 5):
        per = 1 if major == 4 else 2
        if val is None:
            stats["indef"] += 1
            while b[i] != 0xFF:
                i = walk(b, i, stats)
            return i + 1
        for _ in range(val * per):
            i = walk(b, i, stats)
        return i
    if major == 6:
        if val == 258:
            stats["tag258"] += 1
        return walk(b, i, stats)
    raise ValueError(major)


for path in sys.argv[1:]:
    data = bytes.fromhex(open(path).read().strip())
    stats = {"indef": 0, "tag258": 0}
    end = walk(data, 0, stats)
    print(path.rsplit("/", 1)[-1], "bytes", len(data), "end", end, stats)
