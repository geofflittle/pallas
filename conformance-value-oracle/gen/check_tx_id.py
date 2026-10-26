"""Checks that every schema table compares the transaction hash with the ledger's transaction id.

usage: python3 -I check_tx_id.py
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import compare_one, result, tables  # noqa: E402

ID = bytes(range(32))
OTHER = bytes(range(1, 33))


def message(schema, tx_hash):
    h = base64.b64encode(tx_hash).decode()
    return {"common": {"hash": h}} if schema == "v1" else {"hash": h}


def id_dis(schema, kind, tx_hash):
    """The disagreement keys of one comparison in which only the id is set on the ledger side."""
    msg, ledger = message(schema, tx_hash), {"id": "h:" + ID.hex()}
    if kind == "block":
        msg, ledger = {"body": {"tx": [msg]}}, {"txs": [ledger]}
    r = result()
    compare_one(kind, schema, ledger, msg, tables()[schema][kind], r, {})
    return {d["key"] for d in r["dis"]}


cases = []
for schema in ("v1alpha", "v1beta", "v1"):
    for kind, key in (("tx", "id"), ("body", "id"), ("block", "txs[].id")):
        # Must not fire: the hash is the ledger id.
        cases.append((f"{schema} {kind} same", key not in id_dis(schema, kind, ID)))
        # Must fire: the hash is another value.
        cases.append((f"{schema} {kind} other", key in id_dis(schema, kind, OTHER)))
bad = [name for name, ok in cases if not ok]
for name in bad:
    print(f"WRONG {name}")
print(f"transaction id rows: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
