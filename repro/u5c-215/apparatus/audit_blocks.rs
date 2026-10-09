#![cfg(feature = "unstable")]
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
fn audit_every_dijkstra_block() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            (n.starts_with("dijkstra") && n.ends_with(".block")).then_some(n)
        })
        .collect();
    names.sort();
    std::panic::set_hook(Box::new(|info| println!("AUDIT PANIC {info}")));
    let mut panics = 0;
    for n in &names {
        let hex_str = std::fs::read_to_string(format!("{dir}/{n}")).unwrap();
        let cbor = hex::decode(hex_str.trim()).unwrap();
        let block = match pallas_traverse::MultiEraBlock::decode(&cbor) {
            Ok(b) => b,
            Err(e) => {
                println!("AUDIT {n} DECODE_ERR {e:?}");
                continue;
            }
        };
        let certs: usize = block.txs().iter().map(|t| t.certs().len()).sum();
        let a = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pallas_utxorpc::v1alpha::Mapper::new(NoLedger).map_block(&block);
        }))
        .is_ok();
        let b = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pallas_utxorpc::v1beta::Mapper::new(NoLedger).map_block(&block);
        }))
        .is_ok();
        panics += (!a) as usize + (!b) as usize;
        println!(
            "AUDIT {n} txs={} certs={certs} v1alpha_ok={a} v1beta_ok={b}",
            block.txs().len()
        );
    }
    println!("AUDIT TOTAL blocks={} panicking_mappings={panics}", names.len());
}
