"""Prints the ledger view and one schema view of the certificates of one tx, side by side.

usage: python3 -I showcerts.py PREFIX INDEX SCHEMA
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import ledger_view, load_lines, schema_value, schema_view  # noqa: E402
from value import normalize  # noqa: E402

prefix, i, s = sys.argv[1], int(sys.argv[2]), sys.argv[3]
o = load_lines(f"{prefix}.oracle.jsonl")[i]
p = load_lines(f"{prefix}.probe.jsonl")[i]
lv = ledger_view(normalize(o["dump"]), s)["body"]["certs"]
sv = schema_view(schema_value("tx", p["schemas"][s]["msg"], s), s)["body"]["certs"]
print("ledger wrapper", list(lv) if isinstance(lv, dict) else type(lv))
print("schema wrapper", list(sv) if isinstance(sv, dict) else type(sv))
lv = list(lv.values())[0] if isinstance(lv, dict) else lv
sv = list(sv.values())[0] if isinstance(sv, dict) else sv
for n in range(max(len(lv), len(sv))):
    print(f"L{n}", json.dumps(lv[n] if n < len(lv) else None, sort_keys=True))
    print(f"S{n}", json.dumps(sv[n] if n < len(sv) else None, sort_keys=True))
