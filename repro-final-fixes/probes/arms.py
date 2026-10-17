# usage: python3 arms.py <arm> <tree>   applies one single line mutation, asserting the old text occurs once
import sys, pathlib
ARMS = {
    "a1-shelley-cert-key": ("pallas-validate/src/phase1/shelley_ma.rs",
        "            check_vk_wit(key_hash, tx_hash, vk_wits)?;\n",
        "            let _ = key_hash;\n"),
    "a2-v3-deposit-boundary": ("pallas-validate/src/phase2/to_plutus_data.rs",
        "WithRegistrationDeposit(&tx_info.certificates, protocol_major > 9)",
        "WithRegistrationDeposit(&tx_info.certificates, protocol_major >= 9)"),
    "a3-alonzo-cert-native": ("pallas-validate/src/phase1/alonzo.rs",
        "    check_certificate_native_scripts(\n        &MultiEraTx::from_alonzo_compatible(mtx, Era::Alonzo),\n        utxos,\n        Alonzo(NativeScriptDenial),\n    )?;\n",
        "    let _ = check_certificate_native_scripts(\n        &MultiEraTx::from_alonzo_compatible(mtx, Era::Alonzo),\n        utxos,\n        Alonzo(NativeScriptDenial),\n    );\n"),
    "a4-bootstrap-signature": ("pallas-validate/src/utils.rs",
        "            if !verify_bootstrap_signature(wit, data_to_verify) {\n",
        "            if false && !verify_bootstrap_signature(wit, data_to_verify) {\n"),
    "a5-babbage-needed-language": ("pallas-validate/src/phase1/babbage.rs",
        "        if !needed.contains(&script_ref_hash(&script)) {\n",
        "        if false && !needed.contains(&script_ref_hash(&script)) {\n"),
    "a6-alonzo-collateral-trigger": ("pallas-validate/src/phase1/alonzo.rs",
        "    if !option_vec_is_empty(&mtx.transaction_witness_set.redeemer) {\n        check_collaterals",
        "    if !option_vec_is_empty(&mtx.transaction_witness_set.plutus_script) {\n        check_collaterals"),
}
arm, tree = sys.argv[1], pathlib.Path(sys.argv[2])
path, old, new = ARMS[arm]
p = tree / path
text = p.read_text()
assert text.count(old) == 1, (arm, text.count(old))
p.write_text(text.replace(old, new))
print("armed", arm, "in", path)
