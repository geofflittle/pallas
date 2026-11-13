# Builds synthetic Dijkstra transactions and blocks, one construct each, from the
# w42 sample T2 (a direct PlutusV4 spend). Each output file adds one construct the
# live chain and the samples do not carry, so the probe can say per construct
# whether pallas decodes it and whether the v1 mapper sets the field.
# Usage: python3 -I synth.py <w42 samples dir> <out dir>
import sys, os, copy, cbor2
from cbor2 import CBORTag

SAMPLES, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
base = cbor2.loads(open(os.path.join(SAMPLES, 'tx', 't2_v4_direct.cbor'), 'rb').read())
block_src = cbor2.loads(open(os.path.join(SAMPLES, 'rb', 'rb_00008_slot353.cbor'), 'rb').read())

H28 = lambda b: bytes([b]) * 28
H32 = lambda b: bytes([b]) * 32
RA = lambda b: bytes([0xe0]) + bytes([b]) * 28
KEY = lambda b: [0, H28(b)]
SCRIPT = lambda b: [1, H28(b)]
Q = lambda n, d: CBORTag(30, [n, d])
V4 = bytes.fromhex('4e4d01000033222220051200120011')
V3 = bytes.fromhex('4e4d01000033222220051200120012')
V2 = bytes.fromhex('4e4d01000033222220051200120013')
V1 = bytes.fromhex('4e4d01000033222220051200120014')
ADDR = base[0][1][0][0] if isinstance(base[0][1][0], dict) else base[0][1][0][0]
REQ_GUARD = [6, KEY(0x41)]
ANCHOR = ['https://example.invalid/a', H32(0x22)]

PPU_NEW = {
    18: {0: [1, 2], 3: [7, 8, 9]},
    34: 101, 35: 102, 36: 103, 37: Q(5, 2), 38: Q(3, 1), 39: Q(1, 50),
    40: 104, 41: 105, 42: 106, 43: 107, 44: Q(2, 3), 45: 108, 46: 109, 47: [110, 111], 48: 112,
    49: 113, 50: Q(7, 3), 51: 114, 52: 115, 53: 116, 54: Q(1, 9), 55: 117, 56: 118,
}
PPU_NULLS = {38: None, 53: None}
PPU_OLD = {0: 44, 1: 155381, 17: 4310, 33: Q(15, 1)}


def tx(body_extra=None, wit_extra=None, aux=None, out_extra=None):
    t = copy.deepcopy(base)
    body, wits = t[0], t[1]
    for k, v in (body_extra or {}).items():
        body[k] = v
    for k, v in (wit_extra or {}).items():
        wits[k] = v
    if out_extra:
        body[1] = list(body[1]) + out_extra
    return [body, wits, aux]


def output_with_ref(script):
    return {0: ADDR, 1: 2000000, 3: CBORTag(24, cbor2.dumps(script))}


def pool_cert(bls):
    params = [H28(0x02), H32(0x06)]
    if bls != 'absent':
        params.append(bls)
    params += [500, 340, Q(1, 10), RA(0x05), CBORTag(258, [H28(0x04)]), [], None]
    return [3] + params


def proposal(action):
    return [1000, RA(0x07), action, ANCHOR]


cases = {
    'witness_v4': tx(wit_extra={8: CBORTag(258, [V4])}),
    'witness_v3': tx(wit_extra={7: CBORTag(258, [V3])}),
    'witness_native_require_guard': tx(wit_extra={1: CBORTag(258, [REQ_GUARD, [1, [REQ_GUARD, [0, H28(0x42)]]]])}),
    'redeemer_guarding': tx(body_extra={14: CBORTag(258, [KEY(0x41)])},
                            wit_extra={5: {(6, 0): [42, [1, 2]]}}),
    'aux_v1': tx(aux=CBORTag(259, {2: [V1]})),
    'aux_v2': tx(aux=CBORTag(259, {3: [V2]})),
    'aux_v3': tx(aux=CBORTag(259, {4: [V3]})),
    'aux_v4': tx(aux=CBORTag(259, {5: [V4]})),
    'aux_native_require_guard': tx(aux=CBORTag(259, {1: [REQ_GUARD]})),
    'script_ref_v4': tx(out_extra=[output_with_ref([4, V4])]),
    'script_ref_native_require_guard': tx(out_extra=[output_with_ref([0, REQ_GUARD])]),
    'cert_pool_bls_key': tx(body_extra={4: CBORTag(258, [pool_cert([b'\x11' * 96, b'\x12' * 48])])}),
    'cert_pool_bls_nil': tx(body_extra={4: CBORTag(258, [pool_cert(None)])}),
    'cert_pool_bls_absent': tx(body_extra={4: CBORTag(258, [pool_cert('absent')])}),
    'guards_keyhash': tx(body_extra={14: CBORTag(258, [H28(0x41)])}),
    'guards_credential': tx(body_extra={14: CBORTag(258, [KEY(0x41), SCRIPT(0x43)])}),
    'required_top_level_guards': tx(body_extra={24: {(0, H28(0x44)): 42, (1, H28(0x45)): None}}),
    'direct_deposits': tx(body_extra={25: {RA(0x46): 1000}}),
    'account_balance_intervals': tx(body_extra={26: {RA(0x47): [1, None], RA(0x48): [None, 5], RA(0x49): [2, 6], RA(0x4a): 7}}),
    'starting_account_balance_intervals': tx(body_extra={27: {RA(0x4b): [3, 9], RA(0x4c): 11}}),
    'treasury_value_and_donation': tx(body_extra={21: 5000, 22: 6}),
    'proposal_info': tx(body_extra={20: CBORTag(258, [proposal([6])])}),
    'proposal_ppu_old_keys': tx(body_extra={20: CBORTag(258, [proposal([0, None, PPU_OLD, None])])}),
    'proposal_ppu_new_keys': tx(body_extra={20: CBORTag(258, [proposal([0, None, PPU_NEW, None])])}),
    'proposal_ppu_nulls': tx(body_extra={20: CBORTag(258, [proposal([0, None, PPU_NULLS, None])])}),
}

sub_body = {0: CBORTag(258, [[H32(0x51), 0]]), 1: [{0: ADDR, 1: 1500000}],
            14: CBORTag(258, [KEY(0x41)]), 24: {(0, H28(0x44)): None}, 25: {RA(0x46): 10},
            26: {RA(0x47): 3}}
cases['sub_transaction_dijkstra_keys'] = tx(body_extra={23: CBORTag(258, [[sub_body, {8: CBORTag(258, [V4])}, CBORTag(259, {5: [V4]})]])})

for name, t in cases.items():
    open(os.path.join(OUT, 'tx_%s.cbor' % name), 'wb').write(cbor2.dumps(t))

# A block that carries every block level construct: a header that announces an
# endorser block and sets the certificate flag, a body with a Leios and a Peras
# certificate, and two transactions, one marked failed.
era, blk = block_src
header, body = blk
hb = list(header[0])
hb[10] = True
hb[11] = [H32(0x61), 3665]
header = [hb, header[1]]
full = cases['direct_deposits']
txs = [[full[0], full[1], full[2], True], [cases['witness_v4'][0], cases['witness_v4'][1], None, False]]
blk2 = [header, [txs, [b'\xff\x01', b'\x13' * 48], b'\x14' * 32]]
open(os.path.join(OUT, 'block_leios_peras.cbor'), 'wb').write(cbor2.dumps([era, blk2]))
print('wrote', len(cases), 'transactions and 1 block to', OUT)

# One block per transaction case, for the fork's chain oracle coverage check,
# which reads whole blocks.
for name, t in cases.items():
    b = [header, [[[t[0], t[1], t[2], True]], None, None]]
    open(os.path.join(OUT, 'blk_%s.cbor' % name), 'wb').write(cbor2.dumps([era, b]))
