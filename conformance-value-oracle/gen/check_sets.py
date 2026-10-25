"""Checks how the schema comparison pairs set entries a schema list repeats.

usage: python3 -I check_sets.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import Diffs, generic, nset, sdiff  # noqa: E402


def diffs(ls, ss):
    out = Diffs(guard_whole=False)
    sdiff(nset(ls), nset(ss), "scripts", out)
    return sorted(generic(p) for p, _, _ in out)


def mof(m):
    return {"tag": "native", "script": {"tag": "m_of", "m": m, "scripts": []}}


cases = [
    # Must not fire: the schema list repeats a member the ledger set holds once.
    ([mof("2")], [mof("2"), mof("2")], []),
    ([mof("2"), mof("3")], [mof("3"), mof("2"), mof("3")], []),
    # Must fire: two ledger members the schema type made equal each differ in the narrowed field.
    ([mof("5000000000"), mof("6000000000")], [mof("4294967295"), mof("4294967295")], ["scripts{}.script.m", "scripts{}.script.m"]),
    # Must fire: a member the schema lacks, with no repeat to stand for it.
    ([mof("2"), mof("3")], [mof("2")], ["scripts{}"]),
    # Must fire: a repeat stands for one missing member only.
    ([mof("2"), mof("3"), mof("4")], [mof("2"), mof("2")], ["scripts{}", "scripts{}.script.m"]),
]
bad = [(ls, ss, want, diffs(ls, ss)) for ls, ss, want in cases if diffs(ls, ss) != want]
for ls, ss, want, got in bad:
    print(f"WRONG want {want} got {got}")
print(f"set repeats: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
