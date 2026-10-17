import sys
import cbor2

# usage: redeemer_totals.py <tx hex file>...
# prints the redeemer count and the summed mem and steps of each transaction
for path in sys.argv[1:]:
    tx = cbor2.loads(bytes.fromhex(open(path).read().strip()))
    wits = tx[1]
    reds = wits.get(5)
    if reds is None:
        items = []
    elif isinstance(reds, dict):
        items = [v for v in reds.values()]
        items = [(v[1][0], v[1][1]) for v in items]
    else:
        items = [(r[3][0], r[3][1]) for r in reds]
    mem = sum(m for m, _ in items)
    steps = sum(s for _, s in items)
    shape = type(reds).__name__
    scripts = {k: len(wits[k]) for k in (3, 6, 7) if k in wits}
    print("TOTALS %s shape=%s redeemers=%d mem=%d steps=%d witness_plutus=%s fee=%d" % (path.split("/")[-1], shape, len(items), mem, steps, scripts, tx[0][2]))
