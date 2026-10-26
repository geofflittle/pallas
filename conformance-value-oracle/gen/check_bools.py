"""Checks that a schema bool member agrees with the ledger value only when it is true.

usage: python3 -I check_bools.py
"""

import base64
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import compare_one, result, tables  # noqa: E402

KEY = bytes(range(28))


def ppu_dis(key, value, msg):
    r = result()
    compare_one("ppu", "v1", {"m": {f'"{key}"': value}}, msg, tables()["v1"]["ppu"], r, {})
    return len(r["dis"])


def cert_dis(drep_leaf, drep_msg):
    ledger = {"certs": {"oset": [["9", ["0", "h:" + KEY.hex()], drep_leaf]]}}
    msg = {"certificates": [{"voteDelegCert": {"stakeCredential": {"addrKeyHash": base64.b64encode(KEY).decode()}, "drep": drep_msg}}]}
    r = result()
    compare_one("body", "v1beta", ledger, msg, tables()["v1beta"]["body"], r, {})
    return [d["key"] for d in r["dis"]]


cases = [
    # Must not fire: the member is true and the ledger holds the value it stands for.
    ("no limit true", ppu_dis(38, None, {"maxPledgeLeverage": {"noLimit": True}}) == 0),
    ("unset true", ppu_dis(53, None, {"perasBootstrapRound": {"unset": True}}) == 0),
    ("abstain true", "certs[][]" not in cert_dis(["2"], {"abstain": True})),
    ("no confidence true", "certs[][]" not in cert_dis(["3"], {"noConfidence": True})),
    # Must fire: the member is false, which stands for no value the ledger holds.
    ("no limit false", ppu_dis(38, None, {"maxPledgeLeverage": {"noLimit": False}}) == 1),
    ("unset false", ppu_dis(53, None, {"perasBootstrapRound": {"unset": False}}) == 1),
    ("abstain false", "certs[][]" in cert_dis(["2"], {"abstain": False})),
    ("no confidence false", "certs[][]" in cert_dis(["3"], {"noConfidence": False})),
]
bad = [name for name, ok in cases if not ok]
for name in bad:
    print(f"WRONG {name}")
print(f"bool members: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
