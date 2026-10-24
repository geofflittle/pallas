"""Places several items cut from generated records into one fixed frame, so the ledger can decode them together.

usage: python3 -I compose.py OUT_DIR SPEC.json DEST_DIR

SPEC is a JSON list of records, each {"kind": KIND, "frame": HEX, "items":
[[NAME, J, PATH], ...]}, where each "{}" of HEX in turn takes the bytes of
one item, named as in frame.py. The records are written to
DEST_DIR/KIND.framed.gen.jsonl in order, for the ledger oracle to read.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def cut(out, name, j, path):
    for line in open(os.path.join(out, name + ".gen.jsonl")):
        g = json.loads(line)
        if g["i"] == j:
            b = bytes.fromhex(g["hex"])
            s = Spans(b)
            s.item([])
            spans = [(st, en) for p, st, en in s.items if p == path]
            if len(spans) != 1:
                raise SystemExit(f"{name} {j}: {len(spans)} items at {path}")
            return b[spans[0][0]:spans[0][1]]
    raise SystemExit(f"{name} {j}: no such record")


def main():
    out, spec, dest = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(dest, exist_ok=True)
    files = {}
    for i, rec in enumerate(json.load(open(spec))):
        parts = rec["frame"].split("{}")
        if len(parts) != len(rec["items"]) + 1:
            raise SystemExit(f"record {i}: {len(parts) - 1} slots for {len(rec['items'])} items")
        hexed = parts[0]
        for (name, j, path), rest in zip(rec["items"], parts[1:]):
            hexed += cut(out, name, j, path).hex() + rest
        kind = rec["kind"]
        if kind not in files:
            files[kind] = open(os.path.join(dest, kind + ".framed.gen.jsonl"), "w")
        files[kind].write(json.dumps({"i": i, "hex": hexed}) + "\n")
    for f in files.values():
        f.close()


if __name__ == "__main__":
    main()
