"""Check cut headers against the node's secondary index.

Usage: python3 secondary.py <chunk> <block_number> <out_file> [<block_number> <out_file>]...
For each named block, prints its index in the chunk, the slot the secondary
index records, whether the header span the index records equals the one a CBOR
walk finds, whether its header hash equals blake2b-256 of those bytes, and the
block CRC32, then writes the header as lowercase hex with no trailing newline.
"""
import hashlib
import struct
import sys
import zlib

import cbor2

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from parents import blocks, header_span  # noqa: E402

chunk = sys.argv[1]
wanted = dict(zip(map(int, sys.argv[2::2]), sys.argv[3::2]))
buf = open(chunk, "rb").read()
sec = open(chunk.replace(".chunk", ".secondary"), "rb").read()
entries = [struct.unpack(">QHHI32sQ", sec[i:i + 56]) for i in range(0, len(sec), 56)]

offset = 0
for index, block in enumerate(blocks(buf)):
    s, e = header_span(block)
    hdr = block[s:e]
    number = cbor2.loads(hdr)[0][0]
    boff, hoff, hsize, crc, hhash, slot = entries[index]
    if number in wanted:
        ok_span = boff == offset and hoff == s and hsize == e - s
        ok_hash = hhash == hashlib.blake2b(hdr, digest_size=32).digest()
        ok_crc = crc == zlib.crc32(block)
        print(f"block={number} chunk_index={index} slot={slot} bytes={len(hdr)} "
              f"hash={hhash.hex()} span_ok={ok_span} hash_ok={ok_hash} crc_ok={ok_crc}")
        with open(wanted[number], "w") as f:
            f.write(hdr.hex())
    offset += len(block)
