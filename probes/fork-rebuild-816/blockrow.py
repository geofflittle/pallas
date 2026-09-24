"""Print slot, block number and header hash of a `[era, block]` fixture, read only."""
import hashlib
import subprocess
import sys


def head(b, i):
    ib = b[i]
    mt = ib >> 5
    ai = ib & 31
    i += 1
    if ai < 24:
        return mt, ai, i
    n = {24: 1, 25: 2, 26: 4, 27: 8}.get(ai)
    if n is None:
        return mt, None, i
    return mt, int.from_bytes(b[i:i + n], "big"), i + n


def skip(b, i):
    mt, v, i = head(b, i)
    if mt in (0, 1, 7):
        return i
    if mt in (2, 3):
        if v is None:
            while b[i] != 0xFF:
                i = skip(b, i)
            return i + 1
        return i + v
    if mt == 6:
        return skip(b, i)
    k = 2 if mt == 5 else 1
    if v is None:
        while b[i] != 0xFF:
            i = skip(b, i)
        return i + 1
    for _ in range(v * k):
        i = skip(b, i)
    return i


def row(rev, name):
    text = subprocess.run(
        ["/usr/bin/git", "-C", "/Users/geofflittle/code/pallas", "show", f"{rev}:test_data/{name}"],
        check=True, capture_output=True, text=True,
    ).stdout
    b = bytes.fromhex(text.strip())
    _, _, i = head(b, 0)          # outer array
    i = skip(b, i)                # era
    _, _, i = head(b, i)          # block array
    start = i
    end = skip(b, i)              # header
    header = b[start:end]
    _, _, j = head(header, 0)     # header array
    _, _, j = head(header, j)     # header body array
    _, number, j = head(header, j)
    _, slot, j = head(header, j)
    digest = hashlib.blake2b(header, digest_size=32).hexdigest()
    print(f"| {name} | {number} | {slot} | `{digest}` |")


if __name__ == "__main__":
    for n in sys.argv[2:]:
        row(sys.argv[1], n)
