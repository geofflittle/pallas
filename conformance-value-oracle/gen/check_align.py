"""Checks how the schema comparison pairs ordered set entries.

usage: python3 -I check_align.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import Diffs, sdiff  # noqa: E402


def diffs(ls, ss):
    out = Diffs(guard_whole=False)
    sdiff({"oset": ls}, {"oset": ss}, "certs", out)
    return [p for p, _, _ in out]


pool = {"tag": "pool_reg", "bls": None, "cost": "5"}
pool_bls = dict(pool, bls=["h:aa", "h:bb"])
cases = [
    # Must not fire: the ledger dropped a repeated member the schema list still holds.
    ([["4", "a"], ["7", "b"]], [["4", "a"], ["4", "a"], ["7", "b"]], []),
    ([["4", "a"]], [["4", "a"], ["4", "a"]], []),
    # Two ledger members equal in the schema view stay paired by position.
    ([pool, ["4", "a"], pool_bls], [pool, ["4", "a"], pool], ["certs[2].bls"]),
    # Must fire: a repeat with no surplus is compared to the ledger member at its position.
    ([["4", "a"], ["7", "b"]], [["4", "a"], ["4", "a"]], ["certs[1][0]", "certs[1][1]"]),
    # Must fire: a member the schema lacks.
    ([["4", "a"], ["7", "b"]], [["4", "a"]], ["certs[1]"]),
]
bad = [(ls, ss, want, diffs(ls, ss)) for ls, ss, want in cases if diffs(ls, ss) != want]
for ls, ss, want, got in bad:
    print(f"WRONG want {want} got {got}")
print(f"oset alignment: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
