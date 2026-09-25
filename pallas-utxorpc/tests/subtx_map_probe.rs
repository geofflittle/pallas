//! Probe mapping the two sub transaction fixtures through both utxorpc mappers.
#![cfg(feature = "unstable")]

#[derive(Clone)]
struct NoLedger;

impl pallas_utxorpc::LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[pallas_utxorpc::TxoRef]) -> Option<pallas_utxorpc::UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

#[test]
fn map_sub_transaction_fixtures() {
    for (name, text) in [
        ("dijkstra17", include_str!("../../test_data/dijkstra17.block")),
        ("dijkstra18", include_str!("../../test_data/dijkstra18.block")),
    ] {
        let cbor = hex::decode(text.trim()).unwrap();
        let alpha = pallas_utxorpc::v1alpha::Mapper::new(NoLedger).map_block_cbor(&cbor);
        let beta = pallas_utxorpc::v1beta::Mapper::new(NoLedger).map_block_cbor(&cbor);
        let alpha_txs = alpha.body.map(|b| b.tx.len());
        let beta_txs = beta.body.map(|b| b.tx.len());
        println!("PROBE {name} v1alpha txs {alpha_txs:?} v1beta txs {beta_txs:?}");
    }
}
