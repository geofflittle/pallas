//! Maps a Dijkstra block and a Conway block through the v1 schema, as a consumer of the crate does.

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

fn header(block_hex: &str) -> (bool, Option<u32>) {
    let cbor = hex::decode(block_hex.trim()).unwrap();
    let block = pallas_traverse::MultiEraBlock::decode(&cbor).unwrap();
    let mapped = pallas_utxorpc::v1::Mapper::new(NoLedger).map_block(&block);
    let header = mapped.header.unwrap();
    (
        header.body_contains_leios_certificate,
        header
            .endorser_block_announcement
            .map(|x| x.references_size),
    )
}

#[test]
fn a_conway_block_maps_no_leios_field() {
    assert_eq!(
        header(include_str!("../../test_data/conway1.block")),
        (false, None)
    );
}

#[test]
fn a_dijkstra_block_maps_its_leios_certificate_flag_and_announcement() {
    assert_eq!(
        header(include_str!("../../test_data/dijkstra8.block")),
        (true, Some(39_495))
    );
}
