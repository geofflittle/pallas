"""Requires every location the coverage run saw to have agreeing values in each block that holds it.

usage: python3 -I check_coverage_values.py COVERAGE_LOG GEN_JSONL COUNTS_JSON HELD_JSON

COVERAGE_LOG holds the coverage test's `LOC FILE Location` lines, one per location an input held, and its
`ALL Location occ N dis D none U excluded B` lines. GEN_JSONL gives each value run index its input as `cov`.
COUNTS_JSON and HELD_JSON are the `schema.counts.json` and `schema.held.json` of `schemacmp.py block` over the
same blocks. Each location the scope does not exclude names the ledger paths that hold its value. The location
passes when at least one block holding it was compared, every compared block holding it has an agreeing v1 leaf
on those paths, and no v1 path of it is listed as having no field.
Exits 1 when a location fails, when a location has no known paths, or when the log has no locations.
"""

import json
import re
import sys

BODY = {
    0: "inputs", 1: "outputs", 2: "fee", 3: "ttl", 4: "certs", 5: "withdrawals", 7: "aux_hash", 8: "start",
    9: "mint", 11: "script_data_hash", 13: "collateral", 14: "guards", 15: "network", 16: "collateral_return",
    17: "total_collateral", 18: "reference_inputs", 19: "votes", 20: "proposals", 21: "treasury", 22: "donation",
    23: "sub_txs", 24: "top_guards", 25: "deposits", 26: "balance_intervals", 27: "starting_balance_intervals",
}
WITS = {0: "vkeys", 1: "scripts", 2: "boots", 3: "scripts", 4: "datums", 5: "redeemers", 6: "scripts", 7: "scripts", 8: "scripts"}
HEADER = {0: "block_no", 1: "slot", 10: "leios_cert", 11: "eb"}
BLOCK = {0: "txs", 1: "leios_cert", 2: "peras_cert"}


def field(prefix, name):
    return re.compile(rf"(^|\.){re.escape(prefix)}{re.escape(name)}(?![A-Za-z_])")


def paths(kind, n):
    """The pattern of the ledger paths that hold a location's value, or None when none is known."""
    if kind == "HeaderBody" and n in HEADER:
        return re.compile(rf"^header\.{HEADER[n]}(?![A-Za-z_])")
    if kind == "BlockBody" and n in BLOCK:
        return re.compile(rf"^{BLOCK[n]}(?![A-Za-z_])")
    if kind == "TxPart" and n == 2:
        return field("", "aux")
    if kind == "TxPart" and n == 3:
        return field("", "valid")
    if kind == "Body" and n in BODY:
        return field("body.", BODY[n])
    if kind == "Wits" and n in WITS:
        return field("wits.", WITS[n])
    if kind == "RedeemerTag":
        return field("wits.", "redeemers")
    if kind == "NativeClause":
        return re.compile(r"(^|\.)wits\.scripts\{\}\.script(?![A-Za-z_])")
    if kind == "CertTag":
        return field("body.", "certs")
    if kind == "PoolBls":
        return re.compile(r"(^|\.)body\.certs\[\]\.bls(?![A-Za-z_])")
    if kind == "ParamUpdate":
        return re.compile(r"(^|\.)body\.proposals\[\]\.action(?![A-Za-z_])")
    return None


LINE = re.compile(r"^ALL (\w+)(?:\((\d+)\))? occ (\d+) dis (\d+) none (\d+) excluded (true|false)$")


def main():
    log, gen_path, counts_path, held_path = sys.argv[1:5]
    unheld = list(json.load(open(counts_path))["v1"]["nofield"])
    held = {int(i): c for i, c in json.load(open(held_path))["v1"].items()}
    index = {json.loads(line)["cov"]: i for i, line in enumerate(open(gen_path)) if line.strip()}
    holders = {}
    seen = 0
    failed = 0
    for line in open(log):
        if line.startswith("LOC "):
            _, path, name = line.split()
            holders.setdefault(name, []).append(index[path])
            continue
        m = LINE.match(line.strip())
        if not m:
            continue
        kind, n, occ, excluded = m[1], int(m[2] or 0), int(m[3]), m[6] == "true"
        name = f"{kind}({n})" if m[2] else kind
        seen += 1
        if excluded:
            print(f"excluded {name} occ {occ}")
            continue
        pat = paths(kind, n)
        if pat is None:
            print(f"FAIL {name} occ {occ}: no ledger path is known for this location")
            failed += 1
            continue
        blocks = holders.get(name, [])
        compared = [i for i in blocks if i in held]
        empty = [i for i in compared if not any(pat.search(p) for p in held[i])]
        listed = sorted(p for p in unheld if pat.search(p))
        if not compared or empty or listed:
            print(f"FAIL {name} occ {occ}: blocks {len(blocks)}, compared {len(compared)}, without an agreeing value {empty}, listed {listed}")
            failed += 1
        else:
            leaves = sum(c for i in compared for p, c in held[i].items() if pat.search(p))
            print(f"compared {name} occ {occ} blocks {len(compared)} of {len(blocks)} leaves {leaves}")
    print(f"coverage values: locations {seen}, failing {failed}")
    if seen == 0 or failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
