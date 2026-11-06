"""Checks that vtally.py fails a run whose expected difference table has an entry matching no value or lacks a listed class.

usage: python3 -I check_expected.py
"""

import json
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import expected  # noqa: E402
import vtally  # noqa: E402


def write_run(d, value_listed, schema_listed):
    """One kind and source with the given listed classes and no disagreement."""
    counts = dict(totals={"values": 1}, agree={}, differ={}, listed=[[w, "", 1] for w in value_listed], accepted_invalid={})
    json.dump(counts, open(os.path.join(d, "tx.arb.counts.json"), "w"))
    open(os.path.join(d, "tx.arb.dis.jsonl"), "w").close()
    s = {name: dict(agree={}, nofield={}, cannot=[[w, "", 1] for w in schema_listed], compared=1, attach={}) for name in ("v1alpha", "v1beta", "v1")}
    json.dump(s, open(os.path.join(d, "tx.arb.schema.counts.json"), "w"))
    open(os.path.join(d, "tx.arb.schema.dis.jsonl"), "w").close()


cases = [
    # Must not fire: every entry matched once.
    ("all matched", list(expected.VALUE), list(expected.SCHEMA), 0),
    # Must fire: one value entry matched nothing.
    ("value entry unmatched", list(expected.VALUE[1:]), list(expected.SCHEMA), 1),
    # Must fire: one schema entry matched nothing.
    ("schema entry unmatched", list(expected.VALUE), list(expected.SCHEMA[:-1]), 1),
    # Must fire: a class the table does not hold.
    ("unknown class", list(expected.VALUE) + ["a class nobody declared"], list(expected.SCHEMA), 1),
]
bad = []
devnull = open(os.devnull, "w")
for name, vl, sl, want in cases:
    with tempfile.TemporaryDirectory() as d:
        write_run(d, vl, sl)
        out, sys.stdout = sys.stdout, devnull
        try:
            got = vtally.main(d)
        finally:
            sys.stdout = out
    if got != want:
        bad.append((name, want, got))
for name, want, got in bad:
    print(f"WRONG {name}: want exit {want}, got {got}")
print(f"expected difference table: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
