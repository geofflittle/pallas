//! Probe printing the header fields of the two sub transaction fixtures.
#![cfg(feature = "unstable")]
use pallas_traverse::MultiEraBlock;

#[test]
fn print_sub_transaction_fixture_headers() {
    for (name, text) in [
        ("dijkstra17", include_str!("../../test_data/dijkstra17.block")),
        ("dijkstra18", include_str!("../../test_data/dijkstra18.block")),
    ] {
        let cbor = hex::decode(text.trim()).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        let top = block.as_dijkstra().unwrap().block_body.transactions.len();
        println!(
            "PROBE {name} slot {} number {} hash {} bytes {} top {} txs {} count {}",
            block.slot(),
            block.number(),
            block.hash(),
            cbor.len(),
            top,
            block.txs().len(),
            block.tx_count()
        );
    }
}
