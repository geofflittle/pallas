import sys
name = sys.argv[1]
root = sys.argv[2]
def edit(path, old, new, count=1):
    p = root + "/" + path
    s = open(p).read()
    n = s.count(old)
    assert n >= 1, (name, path, old[:60])
    s = s.replace(old, new) if count == 0 else s.replace(old, new, count)
    open(p, "w").write(s)
TPD = "pallas-validate/src/phase2/to_plutus_data.rs"
CW = "pallas-validate/src/phase1/conway.rs"
if name == "none":
    pass
elif name == "K-ge-9":
    edit(TPD, "protocol_major > 9", "protocol_major >= 9", 0)
elif name == "K-gt-10":
    edit(TPD, "protocol_major > 9", "protocol_major > 10", 0)
elif name == "K-redeemers-site-false":
    edit(TPD, "WithRegistrationDeposit(&tx_info.redeemers, protocol_major > 9)", "WithRegistrationDeposit(&tx_info.redeemers, false)")
elif name == "K-certs-site-false":
    edit(TPD, "WithRegistrationDeposit(&tx_info.certificates, protocol_major > 9)", "WithRegistrationDeposit(&tx_info.certificates, false)")
elif name == "K-purpose-site-false":
    edit(TPD, "WithRegistrationDeposit(purpose.as_ref(), protocol_major > 9)", "WithRegistrationDeposit(purpose.as_ref(), false)")
elif name == "M762-spend-mint-reward-unfiltered":
    p = root + "/" + CW
    s = open(p).read()
    a = s.index("fn mk_plutus_script_redeemer_pointers(")
    c = s.index("for (index, cert) in tx_body.certificates", a)
    w = s.index("if let Some(withdrawals) = &tx_body.withdrawals", c)
    e = s.index("fn sort_policies(", w)
    pre, cert, post = s[a:c], s[c:w], s[w:e]
    pre = pre.replace("let mut res: Vec<RedeemersKey> = Vec::new();", "let mut res: Vec<RedeemersKey> = Vec::new();\n    let all_refs = get_reference_script_hashes(tx_body, utxos);", 1)
    assert pre.count("reference_scripts,\n") == 2, pre.count("reference_scripts,\n")
    pre = pre.replace("reference_scripts,\n", "&all_refs,\n")
    assert post.count("reference_scripts,\n") == 1
    post = post.replace("reference_scripts,\n", "&all_refs,\n")
    open(p, "w").write(s[:a] + pre + cert + post + s[e:])
elif name == "MV1-ref-hash-as-v2":
    edit(CW, "ScriptRef::PlutusV1Script(script) => script.compute_hash(),", "ScriptRef::PlutusV1Script(script) => pallas_crypto::hash::Hasher::<224>::hash_tagged(&script.0, 2),")
elif name == "MN-eval-every-native":
    edit(CW, "        if needed\n            && !super::shelley_ma", "        if (needed || true)\n            && !super::shelley_ma")
elif name == "M762-mint-reward-unfiltered":
    p = root + "/" + CW
    s = open(p).read()
    a = s.index("fn mk_plutus_script_redeemer_pointers(")
    c = s.index("for (index, cert) in tx_body.certificates", a)
    w = s.index("if let Some(withdrawals) = &tx_body.withdrawals", c)
    e = s.index("fn sort_policies(", w)
    pre, cert, post = s[a:c], s[c:w], s[w:e]
    pre = pre.replace("let mut res: Vec<RedeemersKey> = Vec::new();", "let mut res: Vec<RedeemersKey> = Vec::new();\n    let all_refs = get_reference_script_hashes(tx_body, utxos);", 1)
    i = pre.index("reference_scripts,\n"); j = pre.index("reference_scripts,\n", i + 1)
    pre = pre[:j] + "&all_refs,\n" + pre[j + len("reference_scripts,\n"):]
    post = post.replace("reference_scripts,\n", "&all_refs,\n")
    open(p, "w").write(s[:a] + pre + cert + post + s[e:])
elif name == "MR-unreg-refund-zero":
    edit(CW, "Certificate::UnReg(_, refund) | Certificate::UnRegDRepCert(_, refund) => (0, *refund),", "Certificate::UnReg(_, _refund) => (0, 0),\n        Certificate::UnRegDRepCert(_, refund) => (0, *refund),")
elif name == "MR-drep-refund-zero":
    edit(CW, "Certificate::UnReg(_, refund) | Certificate::UnRegDRepCert(_, refund) => (0, *refund),", "Certificate::UnReg(_, refund) => (0, *refund),\n        Certificate::UnRegDRepCert(_, _refund) => (0, 0),")
elif name == "MR-dereg-refund-zero":
    edit(CW, "Certificate::StakeDeregistration(..) => (0, key_deposit),", "Certificate::StakeDeregistration(..) => (0, 0),")
elif name == "MW-remaining-first-only":
    edit(CW, "if !*covered && !verify_signature(vkey_wit, data_to_verify) {\n            return Err(PostAlonzo(VKWrongSignature));\n        }", "if !*covered {\n            return if verify_signature(vkey_wit, data_to_verify) { Ok(()) } else { Err(PostAlonzo(VKWrongSignature)) };\n        }")
elif name == "MS-deleg-unwrapped":
    edit(TPD, "                        WithWrappedStakeCredential(stake_credential).to_plutus_data(),\n                        pool_keyhash.to_plutus_data(),", "                        stake_credential.to_plutus_data(),\n                        pool_keyhash.to_plutus_data(),")
elif name == "PB-babbage-ref-collateral-1":
    p = root + "/pallas-validate/tests/babbage.rs"
    s = open(p).read()
    a = s.index("fn successful_mainnet_tx_with_plutus_v2_script")
    e = s.index("    fn ", a + 10)
    body = s[a:e]
    assert body.count("Value::Coin(5000000),") == 1
    body = body.replace("Value::Coin(5000000),", "Value::Coin(1),")
    open(p, "w").write(s[:a] + body + s[e:])
else:
    raise SystemExit("unknown mutant " + name)
print("mutated", name)
