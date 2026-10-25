"""Summarises a vrun.sh log and its mutate counts into the per kind lines, listed classes and invalid acceptance counts.

usage: vsummary.py LOG OUTDIR
Prints, for each kind and source in log order, the value and schema summary lines, then each
listed class summed over its field paths, then for each kind the ledger refusals pallas accepts,
by mutator and by the ledger's error class, read from OUTDIR/KIND.mutate.counts.json.
Exits 1 when any summary line reports a disagreement or a nonzero leaves differ.
"""

import json
import os
import re
import sys
from collections import OrderedDict, defaultdict

SUMMARY = re.compile(r"^(value|schema) ")


def main(log, outdir):
    sources = ["arb", "cddl-plain", "cddl", "mutate"]
    seen = defaultdict(int)
    bad = 0
    classes = OrderedDict()
    current = None
    for line in open(log):
        line = line.rstrip("\n")
        if SUMMARY.match(line):
            words = line.split()
            kind = words[1].rstrip(":") if words[0] == "value" else words[2].rstrip(":")
            if words[0] == "value":
                src = sources[seen[kind] % 4]
                seen[kind] += 1
            else:
                src = sources[(seen[kind] - 1) % 4]
            schema = "value" if words[0] == "value" else words[1]
            current = (kind, src, schema)
            print(f"{kind} {src}: {line}")
            if "disagreements 0 on 0 values" not in line or ("leaves differ" in line and "leaves differ 0," not in line):
                bad += 1
            continue
        parts = re.split(r"\s{2,}", line.strip())
        if current and len(parts) >= 3 and parts[1] == "listed" and parts[0].isdigit():
            n, reason = int(parts[0]), parts[2]
            key = (current[0], current[1], current[2], reason)
            c = classes.setdefault(key, [0, 0])
            c[0] += n
            c[1] += 1
    print()
    print("listed classes, summed over field paths: kind source schema, count, paths, class")
    for (kind, src, schema, reason), (n, paths) in classes.items():
        print(f"{kind} {src} {schema}  {n}  {paths}  {reason}")
    print()
    print("ledger refusals pallas accepts, mutate source")
    for kind in seen:
        path = os.path.join(outdir, f"{kind}.mutate.counts.json")
        if not os.path.exists(path):
            print(f"{kind}: no mutate counts")
            continue
        d = json.load(open(path))
        acc = d.get("accepted_invalid", {})
        total = sum(acc.values())
        bym = defaultdict(int)
        for k, v in acc.items():
            bym[k.split(":", 1)[0]] += v
        t = d.get("totals", {})
        print(f"{kind}: values {t.get('values')}, ledger refuses {t.get('ledger refuses')}, pallas accepts {total} of them, by mutator " + ", ".join(f"{m} {n}" for m, n in sorted(bym.items(), key=lambda x: -x[1])))
        for k, v in sorted(acc.items(), key=lambda x: (-x[1], x[0])):
            print(f"  {v:5d}  {k}")
    print()
    print(f"summary lines with a disagreement or a differing leaf: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
