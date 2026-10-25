"""Compares the value pallas decodes with the value the ledger decodes, field by field.

usage: python3 -I valuecmp.py KIND GEN ORACLE PROBE REORACLE OUT_PREFIX

GEN holds the inputs, ORACLE the ledger result for each, PROBE the pallas
result, REORACLE the ledger result for the pallas re-encoding (a placeholder
line where pallas gave the input back). Prints one summary line, the agree and
differ counts per field path, the listed classes and the disagreements by
class, writes the disagreements to OUT_PREFIX.dis.jsonl and the counts to
OUT_PREFIX.counts.json, and exits 1 when any disagreement is found.
"""

import json
import re
import sys
from collections import Counter

import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from value import Notes, diff, generic, normalize  # noqa: E402


def load_lines(path):
    with open(path) as fh:
        return [json.loads(x) for x in fh if x.strip()]


def short(msg):
    return re.sub(r"\d+", "N", str(msg))[:120]


TAG24_INDEF = re.compile(r"unexpected type indefinite bytes at position (\d+): expected bytes \(definite length\)")


def tag24_indefinite(err, hexstr):
    """Whether pallas refused an indefinite byte string directly under tag 24, the one open refusal class."""
    m = TAG24_INDEF.fullmatch(str(err))
    return bool(m) and hexstr[2 * int(m.group(1)) - 4 : 2 * int(m.group(1))] == "d818"


def compare(kind, gen, oracle, probe, reoracle):
    totals = Counter()
    agree, differ = Counter(), Counter()
    listed, accepted, dis = Counter(), Counter(), []
    for n, g in enumerate(gen):
        o, p, r = oracle[n], probe[n], reoracle[n]
        totals["values"] += 1
        if not o["ok"]:
            totals["ledger refuses"] += 1
            if p["ok"]:
                accepted[f"{g.get('mutation') or g.get('source') or 'input'}: {short(o.get('err'))}"] += 1
            continue
        totals["ledger accepts"] += 1
        if not p["ok"] and tag24_indefinite(p.get("err"), g["hex"]):
            listed[("pallas refuses tag 24 over indefinite bytes, open decision on a stable type", "")] += 1
            continue
        if not p["ok"]:
            dis.append(dict(i=n, cls="pallas refuses ledger-valid input", key=short(p.get("err")), hex=g["hex"]))
            continue
        totals["both accept"] += 1
        notes = Notes()
        ln = normalize(o["dump"])
        pn = normalize(p["dump"], notes=notes)
        for (what, path), c in notes.c.items():
            listed[(what, path)] += c
        good, bad = diff(ln, pn)
        for path in good:
            agree[generic(path)] += 1
        for path, lv, pv in bad:
            gp = generic(path)
            differ[gp] += 1
            cls = "pallas lacks" if pv == "<absent>" else "pallas extra" if lv == "<absent>" else "value differs"
            dis.append(dict(i=n, cls=cls, key=gp, path=path, ledger=lv, pallas=pv, hex=g["hex"]))
        # The re-encodings.
        again = p.get("reencode_hex") or g["hex"]
        if again == o["reencode"]:
            totals["re-encodings identical"] += 1
            continue
        if again == g["hex"]:
            listed[("ledger re-encodes, pallas keeps the input bytes", "")] += 1
            continue
        if not r["ok"]:
            dis.append(dict(i=n, cls="ledger refuses the pallas re-encoding", key=short(r.get("err")), hex=g["hex"], again=again))
            continue
        if normalize(r["dump"]) != ln:
            dis.append(dict(i=n, cls="pallas re-encoding holds a different ledger value", key="", hex=g["hex"], again=again))
            continue
        listed[("pallas re-encodes to other bytes of the same ledger value", "")] += 1
    return totals, agree, differ, listed, accepted, dis


def main():
    kind, gpath, opath, ppath, rpath, prefix = sys.argv[1:7]
    gen, oracle, probe, reoracle = (load_lines(x) for x in (gpath, opath, ppath, rpath))
    if not (len(gen) == len(oracle) == len(probe) == len(reoracle)):
        print(f"line counts differ: gen {len(gen)} oracle {len(oracle)} probe {len(probe)} reoracle {len(reoracle)}")
        sys.exit(2)
    totals, agree, differ, listed, accepted, dis = compare(kind, gen, oracle, probe, reoracle)
    with open(f"{prefix}.dis.jsonl", "w") as fh:
        for d in dis:
            fh.write(json.dumps(d, default=str) + "\n")
    with open(f"{prefix}.counts.json", "w") as fh:
        json.dump(
            dict(
                totals=totals,
                agree=agree,
                differ=differ,
                listed={f"{w} {p}": c for (w, p), c in listed.items()},
                accepted_invalid=accepted,
            ),
            fh,
            indent=1,
            sort_keys=True,
        )
    print(
        f"value {kind}: values {totals['values']}, ledger accepts {totals['ledger accepts']}, "
        f"both accept {totals['both accept']}, field paths {len(set(agree) | set(differ))}, "
        f"leaves agree {sum(agree.values())}, leaves differ {sum(differ.values())}, "
        f"accepted ledger-invalid {sum(accepted.values())}, "
        f"disagreements {len(dis)} on {len({d['i'] for d in dis})} values"
    )
    for (what, path), c in sorted(listed.items(), key=lambda x: (-x[1], str(x[0]))):
        print(f"  {c:5d}  listed  {what} {path}")
    for m, c in sorted(accepted.items(), key=lambda x: (-x[1], x[0])):
        print(f"  {c:5d}  accepted ledger-invalid  {m}")
    by = Counter((d["cls"], d.get("key")) for d in dis)
    for (cls, key), c in sorted(by.items(), key=lambda x: (-x[1], str(x[0]))):
        print(f"  {c:5d}  DISAGREE  {cls}  {key or ''}")
    sys.exit(1 if dis else 0)


if __name__ == "__main__":
    main()
