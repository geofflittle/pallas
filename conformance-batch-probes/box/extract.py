"""Prints the bytes of every item in a generated record whose path matches a pattern or whose bytes start with a prefix.

usage: python3 -I extract.py OUT_DIR NAME I (path PATTERN | prefix HEX) [LIMIT]

A path is written as where.py writes it, for example "[0] {4} [0]".
"""

import json
import os
import re
import sys


class Spans:
    def __init__(self, b):
        self.b = b
        self.p = 0
        self.items = []
        self.keys = []

    def arg(self, info):
        if info < 24:
            return info
        n = {24: 1, 25: 2, 26: 4, 27: 8}[info]
        v = int.from_bytes(self.b[self.p:self.p + n], "big")
        self.p += n
        return v

    def item(self, path):
        start = self.p
        ib = self.b[self.p]
        self.p += 1
        major, info = ib >> 5, ib & 31
        if major in (0, 1):
            self.arg(info)
        elif major in (2, 3):
            if info == 31:
                while self.b[self.p] != 0xFF:
                    self.item(path + ["chunk"])
                self.p += 1
            else:
                n = self.arg(info)
                self.p += n
        elif major == 4:
            if info == 31:
                i = 0
                while self.b[self.p] != 0xFF:
                    self.item(path + [f"[{i}]"])
                    i += 1
                self.p += 1
            else:
                for i in range(self.arg(info)):
                    self.item(path + [f"[{i}]"])
        elif major == 5:
            if info == 31:
                while self.b[self.p] != 0xFF:
                    k = self.key()
                    self.item(path + [f"{{{k}}}"])
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    k = self.key()
                    self.item(path + [f"{{{k}}}"])
        elif major == 6:
            t = self.arg(info)
            self.item(path + [f"tag{t}"])
        elif info in (25, 26, 27):
            self.p += {25: 2, 26: 4, 27: 8}[info]
        elif info == 24:
            self.p += 1
        self.items.append((" ".join(path), start, self.p))

    def key(self):
        ib = self.b[self.p]
        if ib >> 5 == 0 and (ib & 31) < 28:
            self.p += 1
            return self.arg(ib & 31)
        s = self.p
        saved = self.items
        self.items = []
        self.item([])
        self.keys.extend(st for _, st, _ in self.items)
        self.items = saved
        return "k@" + self.b[s:self.p].hex()[:16]


def main():
    out, name, i, mode, pattern = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], sys.argv[5]
    limit = int(sys.argv[6]) if len(sys.argv) > 6 else 3
    for line in open(os.path.join(out, name + ".gen.jsonl")):
        g = json.loads(line)
        if g["i"] != i:
            continue
        b = bytes.fromhex(g["hex"])
        s = Spans(b)
        s.item([])
        shown = 0
        for path, start, end in sorted(s.items, key=lambda x: x[1]):
            sub = b[start:end].hex()
            hit = re.fullmatch(pattern, path) if mode == "path" else sub.startswith(pattern)
            if hit and shown < limit:
                print(path, start, end - start, sub)
                shown += 1


if __name__ == "__main__":
    main()
