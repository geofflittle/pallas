"""A grpcurl stand in that runs the real grpcurl and changes one value in the first DumpHistory page that has a target.

usage: GRPCURL=liar.py LIE=MODE REAL_GRPCURL=path e2e-run.sh ...
modes:
  midtx   flips the last byte of the first output address of the served endorser transaction at
          position 1 of the first certifying block serving three or more transactions
  txhash  flips the last byte of the served hash of the first transaction of the first block with one
  droptx  drops the last served transaction of the first block whose body is not inlined and serves two or more
  body    flips the last byte of the first output address of the first transaction of the first block
          whose body is not inlined, in the served value and at the same bytes of nativeBytes, so the
          served value and the served block bytes agree with each other and differ from the node's
  ebbytes flips the last byte of the last inlined endorser transaction of the first certifying block, in
          nativeBytes only, so the inlined transactions no longer hash to the previous announcement
The lie is told once per calling process, recorded in LIE_DONE.PID with the line that names it.
"""

import base64
import json
import os
import subprocess
import sys

REAL = os.environ.get("REAL_GRPCURL", "grpcurl")
LIE = os.environ.get("LIE", "")
DONE = f"{os.environ.get('LIE_DONE', '/tmp/lie-done')}.{os.getppid()}"


def flip(s):
    raw = bytearray(base64.b64decode(s))
    raw[-1] ^= 1
    return base64.b64encode(bytes(raw)).decode()


def served():
    sys.path.insert(0, os.environ["SERVED_DIR"])
    import served as s

    return s


def lie(res):
    s = served()
    for blk in res.get("block", []):
        nb = bytearray(base64.b64decode(blk["nativeBytes"]))
        txs = blk.get("cardano", {}).get("body", {}).get("tx", [])
        hdr = blk["cardano"]["header"]
        inlined = s.node_block(bytes(nb))[5]
        if LIE == "midtx" and len(txs) >= 3 and inlined:
            outs = txs[1].get("common", {}).get("outputs")
            if outs:
                outs[0]["address"] = flip(outs[0]["address"])
                return f"midtx slot {hdr.get('slot')} pos 1 of {len(txs)}"
        if LIE == "txhash" and txs:
            c = txs[0].setdefault("common", {})
            c["hash"] = flip(c["hash"])
            return f"txhash slot {hdr.get('slot')}"
        if LIE == "droptx" and len(txs) >= 2 and not inlined:
            txs.pop()
            return f"droptx slot {hdr.get('slot')} of {len(txs) + 1}"
        if LIE == "body" and txs and not inlined:
            outs = txs[0].get("common", {}).get("outputs")
            if not outs:
                continue
            addr = base64.b64decode(outs[0]["address"])
            at = nb.find(addr)
            if at < 0:
                continue
            nb[at + len(addr) - 1] ^= 1
            blk["nativeBytes"] = base64.b64encode(bytes(nb)).decode()
            outs[0]["address"] = flip(outs[0]["address"])
            return f"body slot {hdr.get('slot')} byte {at + len(addr) - 1}"
        if LIE == "ebbytes" and inlined:
            body_start = s.end(bytes(nb), 3)
            last = s.elems(bytes(nb), body_start + 1)[-1]
            nb[last[1] - 1] ^= 1
            blk["nativeBytes"] = base64.b64encode(bytes(nb)).decode()
            return f"ebbytes slot {hdr.get('slot')} byte {last[1] - 1}"
    return None


def main():
    out = subprocess.run([REAL] + sys.argv[1:], check=True, capture_output=True, text=True).stdout
    if "DumpHistory" in " ".join(sys.argv) and LIE and not os.path.exists(DONE):
        res = json.loads(out)
        told = lie(res)
        if told:
            with open(DONE, "w") as fh:
                fh.write(told + "\n")
            out = json.dumps(res)
    sys.stdout.write(out)


if __name__ == "__main__":
    main()
