"""Sums the comparisons of one vrun.sh output dir into the tally lines and checks the expected difference table against them.

usage: python3 -I vtally.py OUTDIR
Prints DECODE-ORACLE (valuecmp), V1-FIELDS and PRE-V1 (schemacmp), ATTACH (the redeemer of each v1 item) and
EXPECTED (the entries of expected.py and the values each matched), then one line per expected entry.
Exits 1 when a tally has a disagreement, an expected entry matched no value, or a value was listed under a class
expected.py does not hold.
"""

import glob
import json
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import expected  # noqa: E402


def lines(path):
    with open(path) as fh:
        return [json.loads(x) for x in fh if x.strip()]


def tally(outdir):
    """The summed counts of every comparison in the dir, and the match count of each listed class."""
    t = Counter()
    matched = Counter()
    for path in sorted(glob.glob(os.path.join(outdir, "*.counts.json"))):
        prefix = path[: -len(".counts.json")]
        if prefix.endswith(".schema"):
            continue
        d = json.load(open(path))
        tot = d["totals"]
        t["values"] += tot.get("values", 0)
        t["ledger_accepts"] += tot.get("ledger accepts", 0)
        t["both_accept"] += tot.get("both accept", 0)
        t["leaves_agree"] += sum(d["agree"].values())
        t["accepted_invalid"] += sum(d["accepted_invalid"].values())
        for why, _, c in d["listed"]:
            matched[why] += c
            t["listed"] += c
        t["disagree"] += len(lines(prefix + ".dis.jsonl"))
        schema = prefix + ".schema.counts.json"
        if not os.path.exists(schema):
            continue
        s = json.load(open(schema))
        dis = lines(prefix + ".schema.dis.jsonl")
        for name, r in s.items():
            for why, _, c in r["cannot"]:
                matched[why] += c
            key = "v1" if name == "v1" else "pre_v1"
            t[key + "_compared"] += r["compared"]
            t[key + "_leaves_agree"] += sum(r["agree"].values())
            t[key + "_listed"] += sum(c for _, _, c in r["cannot"])
        for a, c in s["v1"].get("attach", {}).items():
            t["attach_" + a] += c
        for d in dis:
            if d["cls"] == "v1 redeemer attachment differs":
                t["attach_disagree"] += 1
            elif d["cls"].startswith("v1 "):
                t["v1_disagree"] += 1
            else:
                t["pre_v1_disagree"] += 1
    return t, matched


def main(outdir):
    t, matched = tally(outdir)
    table = expected.VALUE + expected.SCHEMA
    unmatched = [e for e in table if not matched[e]]
    unknown = [e for e in matched if e not in table]
    print(
        f"DECODE-ORACLE values={t['values']} ledger_accepts={t['ledger_accepts']} both_accept={t['both_accept']} "
        f"leaves_agree={t['leaves_agree']} listed={t['listed']} accepted_invalid={t['accepted_invalid']} disagree={t['disagree']}"
    )
    print(f"V1-FIELDS compared={t['v1_compared']} leaves_agree={t['v1_leaves_agree']} listed={t['v1_listed']} disagree={t['v1_disagree']}")
    print(f"PRE-V1 compared={t['pre_v1_compared']} leaves_agree={t['pre_v1_leaves_agree']} listed={t['pre_v1_listed']} disagree={t['pre_v1_disagree']}")
    print(
        f"ATTACH items={t['attach_agree'] + t['attach_none'] + t['attach_differs']} agree={t['attach_agree']} "
        f"none={t['attach_none']} disagree={t['attach_disagree']}"
    )
    print(f"EXPECTED entries={len(table)} matched={len(table) - len(unmatched)} unmatched={len(unmatched)} unknown={len(unknown)}")
    for e in table:
        print(f"  {matched[e]:6d}  {e}")
    for e in unknown:
        print(f"  {matched[e]:6d}  UNKNOWN  {e}")
    bad = t["disagree"] + t["v1_disagree"] + t["pre_v1_disagree"] + t["attach_disagree"] + len(unmatched) + len(unknown)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
