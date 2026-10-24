"""Prints, for each re-encoding the ledger refuses, the first path where the decoded values differ.

Length forms and map key order are ignored, so what remains is a change of content or shape.

usage: python3 -I semdiff.py DIS.jsonl [LIMIT]
"""

import collections
import json
import re
import sys


class Reader:
    def __init__(self, b):
        self.b = b
        self.p = 0

    def arg(self, info):
        if info < 24:
            return info
        n = {24: 1, 25: 2, 26: 4, 27: 8}[info]
        v = int.from_bytes(self.b[self.p:self.p + n], "big")
        self.p += n
        return v

    def item(self):
        ib = self.b[self.p]
        self.p += 1
        major, info = ib >> 5, ib & 31
        if major == 0:
            return ("int", self.arg(info))
        if major == 1:
            return ("int", -1 - self.arg(info))
        if major in (2, 3):
            kind = "bytes" if major == 2 else "text"
            if info == 31:
                out = b""
                while self.b[self.p] != 0xFF:
                    out += self.item()[1]
                self.p += 1
                return (kind, out)
            n = self.arg(info)
            v = self.b[self.p:self.p + n]
            self.p += n
            return (kind, v)
        if major == 4:
            xs = []
            if info == 31:
                while self.b[self.p] != 0xFF:
                    xs.append(self.item())
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    xs.append(self.item())
            return ("arr", xs)
        if major == 5:
            kv = []
            if info == 31:
                while self.b[self.p] != 0xFF:
                    kv.append((self.item(), self.item()))
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    kv.append((self.item(), self.item()))
            return ("map", kv)
        if major == 6:
            t = self.arg(info)
            return ("tag", t, self.item())
        if info in (25, 26, 27):
            n = {25: 2, 26: 4, 27: 8}[info]
            v = self.b[self.p:self.p + n]
            self.p += n
            return ("float", v)
        if info == 24:
            v = self.b[self.p]
            self.p += 1
            return ("simple", v)
        return ("simple", info)


def key(v):
    return repr(v)[:24]


def diff(a, b, path):
    if a[0] != b[0]:
        return path, f"{a[0]} vs {b[0]}", a, b
    if a[0] == "arr":
        if len(a[1]) != len(b[1]):
            return path, f"array len {len(a[1])} vs {len(b[1])}", a, b
        for i, (x, y) in enumerate(zip(a[1], b[1])):
            r = diff(x, y, path + [f"[{i}]"])
            if r:
                return r
        return None
    if a[0] == "map":
        ma = {repr(k): (k, v) for k, v in a[1]}
        mb = {repr(k): (k, v) for k, v in b[1]}
        if set(ma) != set(mb):
            only_a = [key(ma[k][0]) for k in ma if k not in mb]
            only_b = [key(mb[k][0]) for k in mb if k not in ma]
            return path, f"map keys only in input {only_a[:3]} only in output {only_b[:3]}", a, b
        for k in ma:
            r = diff(ma[k][1], mb[k][1], path + ["{" + key(ma[k][0]) + "}"])
            if r:
                return r
        return None
    if a[0] == "tag":
        if a[1] != b[1]:
            return path, f"tag {a[1]} vs {b[1]}", a, b
        return diff(a[2], b[2], path + [f"tag{a[1]}"])
    if a != b:
        return path, "value", a, b
    return None


def main():
    path = sys.argv[1]
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    shown = collections.defaultdict(list)
    for line in open(path):
        d = json.loads(line)
        if d.get("cls") != "ledger refuses the pallas re-encoding":
            continue
        a = Reader(bytes.fromhex(d["hex"])).item()
        b = Reader(bytes.fromhex(d.get("reencode_hex", ""))).item()
        r = diff(a, b, [])
        if r is None:
            sig, what, x, y = "<same value>", "", None, None
        else:
            sig, what, x, y = " ".join(r[0]), r[1], r[2], r[3]
        shape = re.sub(r"\[\d+\]", "[i]", re.sub(r"\{[^}]*\}", "{k}", sig) if "{(" in sig else sig) + " | " + what.split(" only")[0]
        shown[shape].append(d["i"])
        if len(shown[shape]) <= limit:
            print(d["i"], sig, "|", what, "|", repr(x)[:120], "|", repr(y)[:120])
    print("== shapes")
    for k, v in sorted(shown.items(), key=lambda x: -len(x[1])):
        print(len(v), k, v[:8])


if __name__ == "__main__":
    main()
