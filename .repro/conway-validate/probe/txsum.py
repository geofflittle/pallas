import sys, cbor2
for path in sys.argv[1:]:
    raw = bytes.fromhex(open(path).read().strip())
    tx = cbor2.loads(raw)
    b = tx[0]
    print(path.split('/')[-1])
    print(' inputs', [(i[0].hex()[:12], i[1]) for i in b[0]])
    print(' outputs', [(o[1] if isinstance(o, list) else o.get(1)) for o in b[1]])
    print(' fee', b[2])
    for k in (3, 4, 5, 9, 11, 13, 14, 16, 17, 18, 19, 20, 21, 22):
        if k in b:
            v = b[k]
            if k == 13 or k == 18:
                v = [(i[0].hex()[:12], i[1]) for i in v]
            elif k == 16:
                v = (v[1] if isinstance(v, list) else v.get(1))
            print(' key', k, v)
    print(' redeemers', 5 in tx[1], 'keys', list(tx[1].keys()))
    w = tx[1]
    print(' redeemer entries', w[5] if isinstance(w[5], list) else list(w[5].keys()))
