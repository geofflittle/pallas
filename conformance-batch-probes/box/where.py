"""Prints, for each disagreement record of one class, the CBOR path at the pallas error position.

usage: python3 -I where.py DIS.jsonl CLASS_SUBSTRING [LIMIT]

The path lists, from the root, each container as array index or map key, so
the decoder that refused can be named from the CDDL.
"""

import json
import re
import sys


class Walk:
    def __init__(self, b):
        self.b = b
        self.p = 0
        self.hits = []

    def arg(self, info):
        if info < 24:
            return info
        n = {24: 1, 25: 2, 26: 4, 27: 8}.get(info)
        if n is None:
            return None
        v = int.from_bytes(self.b[self.p:self.p + n], "big")
        self.p += n
        return v

    def item(self, path, target):
        start = self.p
        if start == target:
            self.hits.append((list(path), self.b[start:start + 12].hex()))
        ib = self.b[self.p]
        self.p += 1
        major, info = ib >> 5, ib & 31
        if major in (0, 1):
            self.arg(info)
            return ("int",)
        if major in (2, 3):
            if info == 31:
                while self.b[self.p] != 0xFF:
                    self.item(path + ["chunk"], target)
                if self.p == target:
                    self.hits.append((path + ["<break>"], self.b[self.p:self.p + 12].hex()))
                self.p += 1
                return ("str",)
            n = self.arg(info)
            self.p += n
            return ("str",)
        if major == 4:
            i = 0
            if info == 31:
                while self.b[self.p] != 0xFF:
                    self.item(path + [f"[{i}]"], target)
                    i += 1
                if self.p == target:
                    self.hits.append((path + [f"<break after {i}>"], self.b[self.p:self.p + 12].hex()))
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    self.item(path + [f"[{i}]"], target)
                    i += 1
            return ("arr",)
        if major == 5:
            if info == 31:
                while self.b[self.p] != 0xFF:
                    k = self.key()
                    self.item(path + [f"{{{k}}}"], target)
                if self.p == target:
                    self.hits.append((path + ["<map break>"], self.b[self.p:self.p + 12].hex()))
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    k = self.key()
                    self.item(path + [f"{{{k}}}"], target)
            return ("map",)
        if major == 6:
            t = self.arg(info)
            self.item(path + [f"tag{t}"], target)
            return ("tag",)
        if major == 7:
            if info in (25, 26, 27):
                self.p += {25: 2, 26: 4, 27: 8}[info]
            elif info == 24:
                self.p += 1
            return ("simple",)
        raise ValueError("bad major")

    def key(self):
        ib = self.b[self.p]
        if ib >> 5 == 0 and (ib & 31) < 28:
            self.p += 1
            return self.arg(ib & 31)
        s = self.p
        self.item([], -1)
        return "k@" + self.b[s:self.p].hex()[:16]


def main():
    path, needle = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 5
    shown = {}
    for line in open(path):
        d = json.loads(line)
        err = str(d.get("pallas") if isinstance(d.get("pallas"), str) else d.get("key"))
        if needle not in json.dumps(d.get("key")) and needle not in err and needle != d.get("cls"):
            continue
        m = re.search(r"position (\d+)", err)
        if not m:
            print(d["i"], "no position:", err[:100])
            continue
        pos = int(m.group(1))
        w = Walk(bytes.fromhex(d["hex"]))
        try:
            w.item([], pos)
        except Exception as e:  # noqa: BLE001
            print(d["i"], "walk failed", e)
        sig = " ".join(w.hits[0][0]) if w.hits else "<no item starts there>"
        shape = re.sub(r"\[\d+\]", "[i]", sig)
        shown.setdefault(shape, []).append(d["i"])
        if len(shown[shape]) <= limit:
            print(d["i"], pos, sig, w.hits[0][1] if w.hits else "", "|", err[:90])
    print("== shapes")
    for k, v in sorted(shown.items(), key=lambda x: -len(x[1])):
        print(len(v), k, v[:8])


if __name__ == "__main__":
    main()
