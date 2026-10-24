"""Prints the generated input, the oracle line and the probe line of chosen records.

usage: python3 -I pick.py OUT_DIR NAME I [I ...]
"""

import json
import os
import sys


def main():
    out, name = sys.argv[1], sys.argv[2]
    want = {int(x) for x in sys.argv[3:]}
    gen = [json.loads(x) for x in open(os.path.join(out, name + ".gen.jsonl")) if x.strip()]
    oracle = [json.loads(x) for x in open(os.path.join(out, name + ".oracle.jsonl")) if x.strip()]
    probe = {}
    for x in open(os.path.join(out, name + ".probe.jsonl")):
        if x.strip():
            d = json.loads(x)
            probe[d["i"]] = d
    for k, g in enumerate(gen):
        if g["i"] in want:
            o = dict(oracle[k])
            o.pop("show", None)
            p = probe.get(g["i"], {})
            print(json.dumps({"name": name, "i": g["i"], "hex": g["hex"], "oracle": o,
                              "probe_err": p.get("err"), "mapper_err": p.get("mapper_err")}))


if __name__ == "__main__":
    main()
