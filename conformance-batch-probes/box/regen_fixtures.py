"""Writes the generated test vectors named in a manifest as hex files, and the oracle facts beside them.

usage: python3 -I regen_fixtures.py OUT_DIR DEST_DIR MANIFEST

Each manifest line is "FILE NAME I [PATH]": FILE is the hex file to write under
DEST_DIR, NAME the kind and source (for example ppu.cddl), I the record index,
and PATH an optional item path in the form where.py prints, selecting one item
inside the record. The record must be one the ledger decoded. The oracle facts
of every whole record are written to DEST_DIR/oracle-facts.json.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def records(out, name):
    gen = [json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()]
    oracle = [json.loads(x) for x in open(os.path.join(out, name + ".oracle.jsonl")) if x.strip()]
    return {g["i"]: (g, oracle[k]) for k, g in enumerate(gen)}


def main():
    out, dest, manifest = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(dest, exist_ok=True)
    cache = {}
    facts = {}
    for line in open(manifest):
        parts = line.split(None, 3)
        if not parts or parts[0].startswith("#"):
            continue
        file, name, i = parts[0], parts[1], int(parts[2])
        path = parts[3].strip() if len(parts) > 3 else None
        if name not in cache:
            cache[name] = records(out, name)
        g, o = cache[name][i]
        if not o.get("ok"):
            raise SystemExit(f"{name} {i}: the ledger refused this record")
        b = bytes.fromhex(g["hex"])
        if path:
            s = Spans(b)
            s.item([])
            spans = [(st, en) for p, st, en in s.items if p == path]
            if len(spans) != 1:
                raise SystemExit(f"{name} {i}: {len(spans)} items at {path}")
            b = b[spans[0][0]:spans[0][1]]
        else:
            facts[file] = o.get("facts")
        with open(os.path.join(dest, file), "w") as f:
            f.write(b.hex())
        print(file, len(b))
    with open(os.path.join(dest, "oracle-facts.json"), "w") as f:
        json.dump(facts, f, indent=1, sort_keys=True)
        f.write("\n")


if __name__ == "__main__":
    main()
