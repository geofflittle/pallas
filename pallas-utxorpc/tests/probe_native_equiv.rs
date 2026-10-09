// Map every native script found in the repository fixtures, under both
// schemas, and write each mapping's protobuf encoding as hex, one per line,
// so two trees can be compared byte for byte. Output dir comes from PROBE_OUT.
// Sources: each tx's witness scripts, output script refs and collateral
// return (sub transactions too under v1beta), and the public
// map_native_script over each Alonzo compatible witness script.
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use prost::Message;
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }
    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

type Found = Vec<(String, Vec<u8>)>;
type Kinds = BTreeMap<String, usize>;

#[test]
fn probe_native_scripts() {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(probe)
        .unwrap()
        .join()
        .unwrap();
}

macro_rules! collect {
    ($name:ident, $($u5c:ident)::+, $subs:ident) => {
        fn $name(tx: &$($u5c)::+::Tx, at: &str, out: &mut Found, kinds: &mut Kinds) {
            use $($u5c)::+ as u5c;
            let mut push = |at: String, s: &u5c::Script| {
                if let Some(u5c::script::Script::Native(n)) = &s.script {
                    let mut stack = vec![n];
                    while let Some(n) = stack.pop() {
                        let kind = match &n.native_script {
                            Some(u5c::native_script::NativeScript::ScriptAll(l)) => {
                                stack.extend(l.items.iter());
                                "all"
                            }
                            Some(u5c::native_script::NativeScript::ScriptAny(l)) => {
                                stack.extend(l.items.iter());
                                "any"
                            }
                            Some(u5c::native_script::NativeScript::ScriptNOfK(k)) => {
                                stack.extend(k.scripts.iter());
                                "nofk"
                            }
                            Some(u5c::native_script::NativeScript::InvalidBefore(_)) => "before",
                            Some(u5c::native_script::NativeScript::InvalidHereafter(_)) => {
                                "hereafter"
                            }
                            Some(_) => "pubkey or guard",
                            None => "none",
                        };
                        *kinds
                            .entry(format!("{} {kind}", stringify!($name)))
                            .or_default() += 1;
                    }
                    out.push((at, n.encode_to_vec()));
                }
            };
            if let Some(w) = &tx.witnesses {
                for (i, s) in w.script.iter().enumerate() {
                    push(format!("{at} witness {i}"), s);
                }
            }
            for (i, o) in tx.outputs.iter().enumerate() {
                if let Some(s) = &o.script {
                    push(format!("{at} output {i}"), s);
                }
            }
            if let Some(s) = tx
                .collateral
                .as_ref()
                .and_then(|c| c.collateral_return.as_ref())
                .and_then(|o| o.script.as_ref())
            {
                push(format!("{at} collateral return"), s);
            }
            $subs(tx, at, out, kinds);
        }
    };
}

fn no_subs<T>(_: &T, _: &str, _: &mut Found, _: &mut Kinds) {}

fn v1beta_subs(
    tx: &pallas_utxorpc::v1beta::spec::cardano::Tx,
    at: &str,
    out: &mut Found,
    kinds: &mut Kinds,
) {
    for (i, sub) in tx.sub_transactions.iter().enumerate() {
        v1beta(sub, &format!("{at} sub {i}"), out, kinds);
    }
}

collect!(v1alpha, pallas_utxorpc::v1alpha::spec::cardano, no_subs);
collect!(v1beta, pallas_utxorpc::v1beta::spec::cardano, v1beta_subs);

fn decode_tx(cbor: &[u8]) -> Option<pallas_traverse::MultiEraTx<'_>> {
    use pallas_traverse::Era;
    if let Ok(tx) = pallas_traverse::MultiEraTx::decode(cbor) {
        return Some(tx);
    }
    #[allow(unused_mut)]
    let mut eras = vec![
        Era::Byron,
        Era::Shelley,
        Era::Allegra,
        Era::Mary,
        Era::Alonzo,
        Era::Babbage,
        Era::Conway,
    ];
    #[cfg(feature = "unstable")]
    eras.push(Era::Dijkstra);
    eras.into_iter()
        .find_map(|e| pallas_traverse::MultiEraTx::decode_for_era(e, cbor).ok())
}

fn map_tx(at: &str, tx: &pallas_traverse::MultiEraTx, found: &mut Found, kinds: &mut Kinds) {
    v1alpha(
        &pallas_utxorpc::v1alpha::Mapper::new(NoLedger).map_tx(tx),
        &format!("{at} v1alpha"),
        found,
        kinds,
    );
    v1beta(
        &pallas_utxorpc::v1beta::Mapper::new(NoLedger).map_tx(tx),
        &format!("{at} v1beta"),
        found,
        kinds,
    );
    for (i, s) in tx.multi_era_native_scripts().iter().enumerate() {
        if let Some(x) = s.as_alonzo_compatible() {
            found.push((
                format!("{at} v1alpha map_native_script {i}"),
                pallas_utxorpc::v1alpha::Mapper::<NoLedger>::map_native_script(x).encode_to_vec(),
            ));
            found.push((
                format!("{at} v1beta map_native_script {i}"),
                pallas_utxorpc::v1beta::Mapper::<NoLedger>::map_native_script(x).encode_to_vec(),
            ));
        }
    }
}

fn probe() {
    let out_dir = std::env::var("PROBE_OUT").expect("PROBE_OUT");
    std::fs::create_dir_all(&out_dir).unwrap();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            (n.ends_with(".block") || n.ends_with(".tx")).then_some(n)
        })
        .collect();
    names.sort();
    let mut found = Found::new();
    let mut kinds = Kinds::new();
    let (mut files, mut skipped, mut txs) = (0, 0, 0);
    for n in &names {
        let Some(cbor) = std::fs::read_to_string(format!("{dir}/{n}"))
            .ok()
            .and_then(|s| hex::decode(s.trim()).ok())
        else {
            println!("PROBE SKIPPED {n} read or hex");
            skipped += 1;
            continue;
        };
        if n.ends_with(".block") {
            let Ok(block) = pallas_traverse::MultiEraBlock::decode(&cbor) else {
                println!("PROBE SKIPPED {n} block decode");
                skipped += 1;
                continue;
            };
            for (t, tx) in block.txs().iter().enumerate() {
                map_tx(&format!("{n} tx {t}"), tx, &mut found, &mut kinds);
                txs += 1;
            }
        } else {
            let Some(tx) = decode_tx(&cbor) else {
                println!("PROBE SKIPPED {n} tx decode");
                skipped += 1;
                continue;
            };
            map_tx(&format!("{n} tx 0"), &tx, &mut found, &mut kinds);
            txs += 1;
        }
        files += 1;
    }
    let mut lines = String::new();
    for (at, bytes) in &found {
        writeln!(lines, "{at} {}", hex::encode(bytes)).unwrap();
    }
    std::fs::write(format!("{out_dir}/native.txt"), &lines).unwrap();
    for (k, v) in &kinds {
        println!("PROBE KIND {k} {v}");
    }
    println!(
        "PROBE native files={files} skipped={skipped} of {} txs={txs} scripts={}",
        names.len(),
        found.len()
    );
}
