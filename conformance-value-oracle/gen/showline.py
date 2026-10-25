"""Prints the ledger view and each schema view of one value, cut to one top level field.

usage: python3 -I showline.py KIND PREFIX INDEX FIELD [SCHEMA...]
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import ledger_view, schema_value, schema_view, load_lines  # noqa: E402
from value import normalize  # noqa: E402

kind, prefix, i, field = sys.argv[1:5]
schemas = sys.argv[5:] or ["v1alpha", "v1beta", "v1"]
i = int(i)
o = load_lines(f"{prefix}.oracle.jsonl")[i]
p = load_lines(f"{prefix}.probe.jsonl")[i]


def pick(v, field):
    for part in field.split("."):
        if part:
            v = v.get(part) if isinstance(v, dict) else None
    return v


ln = normalize(o["dump"])
for s in schemas:
    lv = pick(ledger_view(ln, s), field)
    sv = pick(schema_view(schema_value(kind, p["schemas"][s]["msg"], s), s), field)
    print(f"== {s} ledger")
    print(json.dumps(lv, indent=1, sort_keys=True)[:6000])
    print(f"== {s} schema")
    print(json.dumps(sv, indent=1, sort_keys=True)[:6000])
