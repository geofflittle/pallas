#![cfg(feature = "unstable")]

use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _: &[TxoRef]) -> Option<UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _: u64) -> Option<u64> {
        None
    }
}

#[test]
fn a_dijkstra_block_maps_to_its_v1_snapshot() {
    let cbor = hex::decode(include_str!("../../test_data/dijkstra6.block")).unwrap();
    let block = pallas_traverse::MultiEraBlock::decode(&cbor).unwrap();
    let current = serde_json::json!(pallas_utxorpc::v1::Mapper::new(NoLedger).map_block(&block));
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../test_data/u5c_v1_dijkstra.json")).unwrap();

    assert_eq!(expected, current);
}
