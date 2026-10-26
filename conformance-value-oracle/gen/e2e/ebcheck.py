"""Checks each served certifying block's inlined endorser transactions against the endorser block the node announced.

usage: ebcheck.py GRPC OUT

For every block DumpHistory serves, the script keeps the eb_references_announcement of the header. For a block whose
served body is longer than its header body size (endorser transactions inlined), it rebuilds the eb_references
as a map from transaction hash to transaction size (FORM=array for `[+ [transaction_hash, transaction_size]]`) from the inlined transactions (the hash of each whole transaction and its length) and compares its blake2b 256 hash and size
with the announcement of the previous block. A second arm changes one byte of one inlined transaction and requires a
mismatch. It also records every served transaction that carries proposals.
Exits 1 unless every inlined block agrees with the announcement and every planted byte is refused.
"""

import json
import os
import sys
from collections import Counter

sys.path.insert(0, os.environ["SERVED_DIR"])
from served import b64, blake, call, elems, end, head, uint  # noqa: E402


def enc_uint(n, major=0):
    m = major << 5
    if n < 24:
        return bytes([m | n])
    for info, size in ((24, 1), (25, 2), (26, 4), (27, 8)):
        if n < 1 << (8 * size):
            return bytes([m | info]) + n.to_bytes(size, "big")


FORM = os.environ.get("FORM", "map")
TIP = int(os.environ["NODE_TIP"].split(":")[0]) if os.environ.get("NODE_TIP") else None


def refs_cbor(txs):
    if FORM == "array":
        out = enc_uint(len(txs), 4)
        for body, size in txs:
            out += b"\x82\x58\x20" + blake(body) + enc_uint(size)
        return out
    out = enc_uint(len(txs), 5)
    for body, size in txs:
        out += b"\x58\x20" + blake(body) + enc_uint(size)
    return out


def main():
    addr, out = sys.argv[1:3]
    os.makedirs(out, exist_ok=True)
    c = Counter()
    bad = []
    props = []
    prev_ann = None
    token = None
    with open(f"{out}/blocks.facts.jsonl", "w") as fh:
        while True:
            req = {"max_items": 5}
            if token:
                req["start_token"] = token
            res = call(addr, "utxorpc.v1.sync.SyncService/DumpHistory", req)
            for blk in res.get("block", []):
                nb = b64(blk["nativeBytes"])
                card = blk["cardano"]
                slot = int(card["header"].get("slot", 0))
                c["blocks"] += 1
                hb = elems(nb, elems(nb, 3)[0][0])
                hb_start = hb[0][0]
                body_size = uint(nb, hb[6][0])
                ann_s, ann_e = hb[-1]
                ann = None
                if nb[ann_s] != 0xF6:
                    a = elems(nb, ann_s)
                    _, _, p = head(nb, a[0][0])
                    ann = (nb[p:a[0][1]].hex(), uint(nb, a[1][0]))
                body_start = end(nb, 3)
                inlined = len(nb) - body_start != body_size
                served_txs = card.get("body", {}).get("tx", [])
                for i, t in enumerate(served_txs):
                    if t.get("common", {}).get("proposals"):
                        props.append(dict(slot=slot, pos=i, inlined=inlined, hash=b64(t["common"].get("hash")).hex(),
                                          proposals=t["common"]["proposals"]))
                rec = dict(slot=slot, inlined=inlined, ann=ann, txs=len(served_txs))
                if inlined:
                    c["inlined"] += 1
                    txs = elems(nb, body_start + 1)
                    # Each reference is the blake2b 256 of the whole transaction bytes and their length.
                    full = [(nb[s:e], e - s) for s, e in txs]
                    eb = refs_cbor(full)
                    h = blake(eb).hex()
                    rec.update(eb_hash=h, eb_size=len(eb), n=len(txs))
                    if prev_ann is None:
                        c["inlined without a previous announcement"] += 1
                    else:
                        okh = h == prev_ann[0]
                        oks = len(eb) == prev_ann[1]
                        c[f"hash equals previous announcement {okh}, size equals {oks}"] += 1
                        if not okh and not bad:
                            with open(f"{out}/first.json", "w") as ff:
                                json.dump(dict(slot=slot, announced=prev_ann, nb=nb.hex()), ff)
                        if not okh and len(bad) < 20:
                            bad.append(dict(slot=slot, rebuilt=h, size=len(eb), announced=prev_ann, n=len(txs)))
                        if okh:
                            # Plant: one byte of the last inlined transaction body changed.
                            b0, s0 = full[-1]
                            planted = full[:-1] + [(b0[:-1] + bytes([b0[-1] ^ 1]), s0)]
                            c[f"planted hash equals previous announcement {blake(refs_cbor(planted)).hex() == prev_ann[0]}"] += 1
                fh.write(json.dumps(rec) + "\n")
                prev_ann = ann
            token = res.get("nextToken")
            if TIP and any(int(b["cardano"]["header"].get("slot", 0)) == TIP for b in res.get("block", [])):
                break
            if c["blocks"] >= int(os.environ.get("MAXBLOCKS") or 10**9):
                break
            if not token or not res.get("block"):
                break
    with open(f"{out}/proposals.jsonl", "w") as fh:
        for p in props:
            fh.write(json.dumps(p) + "\n")
    with open(f"{out}/bad.jsonl", "w") as fh:
        for b in bad:
            fh.write(json.dumps(b) + "\n")
    for k, v in sorted(c.items()):
        print(f"{k}: {v}")
    print(f"transactions with proposals: {len(props)}")
    for p in props:
        print(f"  slot {p['slot']} pos {p['pos']} inlined {p['inlined']} hash {p['hash']} kinds {[list(x.get('govAction', {})) for x in p['proposals']]}")
    agree = c["hash equals previous announcement True, size equals True"]
    fired = c["planted hash equals previous announcement False"]
    print(f"ebcheck: inlined blocks {c['inlined']}, announcement agrees {agree}, planted byte refused {fired}")
    if not c["inlined"] or agree != c["inlined"] or fired != c["inlined"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
