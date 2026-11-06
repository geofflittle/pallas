"""Checks that the comparer reads a v1 output datum as inline only when it carries its original bytes, so a hash datum the mapper resolved from the witnesses stays a hash.

usage: python3 -I check_datum.py
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import datum, h  # noqa: E402

HASH = base64.b64encode(bytes([0xE5] * 32)).decode()
PAYLOAD = {"bigInt": {"int": "31"}}

cases = [
    # Inline: the mapper writes the hash of the bytes, the data and the bytes.
    ("inline", {"hash": HASH, "payload": PAYLOAD, "originalCbor": base64.b64encode(b"\x18\x1f").decode()}, "inline"),
    # A hash datum whose data the witnesses hold.
    ("resolved hash", {"hash": HASH, "payload": PAYLOAD}, "hash"),
    # A hash datum whose data the witnesses lack.
    ("unresolved hash", {"hash": HASH}, "hash"),
    # No datum.
    ("none", {}, None),
]
bad = []
for name, d, want in cases:
    got = datum(d)
    tag = None if got is None else got["tag"]
    if tag != want or (tag == "hash" and got["hash"] != h(HASH)):
        bad.append((name, want, got))
for name, want, got in bad:
    print(f"WRONG {name}: want {want}, got {got}")
print(f"output datum: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
