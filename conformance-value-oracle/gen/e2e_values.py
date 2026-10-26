"""Compares every value a utxorpc v1 server serves with the ledger's decoding of the node's block bytes.

usage:
  e2e_values.py walk GRPC OUT [SLOT...]   reads every block by DumpHistory and writes OUT/block.hex,
                                          OUT/tx.hex and OUT/served.jsonl
  e2e_values.py compare OUT               runs the ledger oracle over those inputs and compares each
                                          served v1 value with the ledger dump through the v1 path table

The block a certifying header names carries no transactions, and the server serves the
transactions of the endorser block it certifies in their place. Those are compared one by one
with the ledger's decoding of each transaction, every position of every certifying block, or with
EB_SAMPLE="SEED COUNT" a sample of COUNT positions per block drawn from SEED and the slot, with
positions 0, 1 and the last always in it, and every position for the named SLOTs. Every other block
is compared whole, transactions included. The headers are linked by the ledger's own hashes, from
each previous hash to the hash of the block before, the ledger's hash of each body must equal its
header's body hash, and NODE_TIP (slot:hash, from the node's tip query) must be one of them, the
walk stopping at its slot. Each walk check (a served count or hash against the served bytes) and
each count of judged values against the walk's counts is a disagreement when it falls short.
ORACLE is the command that reads hex lines and prints ledger dumps, taking the kind as its last
argument. MAXBLOCKS stops the walk early. PLANT changes one served value before the comparison: slim flips the last byte of the
first output address and of the first input hash, bls zeroes the first served pool BLS key, eb
flips the last byte of the first output address of the first compared endorser transaction.
Exits 1 when any disagreement is found.
"""

import base64
import json
import os
import random
import shlex
import subprocess
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.environ.get("SERVED_DIR", "."))
from schemacmp import compare_one, result, tables  # noqa: E402
from served import b64, blake, call, elems, mempool, node_block  # noqa: E402
from value import normalize  # noqa: E402

PLANT = os.environ.get("PLANT", "")
NODE_TIP = os.environ.get("NODE_TIP", "")
EB_SAMPLE = os.environ.get("EB_SAMPLE", "")


def picked(slot, n, full):
    """The endorser positions compared for a certifying block of n transactions."""
    if slot in full or not EB_SAMPLE:
        return list(range(n))
    seed, count = EB_SAMPLE.split()
    chosen = set(random.Random(f"{seed}:{slot}").sample(range(n), min(int(count), n)))
    return sorted((chosen | {0, 1, n - 1}) & set(range(n)))


def walk(addr, out, full):
    os.makedirs(out, exist_ok=True)
    token = None
    c = Counter()
    checks = {}

    def check(name, passed):
        p, t = checks.get(name, (0, 0))
        checks[name] = (p + bool(passed), t + 1)

    tip = int(NODE_TIP.split(":")[0]) if NODE_TIP else None
    with open(f"{out}/block.hex", "w") as bh, open(f"{out}/tx.hex", "w") as th, open(f"{out}/served.jsonl", "w") as sj:
        while True:
            req = {"max_items": 20}
            if token:
                req["start_token"] = token
            res = call(addr, "utxorpc.v1.sync.SyncService/DumpHistory", req)
            for blk in res.get("block", []):
                nb = b64(blk["nativeBytes"])
                card = blk["cardano"]
                slot = int(card["header"].get("slot", 0))
                _, _, body, _, _, inlined, block_bytes, _ = node_block(nb)
                served = card.get("body", {}).get("tx", [])
                txs = elems(nb, body[0] + 1)
                c["blocks"] += 1
                check("served transactions equal the body elements", len(served) == len(txs))
                for st, t in zip(served, txs):
                    check("served hash equals the hash of the body bytes", b64(st.get("common", {}).get("hash")).hex() == blake(mempool(nb, t)[1]).hex())
                    c["transactions"] += 1
                bh.write(block_bytes.hex() + "\n")
                rec = dict(slot=slot, inlined=inlined, served_txs=len(served), elements=len(txs), eb=[])
                if inlined:
                    c["certifying blocks with endorser transactions"] += 1
                    c["endorser transactions served"] += len(served)
                    for i in picked(slot, len(txs), full):
                        th.write(mempool(nb, txs[i])[0].hex() + "\n")
                        rec["eb"].append(dict(pos=i, tx=served[i] if i < len(served) else None))
                        c["endorser transactions compared"] += 1
                    card.setdefault("body", {})["tx"] = []
                rec["block"] = card
                sj.write(json.dumps(rec) + "\n")
                if slot == tip:
                    c["node tip slot reached"] += 1
                    break
            token = res.get("nextToken")
            if c["node tip slot reached"]:
                break
            if not token or not res.get("block") or c["blocks"] >= int(os.environ.get("MAXBLOCKS") or 10**9):
                break
    with open(f"{out}/walk.json", "w") as fh:
        json.dump(dict(counts=c, checks=checks, eb_sample=EB_SAMPLE or "every position"), fh)
    for k, v in sorted(c.items()):
        print(f"walk {k}: {v}")
    for k, (p, t) in sorted(checks.items()):
        print(f"walk {k}: {p} of {t}")


def lines(path):
    with open(path) as fh:
        for x in fh:
            if x.strip():
                yield json.loads(x)


def oracle(kind, path):
    """The ledger dump lines of each hex line of path, read as the oracle prints them."""
    cmd = shlex.split(os.environ["ORACLE"]) + [kind]
    with open(path) as fh:
        p = subprocess.Popen(cmd, stdin=fh, stdout=subprocess.PIPE, text=True)
        for x in p.stdout:
            if x.strip():
                yield json.loads(x)
        p.wait()


def flip(b64s):
    raw = bytearray(b64(b64s))
    raw[-1] ^= 1
    return base64.b64encode(bytes(raw)).decode()


def plant(block, done):
    """Changes the one served value PLANT names, once over the whole run."""
    if not PLANT or PLANT in done:
        return
    for t in block.get("body", {}).get("tx", []):
        c = t.get("common", {})
        if PLANT == "slim" and c.get("outputs") and c.get("inputs"):
            c["outputs"][0]["address"] = flip(c["outputs"][0]["address"])
            c["inputs"][0]["txHash"] = flip(c["inputs"][0]["txHash"])
            done.add(PLANT)
            return
        if PLANT == "bls":
            for cert in c.get("certificates", []):
                key = cert.get("poolRegistration", {}).get("blsKey")
                if key and key.get("blsPubkey"):
                    key["blsPubkey"] = base64.b64encode(bytes(len(b64(key["blsPubkey"])))).decode()
                    done.add(PLANT)
                    return


def compare(out):
    tab = tables()["v1"]
    blocks, txs = result(), result()
    link, refused, eb = Counter(), Counter(), []
    anchor = NODE_TIP
    done = set()
    prev = None
    with open(f"{out}/walk.json") as fh:
        walked = json.load(fh)
    for name, (passed, total) in sorted(walked["checks"].items()):
        link[f"walk {name}"] += passed
        if passed != total:
            blocks["dis"].append(dict(slot="walk", cls=f"walk {name} in {passed} of {total}", key="walk"))
    if not walked["checks"]:
        blocks["dis"].append(dict(slot="walk", cls="the walk read no block", key="walk"))
    seen = Counter()
    for rec, o in zip(lines(f"{out}/served.jsonl"), oracle("block", f"{out}/block.hex")):
        seen["blocks"] += 1
        where = dict(slot=rec["slot"])
        if not o["ok"]:
            refused["block"] += 1
            blocks["dis"].append(dict(where, cls="ledger refuses the served block bytes", key=str(o.get("err"))[:120]))
            prev = None
            continue
        ln = normalize(o["dump"])
        hd = ln["header"]
        facts = o["facts"]
        link["ledger hash of the body equals the header body hash"] += facts["hash_of_body"] == facts["body_hash"]
        if facts["hash_of_body"] != facts["body_hash"]:
            blocks["dis"].append(dict(where, cls="ledger hash of the served body differs from the header body hash", key="header.body_hash"))
        if prev is not None:
            link["previous hash equals the hash of the block before"] += hd["prev"] == prev
            if hd["prev"] != prev:
                blocks["dis"].append(dict(where, cls="previous hash differs from the hash of the block before", key="header.prev"))
        prev = hd["hash"]
        if anchor and int(anchor.split(":")[0]) == rec["slot"]:
            link["node tip hash equals the ledger hash of its slot"] += hd["hash"] == "h:" + anchor.split(":")[1]
        if rec["inlined"]:
            link["served transactions of a certifying block equal the endorser elements"] += rec["served_txs"] == rec["elements"]
            if rec["served_txs"] != rec["elements"]:
                blocks["dis"].append(dict(where, cls="served transaction count differs from the endorser elements", key="txs"))
        elif rec["served_txs"] != rec["elements"]:
            blocks["dis"].append(dict(where, cls="served transaction count differs from the block body", key="txs"))
        plant(rec["block"], done)
        compare_one("block", "v1", ln, rec["block"], tab["block"], blocks, where)
        eb += [dict(where, pos=e["pos"], tx=e["tx"]) for e in rec["eb"]]
    for e, o in zip(eb, oracle("tx", f"{out}/tx.hex")):
        seen["endorser transactions compared"] += 1
        where = dict(slot=e["slot"], pos=e["pos"])
        if not o["ok"]:
            refused["tx"] += 1
            txs["dis"].append(dict(where, cls="ledger refuses the served endorser transaction bytes", key=str(o.get("err"))[:120]))
            continue
        if e["tx"] is None:
            txs["dis"].append(dict(where, cls="no served transaction at the endorser position", key="tx"))
            continue
        if PLANT == "eb" and PLANT not in done and e["tx"].get("common", {}).get("outputs"):
            out0 = e["tx"]["common"]["outputs"][0]
            out0["address"] = flip(out0["address"])
            done.add(PLANT)
        compare_one("tx", "v1", normalize(o["dump"]), e["tx"], tab["tx"], txs, where)
    for name in ("blocks", "endorser transactions compared"):
        if seen[name] != walked["counts"].get(name, 0):
            blocks["dis"].append(dict(slot="walk", cls=f"{name}: {seen[name]} judged of {walked['counts'].get(name, 0)} walked", key="walk"))
    print(f"endorser positions: {walked['eb_sample']}")
    if anchor and not link["node tip hash equals the ledger hash of its slot"]:
        blocks["dis"].append(dict(slot=anchor, cls="node tip not among the served blocks with its hash", key="header.hash"))
    if PLANT and PLANT not in done:
        print(f"plant {PLANT} found no value to change")
        sys.exit(2)
    status = 0
    with open(f"{out}/dis.jsonl", "w") as fh:
        for name, r in (("block", blocks), ("endorser tx", txs)):
            for d in r["dis"]:
                fh.write(json.dumps(dict(d, kind=name), default=str) + "\n")
    print(f"ledger refuses: {dict(refused) or 0}")
    for k, v in sorted(link.items()):
        print(f"linked {k}: {v}")
    for name, r in (("block", blocks), ("endorser tx", txs)):
        dis = r["dis"]
        print(
            f"v1 {name}: values {r['compared']}, field paths {len(r['agree'])}, leaves agree {sum(r['agree'].values())}, "
            f"ledger classes with no field {len(r['nofield'])}, listed differences {sum(r['cannot'].values())}, disagreements {len(dis)}"
        )
        for gp in sorted(r["agree"]):
            print(f"  {r['agree'][gp]:9d}  agree  {gp}")
        for gp in sorted(r["nofield"]):
            print(f"  {r['nofield'][gp]:9d}  listed  no field in v1  {gp}")
        for (why, gp), n in sorted(r["cannot"].items()):
            print(f"  {n:9d}  listed  {why}  {gp}")
        for (cls, key), n in sorted(Counter((d["cls"], d.get("key")) for d in dis).items(), key=lambda x: (-x[1], str(x[0]))):
            print(f"  {n:9d}  DISAGREE  {cls}  {key or ''}")
        if dis:
            status = 1
    sys.exit(status)


def main():
    cmd = sys.argv[1]
    if cmd == "walk":
        walk(sys.argv[2], sys.argv[3], {int(x) for x in sys.argv[4:]})
    elif cmd == "compare":
        compare(sys.argv[2])
    else:
        sys.exit(f"unknown command {cmd}")


if __name__ == "__main__":
    main()
