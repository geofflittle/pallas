"""Split immutable chunks into blocks and print each Dijkstra header's fields.

Usage: python3 parents.py <chunk>... [--cut <block_number> <out_file>]...
Prints one line per block: block number, slot, header hash, previous hash,
certifies flag, announcement. With --cut, writes that block's header CBOR as
lowercase hex with no trailing newline.
"""
import hashlib
import sys

import cbor2


def head(buf, i):
    ib = buf[i]
    major, info = ib >> 5, ib & 0x1F
    i += 1
    if info < 24:
        return major, info, i
    if info == 24:
        return major, buf[i], i + 1
    if info == 25:
        return major, int.from_bytes(buf[i:i + 2], "big"), i + 2
    if info == 26:
        return major, int.from_bytes(buf[i:i + 4], "big"), i + 4
    if info == 27:
        return major, int.from_bytes(buf[i:i + 8], "big"), i + 8
    if info == 31:
        return major, None, i
    raise ValueError(f"bad info {info} at {i}")


def skip(buf, i):
    major, arg, i = head(buf, i)
    if major in (0, 1, 7):
        return i
    if major in (2, 3):
        if arg is None:
            while buf[i] != 0xFF:
                i = skip(buf, i)
            return i + 1
        return i + arg
    if major in (4, 5):
        n = arg
        per = 1 if major == 4 else 2
        if n is None:
            while buf[i] != 0xFF:
                i = skip(buf, i)
            return i + 1
        for _ in range(n * per):
            i = skip(buf, i)
        return i
    if major == 6:
        return skip(buf, i)
    raise ValueError(major)


def blocks(buf):
    i = 0
    while i < len(buf):
        j = skip(buf, i)
        yield buf[i:j]
        i = j


def header_span(block):
    _, _, i = head(block, 0)  # era envelope
    _, _, i = head(block, i)  # era tag
    _, _, i = head(block, i)  # block array
    return i, skip(block, i)


def main():
    args = sys.argv[1:]
    cuts = {}
    chunks = []
    while args:
        a = args.pop(0)
        if a == "--cut":
            cuts[int(args.pop(0))] = args.pop(0)
        else:
            chunks.append(a)
    for path in chunks:
        buf = open(path, "rb").read()
        for block in blocks(buf):
            s, e = header_span(block)
            hdr = block[s:e]
            body = cbor2.loads(hdr)[0]
            if len(body) < 12:
                continue
            number, slot, prev = body[0], body[1], body[2]
            h = hashlib.blake2b(hdr, digest_size=32).hexdigest()
            ann = body[11]
            ann_s = "nil" if ann is None else f"{ann[0].hex()}/{ann[1]}"
            print(f"{path} block={number} slot={slot} hash={h} prev={prev.hex() if prev else None} certifies={body[10]} announces={ann_s}")
            if number in cuts:
                with open(cuts[number], "w") as f:
                    f.write(hdr.hex())
                print(f"CUT block={number} bytes={len(hdr)} to {cuts[number]}")


if __name__ == "__main__":
    main()
