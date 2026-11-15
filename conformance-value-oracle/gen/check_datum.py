"""Checks how the comparer reads an output datum: its form from the output's own bytes where the schema carries them, and its bytes against its hash.

usage: python3 -I check_datum.py
"""

import base64
import hashlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import expected  # noqa: E402
from schemacmp import datum, h, listed  # noqa: E402


def b64(b):
    return base64.b64encode(b).decode()


BYTES = b"\x18\x1f"
OTHER = b"\x18\x1e"
HASH_RAW = hashlib.blake2b(BYTES, digest_size=32).digest()
HASH = b64(HASH_RAW)
PAYLOAD = {"bigInt": {"int": "31"}}

# Post Alonzo outputs {0: address, 1: coin, 2: datum option}, and an Alonzo output [address, coin, datum hash].
HASH_OUT = b64(b"\xa3\x00\x41\x00\x01\x01\x02\x82\x00\x58\x20" + HASH_RAW)
INLINE_OUT = b64(b"\xa3\x00\x41\x00\x01\x01\x02\x82\x01\xd8\x18\x42" + BYTES)
LEGACY_OUT = b64(b"\x83\x41\x00\x01\x58\x20" + HASH_RAW)

CARRIED = {"hash": HASH, "payload": PAYLOAD, "originalCbor": b64(BYTES)}

# The must not fire cases come first: each reads as the form the ledger states.
cases = [
    ("inline, output bytes say inline", CARRIED, INLINE_OUT, "inline"),
    ("inline, no output bytes", CARRIED, None, "inline"),
    ("witnessed hash, output bytes say hash", CARRIED, HASH_OUT, "hash"),
    ("witnessed hash, Alonzo output", CARRIED, LEGACY_OUT, "hash"),
    ("resolved hash without bytes", {"hash": HASH, "payload": PAYLOAD}, HASH_OUT, "hash"),
    ("unresolved hash", {"hash": HASH}, HASH_OUT, "hash"),
    ("none", {}, None, None),
    # Must fire: bytes that are not the preimage of the hash read as neither form.
    ("bytes of another datum", {"hash": HASH, "payload": PAYLOAD, "originalCbor": b64(OTHER)}, HASH_OUT, "bytes of another datum"),
    ("bytes of another datum, no output bytes", {"hash": HASH, "payload": PAYLOAD, "originalCbor": b64(OTHER)}, None, "bytes of another datum"),
]
bad = []
for name, d, out, want in cases:
    got = datum(d, out)
    tag = None if got is None else got["tag"]
    if tag != want or (tag == "hash" and got["hash"] != h(HASH)):
        bad.append((name, want, got))

LEDGER_HASH = {"tag": "hash", "hash": h(HASH)}
READ_INLINE = datum(CARRIED, None)
# A v1alpha output carries no bytes of its own, so a witnessed hash datum reads as inline and is listed there only.
listings = [
    ("v1alpha witnessed hash", "v1alpha", LEDGER_HASH, READ_INLINE, expected.DATUM_FORM),
    ("v1 witnessed hash", "v1", LEDGER_HASH, READ_INLINE, None),
    ("v1beta witnessed hash", "v1beta", LEDGER_HASH, READ_INLINE, None),
    ("v1alpha inline read as hash", "v1alpha", READ_INLINE, LEDGER_HASH, None),
]
for name, schema, lv, sv, want in listings:
    got = listed(schema, "body.outputs[].datum", lv, sv)
    if got != want:
        bad.append((name, want, got))
for name, want, got in bad:
    print(f"WRONG {name}: want {want}, got {got}")
print(f"output datum: {len(cases) + len(listings)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
