"""Checks that the attachment comparison fails exactly when a v1 item carries other than the ledger's redeemer at its index.

usage: python3 -I check_attach.py
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import attach  # noqa: E402

TXID = "33" * 32
KEY = "e0" + "11" * 28
SCRIPT = "f0" + "22" * 28


def b64(hexstr):
    return base64.b64encode(bytes.fromhex(hexstr)).decode()


def ledger(redeemers, inputs=2):
    """A raw ledger dump with two inputs, a key and a script withdrawal in the ledger's order, and the given redeemers."""
    return {
        "body": {
            "inputs": {"set": [{"cbor": f"825820{TXID}{n:02x}"} for n in range(inputs)]},
            "withdrawals": {"map": [[{"cbor": "581d" + SCRIPT}, {"cbor": "00"}], [{"cbor": "581d" + KEY}, {"cbor": "00"}]]},
            "certs": {"oset": []},
        },
        "wits": {
            "redeemers": {
                "map": [[{"cbor": f"82{p:02x}{i:02x}"}, {"data": {"tag": "int", "v": str(v)}, "units": {"cbor": "820000"}}] for p, i, v in redeemers]
            }
        },
    }


PURPOSE = {0: "REDEEMER_PURPOSE_SPEND", 3: "REDEEMER_PURPOSE_REWARD"}


def red(p, i, v):
    return {"purpose": PURPOSE[p], "index": i, "payload": {"bigInt": {"int": str(v)}}, "exUnits": {"memory": 0, "steps": 0}}


def message(inputs, withdrawals):
    """A v1 message whose inputs and withdrawals carry the given redeemers, withdrawals keyed by account."""
    return {
        "common": {
            "inputs": [{"txHash": b64(TXID), "outputIndex": n, "redeemer": r} for n, r in enumerate(inputs)],
            "withdrawals": [{"rewardAccount": b64(acct), "coin": {"int": "0"}, "redeemer": r} for acct, r in withdrawals],
        }
    }


FULL = [(0, 0, 10), (0, 1, 11), (3, 0, 30)]

cases = [
    # Must not fire: each item carries the redeemer at its ledger index, the script account first.
    ("ledger order", ledger(FULL), message([red(0, 0, 10), red(0, 1, 11)], [(KEY, None), (SCRIPT, red(3, 0, 30))]), 0),
    # Must not fire: no redeemers and no item carries one.
    ("no redeemers", ledger([]), message([None, None], [(KEY, None), (SCRIPT, None)]), 0),
    # Must fire twice: the reward redeemer on the key account, the first by raw bytes, and none on the script account.
    ("raw byte order", ledger(FULL), message([red(0, 0, 10), red(0, 1, 11)], [(KEY, red(3, 0, 30)), (SCRIPT, None)]), 2),
    # Must fire twice: each input carries the redeemer of the next index.
    ("shifted spend", ledger(FULL), message([red(0, 1, 11), None], [(KEY, None), (SCRIPT, red(3, 0, 30))]), 2),
    # Must fire once: an input carries a redeemer the ledger does not have.
    ("extra redeemer", ledger([(3, 0, 30)]), message([red(0, 0, 10), None], [(KEY, None), (SCRIPT, red(3, 0, 30))]), 1),
    # Must fire once: same index and purpose, other data.
    ("other data", ledger(FULL), message([red(0, 0, 99), red(0, 1, 11)], [(KEY, None), (SCRIPT, red(3, 0, 30))]), 1),
]
bad = []
for name, raw, msg, want in cases:
    got = sum(1 for v, *_ in attach("tx", raw, msg) if v == "differs")
    if got != want:
        bad.append((name, want, got))
for name, want, got in bad:
    print(f"WRONG {name}: want {want} differing items, got {got}")
print(f"redeemer attachment: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
