import hashlib, sys
def head(b, i):
    ib = b[i]; mt = ib >> 5; ai = ib & 31; i += 1
    if ai < 24: return mt, ai, i
    n = {24:1,25:2,26:4,27:8}.get(ai)
    if n is None: return mt, None, i  # indefinite
    return mt, int.from_bytes(b[i:i+n], 'big'), i + n
def skip(b, i):
    mt, v, i = head(b, i)
    if mt in (0, 1, 7): return i
    if mt in (2, 3):
        if v is None:
            while b[i] != 0xff: i = skip(b, i)
            return i + 1
        return i + v
    if mt == 6: return skip(b, i)
    k = 2 if mt == 5 else 1
    if v is None:
        while b[i] != 0xff: i = skip(b, i)
        return i + 1
    for _ in range(v * k): i = skip(b, i)
    return i
root = sys.argv[1]
for name in sys.argv[2:]:
    body = bytes.fromhex(open(f"{root}/{name}.ebbody").read().strip())
    mt, n, i = head(body, 0)
    _, klen, j = head(body, i)
    key = body[j:j+klen]; _, size, _ = head(body, j + klen)
    tx = bytes.fromhex(open(f"{root}/{name}.ebtxs").read().split()[0])
    _, tl, t = head(tx, 0); inner = tx[t:t+tl]
    _, _, a = head(inner, 0); e = skip(inner, a)
    txid = hashlib.blake2b(inner[a:e], digest_size=32).hexdigest()
    ntx = len(open(f"{root}/{name}.ebtxs").read().split())
    print(name, len(body), n, ntx, key.hex(), size, len(inner), txid, hashlib.blake2b(body, digest_size=32).hexdigest())
