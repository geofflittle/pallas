"""Lists ledger accepted generated bodies holding a value whose last array element is a null.

usage: python3 -I null_candidates.py OUT_DIR NAME

Covers voting procedures under body key 19, gov actions and constitutions
under key 20, and certificates under key 4. Prints, per hit, the record
index, the value kind with its leading element, the element count, the size
of the value, and its path.
"""

import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402

KINDS = [
    ("vote", re.compile(r"\{19\} \{k@[0-9a-f]+\} \{k@[0-9a-f]+\}")),
    ("action", re.compile(r"\{20\}( tag258)? \[\d+\] \[2\]")),
    ("constitution", re.compile(r"\{20\}( tag258)? \[\d+\] \[2\] \[2\]")),
    ("cert", re.compile(r"\{4\}( tag258)? \[\d+\]")),
]
ELEM = re.compile(r"(.*) \[(\d+)\]")


def main():
    out, name = sys.argv[1], sys.argv[2]
    gen = [json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()]
    oracle = [json.loads(x) for x in open(os.path.join(out, name + ".oracle.jsonl")) if x.strip()]
    for g, o in zip(gen, oracle):
        if not o.get("ok"):
            continue
        b = bytes.fromhex(g["hex"])
        s = Spans(b)
        s.item([])
        spans = {p: (st, en) for p, st, en in s.items}
        children = {}
        for p in spans:
            m = ELEM.fullmatch(p)
            if m:
                children.setdefault(m.group(1), []).append(int(m.group(2)))
        for kind, pattern in KINDS:
            for p, (st, en) in spans.items():
                if not pattern.fullmatch(p) or p not in children:
                    continue
                n = max(children[p])
                last = spans[f"{p} [{n}]"]
                if b[last[0]:last[1]] != b"\xf6":
                    continue
                first = spans[f"{p} [0]"]
                lead = b[first[0]:first[1]].hex() if kind in ("action", "cert") else ""
                if kind == "constitution":
                    head = spans[p.rsplit(" ", 1)[0] + " [0]"]
                    if b[head[0]:head[1]] != b"\x05":
                        continue
                print(g["i"], kind + lead, n + 1, en - st, p)


if __name__ == "__main__":
    main()
