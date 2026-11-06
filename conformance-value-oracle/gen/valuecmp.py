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
import expected  # noqa: E402
from mutate import GUARDS, bodies, copy  # noqa: E402
from value import Notes, Reader, diff, generic, normalize, rows  # noqa: E402


def short(msg):
    return re.sub(r"\d+", "N", str(msg))[:120]


def without_ids(v):
    """The value without its transaction ids, which name the input bytes rather than the value."""
    if isinstance(v, dict):
        return {k: without_ids(x) for k, x in v.items() if k != "id"}
    if isinstance(v, list):
        return [without_ids(x) for x in v]
    return v


def empty_guards(kind, hexstr):
    """Whether a transaction or body input writes key 14 of its body or of a sub transaction body as an empty set."""
    if kind not in ("tx", "body"):
        return False
    top, subs = bodies(copy(Reader(bytes.fromhex(hexstr)).item()))
    for b in ([top] if top else []) + subs:
        for k, v in b[1]:
            inner = v[2] if v[0] == "tag" else v
            if k == GUARDS and inner[0] == "list" and not inner[1]:
                return True
    return False


def eb_list_form(kind, hexstr):
    """Whether an endorser block input is a CBOR array, the pinned ledger's form of eb_references."""
    return kind == "eb" and hexstr[:1] in ("8", "9")


def compare(kind, aligned):
    totals = Counter()
    agree, differ = Counter(), Counter()
    listed, accepted, dis = Counter(), Counter(), []
    for n, (g, o, p, r) in enumerate(aligned):
        totals["values"] += 1
        if not o["ok"]:
            totals["ledger refuses"] += 1
            if p["ok"]:
                accepted[f"{g.get('mutation') or g.get('source') or 'input'}: {short(o.get('err'))}"] += 1
            continue
        totals["ledger accepts"] += 1
        if not p["ok"] and eb_list_form(kind, g["hex"]):
            listed[(expected.EB_LIST_FORM, "")] += 1
            continue
        if p["ok"] and empty_guards(kind, g["hex"]):
            listed[(expected.EMPTY_GUARDS_NEXT_NODE, "")] += 1
        if not p["ok"]:
            dis.append(dict(i=n, cls="pallas refuses ledger-valid input", key=short(p.get("err")), hex=g["hex"]))
            continue
        totals["both accept"] += 1
        notes = Notes()
        ln = normalize(o["dump"])
        pn = normalize(p["dump"], notes=notes)
        for what, path in notes.c:
            dis.append(dict(i=n, cls="pallas holds a duplicate the ledger refuses", key=f"{what} {path}", hex=g["hex"]))
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
            listed[(expected.KEEPS_INPUT, "")] += 1
            continue
        if not r["ok"]:
            dis.append(dict(i=n, cls="ledger refuses the pallas re-encoding", key=short(r.get("err")), hex=g["hex"], again=again))
            continue
        if without_ids(normalize(r["dump"])) != without_ids(ln):
            dis.append(dict(i=n, cls="pallas re-encoding holds a different ledger value", key="", hex=g["hex"], again=again))
            continue
        listed[(expected.OTHER_BYTES, "")] += 1
    return totals, agree, differ, listed, accepted, dis


def main():
    kind, gpath, opath, ppath, rpath, prefix = sys.argv[1:7]
    totals, agree, differ, listed, accepted, dis = compare(kind, rows(gpath, opath, ppath, rpath))
    with open(f"{prefix}.dis.jsonl", "w") as fh:
        for d in dis:
            fh.write(json.dumps(d, default=str) + "\n")
    with open(f"{prefix}.counts.json", "w") as fh:
        json.dump(
            dict(
                totals=totals,
                agree=agree,
                differ=differ,
                listed=[[w, p, c] for (w, p), c in sorted(listed.items())],
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
