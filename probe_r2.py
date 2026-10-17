import sys
# usage: probe_r2.py <tree> <arm>
# arm: head | old-fee-trigger | redeemer-exunits-trigger
root = sys.argv[1]
arm = sys.argv[2]
T = root + "/pallas-validate/tests/"
SRC = root + "/pallas-validate/src/phase1/"


def rd(p):
    return open(p).read()


def wr(p, s):
    open(p, "w").write(s)


def fn_span(s, name):
    a = s.index("    fn " + name + "(")
    ends = [i for i in (s.find("\n    #[test]", a), s.find("\n    fn mk_", a)) if i != -1]
    return a, min(ends)


def clone(s, name, new, subs, cut_from, tail):
    a, e = fn_span(s, name)
    body = s[a:e]
    body = body.replace("fn " + name + "(", "fn " + new + "(", 1)
    for old, rep in subs:
        assert body.count(old) == 1, (new, old[:50], body.count(old))
        body = body.replace(old, rep)
    c = body.index(cut_from)
    body = body[:c] + tail + "    }\n"
    return s[:e] + "\n\n    #[test]\n" + body + s[e:]


# Alonzo: four witness edits times two collateral amounts
p = T + "alonzo.rs"
s = rd(p)
edits = {
    "keep": "",
    "noscripts": "tx_wits.plutus_script = Some(Vec::new());",
    "noredeemers": "tx_wits.redeemer = None;",
    "neither": "tx_wits.plutus_script = Some(Vec::new()); tx_wits.redeemer = None;",
}
for ename, edit in edits.items():
    for cname, coin in (("short", "1236118"), ("ample", "5000000")):
        n = "probe_alonzo_%s_%s" % (ename, cname)
        tail = (
            '        let r = validate_txs(&[metx], &env, &utxos, &mut cert_state);\n'
            '        eprintln!("PROBE %s {:?}", r);\n' % n
        )
        s = clone(
            s,
            "redeemers_without_scripts_require_collateral",
            n,
            [
                ("Value::Coin(1236118)", "Value::Coin(%s)" % coin),
                ("tx_wits.plutus_script = Some(Vec::new());", edit),
            ],
            "        match validate_txs(",
            tail,
        )
wr(p, s)

# Babbage: reference script tx with ample collateral, max ex units default or 1
p = T + "babbage.rs"
s = rd(p)
for pname, params in (
    ("default", "mk_mainnet_params_epoch_380()"),
    ("max1", "{ let mut p = mk_mainnet_params_epoch_380(); p.max_tx_ex_units.mem = 1; p.max_tx_ex_units.steps = 1; p }"),
):
    n = "probe_babbage_refscript_exunits_" + pname
    tail = (
        '        let r = validate_txs(&[metx], &env, &utxos, &mut cert_state);\n'
        '        eprintln!("PROBE %s {:?}", r);\n' % n
    )
    s = clone(
        s,
        "reference_script_tx_with_collateral_one_lovelace_short",
        n,
        [
            ("Value::Coin(613885)", "Value::Coin(5000000)"),
            ("MultiEraProtocolParameters::Babbage(mk_mainnet_params_epoch_380())", "MultiEraProtocolParameters::Babbage(%s)" % params),
        ],
        "        match validate_txs(",
        tail,
    )
wr(p, s)

# Conway: V3 reference script tx (conway5) and the preview V3 tx, max ex units default or 1
p = T + "conway.rs"
s = rd(p)
for src, fname, mk in (
    ("successful_mainnet_tx_with_plutus_v3_script", "refscript", "mk_mainnet_params_epoch_380"),
    ("successful_preview_tx_with_plutus_v3_script", "preview", "mk_preview_params_epoch_380"),
):
    for pname, params in (
        ("default", mk + "()"),
        ("max1", "{ let mut p = %s(); p.max_tx_ex_units.mem = 1; p.max_tx_ex_units.steps = 1; p }" % mk),
    ):
        n = "probe_conway_%s_exunits_%s" % (fname, pname)
        tail = (
            '        let r = validate_txs(std::slice::from_ref(&metx), &env, &utxos, &mut cert_state);\n'
            '        eprintln!("PROBE %s wits_v3={} redeemers={} {:?}", metx.plutus_v3_scripts().len(), metx.redeemers().len(), r);\n' % n
        )
        s = clone(
            s,
            src,
            n,
            [("MultiEraProtocolParameters::Conway(%s())" % mk, "MultiEraProtocolParameters::Conway(%s)" % params)],
            "        match validate_txs(",
            tail,
        )
wr(p, s)

# Fixture census: redeemers present versus Plutus witness scripts present
wr(T + "probe_triggers.rs", r'''
use pallas_traverse::{MultiEraBlock, MultiEraTx};

fn row(file: &str, i: usize, tx: &MultiEraTx, tot: &mut [usize; 4]) {
    let red = !tx.redeemers().is_empty();
    let wit = !tx.plutus_v1_scripts().is_empty()
        || !tx.plutus_v2_scripts().is_empty()
        || !tx.plutus_v3_scripts().is_empty();
    tot[0] += 1;
    if red { tot[1] += 1; }
    if wit { tot[2] += 1; }
    if red != wit {
        tot[3] += 1;
        eprintln!("DIFF {file} tx={i} redeemers={red} witness_scripts={wit} valid={} era={:?}", tx.is_valid(), tx.era());
    }
}

#[test]
fn probe_fixture_triggers() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| (n.starts_with("alonzo") || n.starts_with("babbage") || n.starts_with("conway"))
            && (n.ends_with(".block") || n.ends_with(".tx")))
        .collect();
    names.sort();
    let mut grand = [0usize; 4];
    for n in names {
        let text = std::fs::read_to_string(format!("{dir}/{n}")).unwrap();
        let Ok(bytes) = hex::decode(text.trim()) else { eprintln!("FILE {n} not hex"); continue };
        let mut tot = [0usize; 4];
        if n.ends_with(".block") {
            match MultiEraBlock::decode(&bytes) {
                Ok(b) => for (i, tx) in b.txs().iter().enumerate() { row(&n, i, tx, &mut tot) },
                Err(e) => { eprintln!("FILE {n} undecodable {e:?}"); continue }
            }
        } else {
            match MultiEraTx::decode(&bytes) {
                Ok(tx) => row(&n, 0, &tx, &mut tot),
                Err(e) => { eprintln!("FILE {n} undecodable {e:?}"); continue }
            }
        }
        eprintln!("FILE {n} txs={} redeemers={} witness_scripts={} differ={}", tot[0], tot[1], tot[2], tot[3]);
        for k in 0..4 { grand[k] += tot[k]; }
    }
    eprintln!("CENSUS txs={} redeemers={} witness_scripts={} differ={}", grand[0], grand[1], grand[2], grand[3]);
}
''')

PRES = "presence_of_plutus_scripts(mtx)"
RED = "!option_vec_is_empty(&mtx.transaction_witness_set.redeemer)"
if arm == "head":
    pass
elif arm == "old-fee-trigger":
    for f in ("alonzo.rs", "babbage.rs"):
        q = SRC + f
        t = rd(q)
        assert t.count("if " + RED + " {\n        check_collaterals") == 1, f
        wr(q, t.replace("if " + RED + " {\n        check_collaterals", "if " + PRES + " {\n        check_collaterals"))
elif arm == "redeemer-exunits-trigger":
    for f in ("babbage.rs", "conway.rs"):
        q = SRC + f
        t = rd(q)
        a = t.index("fn check_tx_ex_units(")
        b = t.index("if " + PRES + " {", a)
        assert b - a < 200, (f, b - a)
        t = t[:b] + "if tx_wits.redeemer.is_some() {" + t[b + len("if " + PRES + " {"):]
        wr(q, t)
elif arm == "exunits-full-fix":
    for f in ("alonzo.rs", "babbage.rs", "conway.rs"):
        q = SRC + f
        t = rd(q)
        a = t.index("fn check_tx_ex_units(")
        b = t.index("if " + PRES + " {", a)
        assert b - a < 200, (f, b - a)
        t = t[:b] + "if tx_wits.redeemer.is_some() {" + t[b + len("if " + PRES + " {"):]
        if f == "conway.rs":
            assert t.count("let _ = r.iter().map(|x| {") == 2
            t = t.replace("let _ = r.iter().map(|x| {", "r.iter().for_each(|x| {")
        wr(q, t)
else:
    raise SystemExit("unknown arm " + arm)
print("probe arm", arm)
