"""Compares each utxorpc schema's mapping of the values pallas decodes against the ledger oracle.

usage: schemas.py KIND GEN ORACLE PROBE OUT_PREFIX

The probe line carries `schemas.{v1alpha,v1beta,v1}` as `{ok, facts}` or `{ok, err}`.
For each schema the script prints one summary line, the panics and errors by
message, and the fact disagreements by class and key, and writes them to
OUT_PREFIX.SCHEMA.dis.jsonl. Facts are compared with the comparison compare.py
applies to the v1 mapper.
"""

import json
import os
import re
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from compare import compare, ledger_value, load_lines  # noqa: E402

SCHEMAS = ["v1alpha", "v1beta", "v1"]
BEFORE_V1 = {"v1alpha", "v1beta"}

# Proto3 scalars of the before v1 schemas, where an unset field reads as zero.
ZERO_KEYS = {"ttl", "validity_start", "fee", "total_collateral"}
ZERO_PPU_KEYS = {f"k{k}" for k in (2, 3, 4, 7, 8, 22, 23, 24, 27, 28, 29, 32)}
# The last protocol parameter key the before v1 `PParams` message has a field for.
LAST_BEFORE_V1_PPU_KEY = 33

LEDGER_PURPOSE = {
    "DijkstraSpending": "SPEND",
    "DijkstraMinting": "MINT",
    "DijkstraCertifying": "CERT",
    "DijkstraWithdrawing": "REWARD",
    "DijkstraVoting": "VOTE",
    "DijkstraProposing": "PROPOSE",
    "DijkstraGuarding": "GUARDING",
}
SHOW_LIMIT = 4000
REDEEMER_RE = re.compile(r"\((Dijkstra\w+) \(AsIx \{unAsIx = (\d+)\}\)")


def short(msg):
    return re.sub(r"\d+", "N", str(msg))[:100]


def bare(key):
    return key.split(".", 1)[1] if "." in key else key


def ledger_redeemer_keys(o, count_key):
    """The redeemer purposes and indexes the ledger show prints, or None when the show cannot settle them."""
    show = o.get("show", "")
    if len(show) >= SHOW_LIMIT:
        return None
    found = REDEEMER_RE.findall(show)
    if len(found) != o["facts"].get(count_key):
        return None
    return sorted(f"{LEDGER_PURPOSE.get(p, p)}:{i}" for p, i in found)


def judge_redeemers(schema, ledger, mapped):
    """None when they agree, otherwise the class of the disagreement."""
    if ledger == mapped:
        return None
    if schema in BEFORE_V1:
        held = sorted(k.replace("GUARDING:", "UNSPECIFIED:") for k in ledger)
        if held == mapped:
            return "guarding held as unspecified"
    return "mapper redeemer purpose"


def run(schema, kind, gen, oracle, probe):
    view = {}
    for n, p in probe.items():
        s = p["schemas"][schema]
        view[n] = dict(p, mapper_ok=s["ok"], mapper=s.get("facts"), mapper_err=s.get("err"))
    dis = []
    no_field = Counter()
    zero = 0
    for d in compare(kind, gen, oracle, view):
        # A failure to map is counted by its message in the summary line.
        if not d["cls"].startswith("mapper") or d["cls"] == "mapper fails on a value pallas decodes":
            continue
        key = d.get("key")
        if schema in BEFORE_V1 and d["cls"] == "mapper fact":
            if kind == "ppu" and d.get("mapper") is None and int(key[1:]) > LAST_BEFORE_V1_PPU_KEY:
                no_field[key] += 1
                continue
            if kind == "ppu":
                ledger = ledger_value(d["ledger"]) if d.get("ledger") is not None else None
                zero_key = key in ZERO_PPU_KEYS
            else:
                ledger = d.get("ledger")
                zero_key = bare(key) in ZERO_KEYS
            # Main writes an absent total collateral as zero in these schemas, for every era.
            if zero_key and {ledger, d.get("mapper")} <= {0, None}:
                zero += 1
                continue
        d["schema"] = schema
        dis.append(d)
    checked = 0
    count_key = {"tx": "wits.redeemers", "wits": "redeemers"}.get(kind)
    keys_key = {"tx": "wits.redeemer_keys", "wits": "redeemer_keys"}.get(kind)
    for n, g in gen.items():
        o, p = oracle.get(n), view.get(n)
        if o is None or p is None or not o["ok"]:
            continue
        mf = p["mapper"] or {}
        lf = o["facts"]
        if kind != "ppu":
            for k in lf:
                if p["mapper_ok"] and k not in mf and not k.startswith("raw."):
                    no_field[k] += 1
        if count_key is None or not p["mapper_ok"] or keys_key not in mf:
            continue
        ledger = ledger_redeemer_keys(o, count_key)
        if ledger is None:
            continue
        checked += 1
        cls = judge_redeemers(schema, ledger, mf[keys_key])
        if cls:
            dis.append(dict(i=n, schema=schema, cls=cls, key=keys_key, ledger=ledger, mapper=mf[keys_key], hex=g["hex"]))
    return dis, no_field, zero, checked


def main():
    kind, gpath, opath, ppath, prefix = sys.argv[1:6]
    gen = dict(enumerate(load_lines(gpath)))
    oracle = dict(enumerate(load_lines(opath)))
    probe = dict(enumerate(load_lines(ppath)))
    for schema in SCHEMAS:
        pok = [n for n, p in probe.items() if p["ok"]]
        errs = Counter()
        for n in pok:
            s = probe[n]["schemas"][schema]
            if not s["ok"]:
                errs[short(s["err"])] += 1
        panics = sum(c for m, c in errs.items() if m.startswith("panic: "))
        dis, no_field, zero, checked = run(schema, kind, gen, oracle, probe)
        with open(f"{prefix}.{schema}.dis.jsonl", "w") as fh:
            for d in dis:
                fh.write(json.dumps(d, default=str) + "\n")
        mapped = len(pok) - sum(errs.values())
        print(
            f"schema {schema} {kind}: values {len(gen)}, pallas_ok {len(pok)}, mapped {mapped}, "
            f"panics {panics}, errors {sum(errs.values()) - panics}, "
            f"disagreements {len(dis)} on {len({d['i'] for d in dis})} values, "
            f"redeemer purposes checked {checked}, zero and absent read alike {zero}"
        )
        for m, c in sorted(errs.items(), key=lambda x: (-x[1], x[0])):
            print(f"  {c:4d}  fails  {m}")
        by = Counter((d["cls"], d.get("key")) for d in dis)
        for (cls, key), c in sorted(by.items(), key=lambda x: (-x[1], str(x[0]))):
            print(f"  {c:4d}  {cls}  {key or ''}")
        if no_field:
            print(f"  no field in {schema}: {sorted(no_field)}")


if __name__ == "__main__":
    main()
