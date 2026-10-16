# Applies the blind lens arms to an extracted copy of the fix branch tip.
# Usage: python3 -I apply_arms_tip.py <tree> <this dir>
# Differences from the 6b71fdeb apparatus: check_preservation_of_value takes the
# DRep and proposal deposits, and the two chain fixture tests already supply the
# chain values, so F_result prints the validate_txs result of those two tests.
import os, shutil, sys

t, a = sys.argv[1], sys.argv[2]
shutil.copy(os.path.join(a, 'blind_arms.rs'), os.path.join(t, 'pallas-validate/src/phase1/conway/blind_arms.rs'))
p = os.path.join(t, 'pallas-validate/src/phase1/conway.rs')
s = open(p).read()
anchor = '#[cfg(test)]\nmod registration_deposits;\n'
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + '#[cfg(test)]\nmod blind_arms;\n', 1)
open(p, 'w').write(s)

p = os.path.join(t, 'pallas-validate/src/phase2/script_context.rs')
s = open(p).read().rstrip()
assert s.endswith('}')
s = s[:-1] + open(os.path.join(a, 'phase2_arms.rs.snippet')).read() + '}\n'
open(p, 'w').write(s)

p = os.path.join(t, 'pallas-validate/tests/conway.rs')
s = open(p).read()
old = ('        match validate_txs(std::slice::from_ref(&metx), &env, &utxos, &mut cert_state) {\n'
       '            Ok(()) => (),\n'
       '            Err(err) => panic!("Unexpected error ({err:?})"),\n'
       '        };\n')
new = ('        let result = validate_txs(std::slice::from_ref(&metx), &env, &utxos, &mut cert_state);\n'
       '        println!("ARM F_result {result:?}");\n')
for name in ['fn successful_preview_tx_with_plutus_v3_script', 'fn successful_mainnet_tx_with_plutus_v3_script']:
    start = s.index(name)
    at = s.index(old, start)
    nxt = s.find('#[test]', start)
    assert at < nxt, name
    s = s[:at] + new + s[at + len(old):]
open(p, 'w').write(s)
print('arms applied')
