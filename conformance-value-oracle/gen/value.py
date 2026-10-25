"""Normalizes value dumps and compares them path by path.

A dump is the JSON form the ledger oracle and the pallas probe both print:
`{"cbor": hex}` leaves in each side's own encoding, `set`, `oset`, `map` and
`omap` wrappers where the ledger value is a collection, objects for records
and variants, and strings, booleans and null. `normalize` turns a dump into
plain Python values that compare equal when the values are equal, and
`flatten` turns that into one leaf per path.
"""

import hashlib
import json
import re
import sys
from collections import Counter
from fractions import Fraction
from itertools import zip_longest

END = object()


def rows(*paths):
    """Yields the parsed lines of the files one aligned row at a time, and exits 2 at the first row a file lacks or when there is no row."""
    files = [open(p) for p in paths]
    parsed = [(json.loads(x) for x in fh if x.strip()) for fh in files]
    n = 0
    for n, row in enumerate(zip_longest(*parsed, fillvalue=END), 1):
        if any(x is END for x in row):
            ended = " ".join(p for p, x in zip(paths, row) if x is END)
            print(f"line counts differ: row {n - 1} is missing from {ended}")
            sys.exit(2)
        yield row
    if n == 0:
        print(f"no rows in {' '.join(paths)}")
        sys.exit(2)


class Reader:
    """A CBOR reader that keeps map keys in order and folds encoding choices that carry no value."""

    def __init__(self, b):
        self.b = b
        self.p = 0

    def byte(self):
        x = self.b[self.p]
        self.p += 1
        return x

    def arg(self, info):
        if info < 24:
            return info
        n = {24: 1, 25: 2, 26: 4, 27: 8}.get(info)
        if n is None:
            raise ValueError(f"reserved additional info {info}")
        v = int.from_bytes(self.b[self.p:self.p + n], "big")
        self.p += n
        return v

    def items_until_break(self, one):
        out = []
        while self.b[self.p] != 0xFF:
            out.append(one())
        self.p += 1
        return out

    def item(self):
        ib = self.byte()
        major, info = ib >> 5, ib & 31
        if major in (0, 1):
            v = self.arg(info)
            return ("int", v if major == 0 else -1 - v)
        if major in (2, 3):
            if info == 31:
                parts = self.items_until_break(self.item)
                if major == 2:
                    return ("bytes", b"".join(x[1] for x in parts))
                return ("text", "".join(x[1] for x in parts))
            n = self.arg(info)
            raw = bytes(self.b[self.p:self.p + n])
            self.p += n
            return ("bytes", raw) if major == 2 else ("text", raw.decode("utf8", "replace"))
        if major == 4:
            if info == 31:
                return ("list", self.items_until_break(self.item))
            return ("list", [self.item() for _ in range(self.arg(info))])
        if major == 5:
            pair = lambda: (self.item(), self.item())  # noqa: E731
            if info == 31:
                return ("map", self.items_until_break(pair))
            return ("map", [pair() for _ in range(self.arg(info))])
        if major == 6:
            return ("tag", self.arg(info), self.item())
        if major == 7:
            if info == 20:
                return ("bool", False)
            if info == 21:
                return ("bool", True)
            if info == 22:
                return ("null",)
            if info == 23:
                return ("undefined",)
            if info in (25, 26, 27):
                n = {25: 2, 26: 4, 27: 8}[info]
                raw = bytes(self.b[self.p:self.p + n])
                self.p += n
                return ("float", raw.hex())
            return ("simple", self.arg(info))
        raise ValueError("bad major type")


def canon(v):
    """A string that two normalized values share exactly when they are equal."""
    return json.dumps(v, sort_keys=True, separators=(",", ":"))


def key_of(v):
    """A short path segment naming a collection element or map key by its value."""
    return hashlib.sha1(canon(v).encode()).hexdigest()[:12]


def term(t):
    """The normalized value of one CBOR term."""
    k = t[0]
    if k == "int":
        return str(t[1])
    if k == "bytes":
        return "h:" + t[1].hex()
    if k == "text":
        return "t:" + t[1]
    if k == "bool":
        return t[1]
    if k == "null":
        return None
    if k == "undefined":
        return "undefined"
    if k == "float":
        return "f:" + t[1]
    if k == "simple":
        return f"simple {t[1]}"
    if k == "list":
        return [term(x) for x in t[1]]
    if k == "map":
        out = {}
        for a, b in t[1]:
            out[canon(term(a))] = term(b)
        return {"m": out}
    if k == "tag":
        tag, inner = t[1], t[2]
        if tag == 258:
            return term(inner)
        if tag == 2 and inner[0] == "bytes":
            return str(int.from_bytes(inner[1], "big"))
        if tag == 3 and inner[0] == "bytes":
            return str(-1 - int.from_bytes(inner[1], "big"))
        if tag == 30 and inner[0] == "list" and len(inner[1]) == 2 and all(x[0] == "int" for x in inner[1]):
            n, d = inner[1][0][1], inner[1][1][1]
            if d != 0:
                r = Fraction(n, d)
                return f"r:{r.numerator}/{r.denominator}"
        return {"tag": str(tag), "v": term(inner)}
    raise ValueError(f"unknown term {k}")


def cbor(hexstr):
    r = Reader(bytes.fromhex(hexstr))
    t = r.item()
    if r.p != len(r.b):
        return {"trailing": hexstr}
    return term(t)


class Notes:
    """Counts of value differences a normalization folds, by generic path."""

    def __init__(self):
        self.c = Counter()

    def add(self, what, path):
        self.c[(what, generic(path))] += 1


def normalize(d, path="", notes=None):
    """The normalized value of a dump, recording folded duplicates in `notes`."""
    if isinstance(d, dict):
        if set(d) == {"cbor"}:
            return cbor(d["cbor"])
        if set(d) == {"set"}:
            items = [normalize(x, path + "[]", notes) for x in d["set"]]
            out = {canon(x): x for x in items}
            if notes is not None and len(out) != len(items):
                notes.add("duplicate set elements folded", path)
            return {"set": {key_of(v): v for _, v in sorted(out.items())}}
        if set(d) == {"oset"}:
            seen, out = set(), []
            for x in d["oset"]:
                v = normalize(x, path + "[]", notes)
                c = canon(v)
                if c in seen:
                    if notes is not None:
                        notes.add("duplicate ordered set elements folded", path)
                    continue
                seen.add(c)
                out.append(v)
            return {"oset": out}
        if set(d) in ({"map"}, {"omap"}):
            ordered = "omap" in d
            out = {}
            for k, v in d["map" if not ordered else "omap"]:
                nk = normalize(k, path + "{}", notes)
                c = canon(nk)
                if c in out and notes is not None:
                    notes.add("duplicate map keys folded", path)
                if ordered and c in out:
                    continue
                out[c] = (nk, normalize(v, path + "{}", notes))
            entries = {key_of(nk): {"k": nk, "v": nv} for nk, nv in out.values()}
            if ordered:
                return {"omap": entries, "order": [key_of(nk) for nk, _ in out.values()]}
            return {"map": entries}
        return {k: normalize(v, path + "." + k, notes) for k, v in d.items()}
    if isinstance(d, list):
        return [normalize(x, path + "[]", notes) for x in d]
    return d


def generic(path):
    """The path with collection positions and keys replaced, so it names a field rather than one value."""
    out, i = [], 0
    while i < len(path):
        ch = path[i]
        if ch == "[":
            j = path.index("]", i)
            out.append("[]")
            i = j + 1
        elif ch == "{":
            j = path.index("}", i)
            out.append("{}")
            i = j + 1
        else:
            out.append(ch)
            i += 1
    return "".join(out)


INT_KEY = re.compile(r'^"-?\d+"$')


def segment(kind, key):
    """The path segment of one map entry: a small integer key by its value, any other key by its hash."""
    if kind == "m":
        if INT_KEY.match(key):
            return f"<{key.strip(chr(34))}>"
        return f"{{{key_of(key)}}}"
    return f"{{{key}}}"


def flatten(v, path="", out=None):
    """One leaf per path of a normalized value."""
    if out is None:
        out = {}
    if isinstance(v, dict):
        if not v:
            out[path + "{empty}"] = True
        for k, x in v.items():
            if k in ("set", "map", "omap", "m") and isinstance(x, dict):
                if not x:
                    out[path + "{empty}"] = True
                for kk, xx in x.items():
                    flatten(xx, f"{path}{segment(k, kk)}", out)
            elif k == "oset" and isinstance(x, list):
                if not x:
                    out[path + "[empty]"] = True
                for n, xx in enumerate(x):
                    flatten(xx, f"{path}[{n}]", out)
            else:
                flatten(x, f"{path}.{k}" if path else k, out)
    elif isinstance(v, list):
        if not v:
            out[path + "[empty]"] = True
        for n, x in enumerate(v):
            flatten(x, f"{path}[{n}]", out)
    else:
        out[path] = v
    return out


def diff(ledger, pallas):
    """The leaves that agree and the leaves that differ, as (path, ledger leaf, pallas leaf)."""
    a, b = flatten(ledger), flatten(pallas)
    agree, differ = [], []
    for p in sorted(set(a) | set(b)):
        if p in a and p in b and a[p] == b[p]:
            agree.append(p)
        else:
            differ.append((p, a.get(p, "<absent>"), b.get(p, "<absent>")))
    return agree, differ
