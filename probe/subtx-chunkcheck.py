"""Check a fixture's chunk and secondary index entry against the node's immutable db, read only."""
import struct
import sys
import zlib

DB = "/opt/musashi/data/db/immutable"
chunk, entry, fixture = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
sec = open(f"{DB}/{chunk:05d}.secondary", "rb").read()
data = open(f"{DB}/{chunk:05d}.chunk", "rb").read()
rows = [sec[i:i + 56] for i in range(0, len(sec), 56)]
off, _hoff, _hsize, crc, hhash, slot = struct.unpack(">QHHI32sQ", rows[entry])
end = struct.unpack(">Q", rows[entry + 1][:8])[0] if entry + 1 < len(rows) else len(data)
block = data[off:end]
want = bytes.fromhex(open(fixture).read().strip())
print(
    f"chunk {chunk:05d}/{entry} slot {slot} hash {hhash.hex()} bytes {len(block)}"
    f" crc_ok {zlib.crc32(block) == crc} equal_fixture {block == want}"
)
