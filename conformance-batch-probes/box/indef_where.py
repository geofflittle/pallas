"""Prints the path of every indefinite length item in a hex file.

usage: python3 -I indef_where.py FILE [FILE ...]
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extract import Spans  # noqa: E402

for f in sys.argv[1:]:
    b = bytes.fromhex(open(f).read().strip())
    s = Spans(b)
    s.item([])
    kinds = {0x5F: "bytes", 0x7F: "text", 0x9F: "array", 0xBF: "map"}
    print(f, len(b), [(kinds[b[st]], p) for p, st, _ in sorted(s.items, key=lambda x: x[1]) if b[st] in kinds],
          "keys", [kinds[b[st]] for st in s.keys if b[st] in kinds])
