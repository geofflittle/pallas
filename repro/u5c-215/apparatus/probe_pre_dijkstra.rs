// Map every pre Dijkstra block fixture under both schemas and write the JSON,
// so two trees can be compared file by file. Output dir comes from PROBE_OUT.
// Each skipped file is printed by name with the step that refused it.
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};

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

#[test]
fn probe_pre_dijkstra_blocks() {
    std::thread::Builder::new().stack_size(512 << 20).spawn(probe).unwrap().join().unwrap();
}

fn probe() {
    let out = std::env::var("PROBE_OUT").expect("PROBE_OUT");
    std::fs::create_dir_all(&out).unwrap();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            (n.ends_with(".block") && !n.starts_with("dijkstra")).then_some(n)
        })
        .collect();
    names.sort();
    let (mut ok, mut skipped) = (0, 0);
    for n in &names {
        let hex_str = match std::fs::read_to_string(format!("{dir}/{n}")) {
            Ok(s) => s,
            Err(e) => { println!("PROBE SKIPPED {n} read {e}"); skipped += 1; continue }
        };
        let cbor = match hex::decode(hex_str.trim()) {
            Ok(c) => c,
            Err(e) => { println!("PROBE SKIPPED {n} hex {e}"); skipped += 1; continue }
        };
        let block = match pallas_traverse::MultiEraBlock::decode(&cbor) {
            Ok(b) => b,
            Err(e) => { println!("PROBE SKIPPED {n} decode {e:?}"); skipped += 1; continue }
        };
        let a = serde_json::json!(pallas_utxorpc::v1alpha::Mapper::new(NoLedger).map_block(&block));
        let b = serde_json::json!(pallas_utxorpc::v1beta::Mapper::new(NoLedger).map_block(&block));
        std::fs::write(format!("{out}/{n}.v1alpha.json"), serde_json::to_string_pretty(&a).unwrap()).unwrap();
        std::fs::write(format!("{out}/{n}.v1beta.json"), serde_json::to_string_pretty(&b).unwrap()).unwrap();
        ok += 1;
    }
    println!("PROBE mapped={ok} skipped={skipped} of {}", names.len());
}
