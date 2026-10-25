"""Compares the ledger oracle, the pallas decode and the pallas v1 mapper per generated value.

usage: python3 -I compare.py KIND GEN.jsonl ORACLE.jsonl PROBE.jsonl OUT.jsonl

Prints a summary to stdout and writes one JSON line per disagreement to OUT.
"""

import json
import re
import sys
from fractions import Fraction


class Reader:
    """A small CBOR reader that folds definite and indefinite lengths together."""

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
            return None
        v = int.from_bytes(self.b[self.p:self.p + n], "big")
        self.p += n
        return v

    def item(self):
        ib = self.byte()
        major, info = ib >> 5, ib & 31
        if major in (0, 1):
            v = self.arg(info)
            return v if major == 0 else -1 - v
        if major in (2, 3):
            if info == 31:
                parts = []
                while self.b[self.p] != 0xFF:
                    parts.append(self.item())
                self.p += 1
                return ("bytes", b"".join(x[1] for x in parts)) if major == 2 else "".join(parts)
            n = self.arg(info)
            raw = self.b[self.p:self.p + n]
            self.p += n
            return ("bytes", bytes(raw)) if major == 2 else raw.decode("utf8", "replace")
        if major == 4:
            out = []
            if info == 31:
                while self.b[self.p] != 0xFF:
                    out.append(self.item())
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    out.append(self.item())
            return ("list", tuple(out))
        if major == 5:
            out = []
            if info == 31:
                while self.b[self.p] != 0xFF:
                    out.append((self.item(), self.item()))
                self.p += 1
            else:
                for _ in range(self.arg(info)):
                    out.append((self.item(), self.item()))
            return ("map", tuple(sorted(out, key=repr)))
        if major == 6:
            tag = self.arg(info)
            v = self.item()
            if tag == 30 and v[0] == "list" and len(v[1]) == 2:
                n, d = v[1]
                if d:
                    return ("ratio", Fraction(n, d))
            if tag == 258:
                return v
            if tag == 2 and v[0] == "bytes":
                return int.from_bytes(v[1], "big")
            if tag == 3 and v[0] == "bytes":
                return -1 - int.from_bytes(v[1], "big")
            return ("tag", tag, v)
        if major == 7:
            if info == 20:
                return False
            if info == 21:
                return True
            if info == 22:
                return None
            if info == 23:
                return ("undefined",)
            if info in (25, 26, 27):
                n = {25: 2, 26: 4, 27: 8}[info]
                raw = self.b[self.p:self.p + n]
                self.p += n
                return ("float", bytes(raw))
            return ("simple", self.arg(info))
        raise ValueError("bad major")


def cbor(hexstr):
    return Reader(bytes.fromhex(hexstr)).item()


def num(v):
    if isinstance(v, bool) or v is None:
        return v
    if isinstance(v, (int,)):
        return v
    if isinstance(v, str):
        try:
            return int(v)
        except ValueError:
            return v
    return v


def load(path):
    out = {}
    with open(path) as fh:
        for n, line in enumerate(fh):
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            out[rec.get("i", n)] = rec
    return out


def load_lines(path):
    with open(path) as fh:
        return [json.loads(x) for x in fh if x.strip()]


# Coverage ---------------------------------------------------------------------

def keys_of(m):
    return {k for k, _ in m[1]} if isinstance(m, tuple) and m and m[0] == "map" else set()


def get(m, key):
    if isinstance(m, tuple) and m and m[0] == "map":
        for k, v in m[1]:
            if k == key:
                return v
    return None


def elems(v):
    return list(v[1]) if isinstance(v, tuple) and v and v[0] == "list" else []


def cover_body(cov, body, prefix="body"):
    for k in keys_of(body):
        cov.setdefault(f"{prefix}.key", set()).add(k)
    for c in elems(get(body, 4)):
        e = elems(c)
        if e:
            cov.setdefault("cert.tag", set()).add(e[0])
    for p in elems(get(body, 20)):
        e = elems(p)
        if len(e) >= 3:
            ga = elems(e[2])
            if ga:
                cov.setdefault("gov_action.tag", set()).add(ga[0])
    for o in elems(get(body, 1)):
        if isinstance(o, tuple) and o[0] == "list":
            cov.setdefault("output.form", set()).add("legacy")
        else:
            cov.setdefault("output.form", set()).add("map")
            for k in keys_of(o):
                cov.setdefault("output.key", set()).add(k)
    g = get(body, 14)
    if g is not None:
        e = elems(g)
        if e:
            cov.setdefault("guards.arm", set()).add("credential" if isinstance(e[0], tuple) and e[0][0] == "list" else "keyhash")
    subs = get(body, 23)
    for s in elems(subs):
        e = elems(s)
        if e:
            cover_body(cov, e[0], "sub_body")
            cover_wits(cov, e[1] if len(e) > 1 else None, "sub_wits")


def cover_wits(cov, w, prefix="wits"):
    for k in keys_of(w):
        cov.setdefault(f"{prefix}.key", set()).add(k)
    r = get(w, 5)
    if r is not None:
        cov.setdefault("redeemers.form", set()).add(r[0])
        if r[0] == "map":
            for k, _ in r[1]:
                e = elems(k)
                if e:
                    cov.setdefault("redeemer.tag", set()).add(e[0])
        else:
            for x in elems(r):
                e = elems(x)
                if e:
                    cov.setdefault("redeemer.tag", set()).add(e[0])


def coverage(kind, hexstr, cov):
    try:
        v = cbor(hexstr)
    except Exception as e:  # noqa: BLE001
        cov.setdefault("unreadable", set()).add(str(e)[:40])
        return
    if kind == "body":
        cover_body(cov, v)
    elif kind == "wits":
        cover_wits(cov, v)
    elif kind == "tx":
        e = elems(v)
        cov.setdefault("tx.len", set()).add(len(e))
        if e:
            cover_body(cov, e[0])
        if len(e) > 1:
            cover_wits(cov, e[1])
    elif kind == "ppu":
        for k in keys_of(v):
            cov.setdefault("ppu.key", set()).add(k)
    elif kind == "block":
        e = elems(v)
        if len(e) == 2:
            body = elems(e[1])
            if body:
                txs = elems(body[0])
                cov.setdefault("block.txs", set()).add(min(len(txs), 5))
                for t in txs:
                    te = elems(t)
                    if te:
                        cover_body(cov, te[0])
                    if len(te) > 1:
                        cover_wits(cov, te[1])
                if len(body) > 1:
                    cov.setdefault("block.leios_cert", set()).add(body[1] is not None)
                if len(body) > 2:
                    cov.setdefault("block.peras_cert", set()).add(body[2] is not None)


# Comparison -------------------------------------------------------------------

def ledger_value(hexstr):
    v = cbor(hexstr)
    return v


def unwrap(mapped):
    while isinstance(mapped, dict) and len(mapped) == 1:
        mapped = next(iter(mapped.values()))
    return mapped


def raw_ratio(ledger_hex):
    """The numerator and denominator as written, before any reduction."""
    b = bytes.fromhex(ledger_hex)
    if b[:2] != b"\xd8\x1e":
        return None
    r = Reader(b[2:])
    pair = r.item()
    if pair[0] != "list" or len(pair[1]) != 2:
        return None
    return pair[1]


def to_i32(x):
    x &= 0xFFFFFFFF
    return x - (1 << 32) if x >= (1 << 31) else x


def mapper_verdict(key, ledger_hex, mapped, pallas_hex=None):
    """None when the mapper agrees with the ledger, otherwise the class of the disagreement.

    A rational is checked for truncation against the numerator and denominator
    pallas decoded, since the ledger re-encoding writes the reduced fraction.
    """
    v = ledger_value(ledger_hex)
    m = unwrap(mapped)
    if isinstance(v, int) and not isinstance(v, bool):
        return None if num(m) == v else "mapper fact"
    if isinstance(v, tuple) and v[0] == "ratio" and isinstance(m, dict):
        n = num(m.get("numerator", 0))
        d = num(m.get("denominator", 1))
        try:
            if Fraction(int(n), int(d)) == v[1]:
                return None
        except (ValueError, ZeroDivisionError, TypeError):
            pass
        raw = raw_ratio(pallas_hex) if pallas_hex else raw_ratio(ledger_hex)
        if raw is not None:
            rn, rd = raw
            if int(n) == to_i32(rn) and int(d) == rd & 0xFFFFFFFF:
                return "mapper truncates a rational to 32 bits"
        return "mapper fact"
    return None if mapped is not None else "mapper fact"


def first_diff(a, b):
    for n, (x, y) in enumerate(zip(a, b)):
        if x != y:
            return n
    return min(len(a), len(b))


def compare(kind, gen, oracle, probe, reenc=None):
    out = []
    for i, g in gen.items():
        o = oracle.get(i)
        p = probe.get(i)
        base = {"i": i, "source": g.get("source"), "cddl_valid": g.get("cddl_valid"), "hex": g["hex"]}
        if o is None or p is None:
            out.append(dict(base, cls="missing", ledger=o, pallas=p))
            continue
        if o["ok"] != p["ok"]:
            cls = "pallas refuses, ledger accepts" if o["ok"] else "pallas accepts, ledger refuses"
            reason = p.get("err") if o["ok"] else o.get("err")
            reason = re.sub(r"\d+", "N", str(reason))[:90]
            out.append(dict(base, cls=cls, key=reason, ledger=o.get("err") or o.get("facts"), pallas=p.get("err") or p.get("facts")))
        if not o["ok"]:
            continue
        lf = o["facts"]
        # Keys whose decode already disagrees, so the mapper and the re-encoding are not counted again for them.
        bad = set()
        if p["ok"]:
            pf = p["facts"]
            if kind == "ppu":
                for k in sorted(set(lf) | set(pf)):
                    a, b = lf.get(k), pf.get(k)
                    same = a is not None and b is not None and ledger_value(a) == ledger_value(b)
                    if not same:
                        bad.add(k)
                        cls = "pallas drops a key the ledger reads" if b is None else "decode fact"
                        out.append(dict(base, cls=cls, key=k, ledger=a, pallas=b))
            else:
                for k, a in lf.items():
                    b = pf.get(k, "<absent>")
                    if num(a) != num(b):
                        out.append(dict(base, cls="decode fact", key=k, ledger=a, pallas=b))
            r = reenc.get(i) if reenc is not None else None
            if not p.get("reencode_equal") and r is not None:
                again = p.get("reencode_hex", "")
                at = first_diff(bytes.fromhex(g["hex"]), bytes.fromhex(again))
                where = dict(
                    offset=at,
                    input_at=g["hex"][max(0, 2 * at - 20):2 * at + 40],
                    pallas_at=again[max(0, 2 * at - 20):2 * at + 40],
                    reencode_hex=again,
                )
                if not r["ok"]:
                    out.append(
                        dict(
                            base,
                            cls="ledger refuses the pallas re-encoding",
                            key=re.sub(r"\d+", "N", r["err"])[:90],
                            ledger=r["err"],
                            **where,
                        )
                    )
                else:
                    rf = r["facts"]
                    if kind == "ppu":
                        changed = [k for k in sorted(set(lf) | set(rf)) if k not in bad and (lf.get(k) is None or rf.get(k) is None or ledger_value(lf[k]) != ledger_value(rf[k]))]
                    else:
                        changed = [k for k in sorted(set(lf) | set(rf)) if num(lf.get(k)) != num(rf.get(k))]
                    if changed:
                        out.append(
                            dict(
                                base,
                                cls="pallas re-encoding changes what the ledger reads",
                                key=",".join(changed),
                                ledger={k: lf.get(k) for k in changed},
                                pallas={k: rf.get(k) for k in changed},
                                **where,
                            )
                        )
        if p["ok"] and not p["mapper_ok"]:
            out.append(dict(base, cls="mapper fails on a value pallas decodes", pallas=p.get("mapper_err")))
        if p["mapper_ok"]:
            mf = p["mapper"]
            if kind == "ppu":
                for k in sorted(set(lf) | set(mf)):
                    if k in bad:
                        continue
                    if k not in lf:
                        out.append(dict(base, cls="mapper fact", key=k, ledger=None, mapper=mf[k]))
                    elif k not in mf:
                        out.append(dict(base, cls="mapper fact", key=k, ledger=lf[k], mapper=None))
                    else:
                        verdict = mapper_verdict(k, lf[k], mf[k], (p.get("facts") or {}).get(k))
                        if verdict:
                            out.append(dict(base, cls=verdict, key=k, ledger=lf[k], mapper=mf[k]))
            else:
                pf = p.get("facts") or {}
                for k, a in lf.items():
                    if k not in mf:
                        continue
                    b = mf[k]
                    if num(a) == num(b):
                        continue
                    raw = pf.get("raw." + k)
                    if raw is not None and num(raw) == num(b) and num(pf.get(k)) == num(a):
                        cls = "mapper keeps duplicates the ledger collapses"
                    else:
                        cls = "mapper fact"
                    out.append(dict(base, cls=cls, key=k, ledger=a, mapper=b))
    return out


def main():
    kind, gpath, opath, ppath, outpath = sys.argv[1:6]
    gen_lines = load_lines(gpath)
    gen = {}
    for n, g in enumerate(gen_lines):
        gen[n] = g
    oracle = {n: r for n, r in enumerate(load_lines(opath))}
    probe = {n: r for n, r in enumerate(load_lines(ppath))}
    reenc = {n: r for n, r in enumerate(load_lines(sys.argv[6]))} if len(sys.argv) > 6 else None
    dis = compare(kind, gen, oracle, probe, reenc)
    cov = {}
    for g in gen.values():
        coverage(kind, g["hex"], cov)
    with open(outpath, "w") as fh:
        for d in dis:
            fh.write(json.dumps(d, default=str) + "\n")
    n = len(gen)
    lok = sum(1 for r in oracle.values() if r["ok"])
    pok = sum(1 for r in probe.values() if r["ok"])
    mok = sum(1 for r in probe.values() if r["mapper_ok"])
    reenc = sum(1 for r in probe.values() if r.get("reencode_equal"))
    valid = sum(1 for g in gen.values() if g.get("cddl_valid") is True)
    print(f"{kind}: values {n}, cddl_valid {valid}, ledger_ok {lok}, pallas_ok {pok}, mapper_ok {mok}, reencode_equal {reenc}")
    by = {}
    for d in dis:
        key = (d["cls"], d.get("key"))
        by[key] = by.get(key, 0) + 1
    print(f"{kind}: disagreements {len(dis)} on {len({d['i'] for d in dis})} values")
    for (cls, key), c in sorted(by.items(), key=lambda x: (-x[1], str(x[0]))):
        print(f"  {c:4d}  {cls}  {key or ''}")
    for k in sorted(cov):
        print(f"  cover {k}: {sorted(cov[k], key=str)}")


if __name__ == "__main__":
    main()
