"""Prints one item of a generated record by its exact path, with its length and whether it holds an indefinite string anywhere, map keys included.

usage: python3 -I cutitem.py OUT_DIR NAME I PATH
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from compose import cut  # noqa: E402


def indefinite_strings(b):
    """Counts the indefinite byte and text strings in one CBOR item."""
    count = 0
    p = 0

    def arg(info):
        nonlocal p
        if info < 24:
            return info
        n = {24: 1, 25: 2, 26: 4, 27: 8}[info]
        v = int.from_bytes(b[p:p + n], "big")
        p += n
        return v

    def item():
        nonlocal p, count
        ib = b[p]
        p += 1
        major, info = ib >> 5, ib & 31
        if major in (0, 1):
            arg(info)
        elif major in (2, 3):
            if info == 31:
                count += 1
                while b[p] != 0xFF:
                    item()
                p += 1
            else:
                n = arg(info)
                p += n
        elif major in (4, 5):
            width = 1 if major == 4 else 2
            if info == 31:
                while b[p] != 0xFF:
                    for _ in range(width):
                        item()
                p += 1
            else:
                for _ in range(arg(info) * width):
                    item()
        elif major == 6:
            arg(info)
            item()
        elif info in (24, 25, 26, 27):
            p += {24: 1, 25: 2, 26: 4, 27: 8}[info]

    item()
    if p != len(b):
        raise SystemExit(f"item ends at {p} of {len(b)}")
    return count


def main():
    out, name, i, path = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
    b = cut(out, name, i, path)
    n = indefinite_strings(b)
    print(len(b), "chunked" if n else "clean", b.hex())


if __name__ == "__main__":
    main()
