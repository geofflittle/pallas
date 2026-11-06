"""Checks which pallas results on a ledger-accepted value the value comparison counts as disagreements.

usage: python3 -I check_listing.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from valuecmp import compare  # noqa: E402

ERR = "unexpected type indefinite bytes at position 2: expected bytes (definite length)"


def row(hexstr, ledger, pallas):
    """One aligned value the ledger accepts, with the ledger dump `ledger` and the pallas result `pallas`."""
    return ({"hex": hexstr}, {"ok": True, "dump": ledger, "reencode": hexstr}, pallas, {"ok": True})


def classes(r):
    totals, agree, differ, listed, accepted, dis = compare("ppu", [r])
    return sorted(d["cls"] for d in dis), sorted(w for w, _ in listed)


cases = [
    # Must not fire: pallas holds the set the ledger holds.
    ("same set", row("a0", {"x": {"set": ["1", "2"]}}, {"ok": True, "dump": {"x": {"set": ["2", "1"]}}}), ([], [])),
    # Must not fire: an ordered set and a map pallas holds as the ledger does.
    ("same oset and map", row("a0", {"o": {"oset": ["1"]}, "m": {"map": [["1", "2"]]}}, {"ok": True, "dump": {"o": {"oset": ["1"]}, "m": {"map": [["1", "2"]]}}}), ([], [])),
    # Must fire: pallas holds a set element twice that the ledger, refusing duplicates, holds once.
    ("duplicate set element", row("a0", {"x": {"set": ["1"]}}, {"ok": True, "dump": {"x": {"set": ["1", "1"]}}}), (["pallas holds a duplicate the ledger refuses"], [])),
    ("duplicate oset element", row("a0", {"o": {"oset": ["1"]}}, {"ok": True, "dump": {"o": {"oset": ["1", "1"]}}}), (["pallas holds a duplicate the ledger refuses"], [])),
    ("duplicate map key", row("a0", {"m": {"map": [["1", "2"]]}}, {"ok": True, "dump": {"m": {"map": [["1", "2"], ["1", "2"]]}}}), (["pallas holds a duplicate the ledger refuses"], [])),
    # Must fire: pallas refuses tag 24 over indefinite bytes the ledger accepts.
    ("tag 24 over indefinite bytes", row("d8185f4101ff", {}, {"ok": False, "err": ERR}), (["pallas refuses ledger-valid input"], [])),
]
bad = [(name, want, classes(r)) for name, r, want in cases if classes(r) != want]
for name, want, got in bad:
    print(f"WRONG {name}: want {want}, got {got}")
print(f"value listing: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
