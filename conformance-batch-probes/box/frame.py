"""Places items cut from generated records into fixed frames, so the ledger can decode each item inside a value of a kind it knows.

usage: python3 -I frame.py OUT_DIR SPEC DEST_DIR

Each SPEC line is "I KIND NAME J PATH | FRAME": I the index of the framed record,
KIND the kind the frame is (body, tx, wits), NAME and J the generated record,
PATH the item path in the form where.py prints, or "-" for the whole record,
and FRAME a hex template whose "{}" the item bytes replace. NAME "-" marks a
record written by hand, whose FRAME is the whole value. A "^" before PATH
replaces the outer array header of the item with an indefinite one. The framed
records are written to DEST_DIR/KIND.framed.gen.jsonl, for the ledger oracle to
read, and the source of each line to DEST_DIR/KIND.framed.src.jsonl.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402


def indefinite(b):
    """Rewrites a definite array header as an indefinite one with a closing break."""
    ib = b[0]
    if ib >> 5 != 4 or ib & 31 == 31:
        raise SystemExit("not a definite array")
    info = ib & 31
    skip = 1 + ({24: 1, 25: 2, 26: 4, 27: 8}.get(info, 0))
    return bytes([0x9F]) + b[skip:] + bytes([0xFF])


def main():
    out, spec, dest = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(dest, exist_ok=True)
    files = {}
    for line in open(spec):
        if not line.strip() or line.startswith("#"):
            continue
        head, frame = line.split("|")
        i, kind, name, j, path = head.split(None, 4)
        path = path.strip()
        if name == "-":
            b = b""
        else:
            recs = {json.loads(x)["i"]: json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()}
            b = bytes.fromhex(recs[int(j)]["hex"])
        reframe = path.startswith("^")
        path = path.lstrip("^").strip()
        if path != "-":
            s = Spans(b)
            s.item([])
            spans = [(st, en) for p, st, en in s.items if p == path]
            if len(spans) != 1:
                raise SystemExit(f"{name} {j}: {len(spans)} items at {path}")
            b = b[spans[0][0]:spans[0][1]]
        if reframe:
            b = indefinite(b)
        hexed = frame.strip().replace("{}", b.hex())
        if kind not in files:
            files[kind] = (open(os.path.join(dest, kind + ".framed.gen.jsonl"), "w"),
                           open(os.path.join(dest, kind + ".framed.src.jsonl"), "w"))
        files[kind][0].write(json.dumps({"i": int(i), "hex": hexed}) + "\n")
        files[kind][1].write(json.dumps({"i": int(i), "name": name, "j": int(j), "path": head.split(None, 4)[4].strip(), "frame": frame.strip()}) + "\n")
    for g, s in files.values():
        g.close()
        s.close()


if __name__ == "__main__":
    main()
