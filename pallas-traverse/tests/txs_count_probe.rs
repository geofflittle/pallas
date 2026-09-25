#![cfg(feature = "unstable")]
use pallas_codec::minicbor;
use pallas_primitives::dijkstra;
use pallas_traverse::MultiEraBlock;

fn run(name: &str, hex_str: &str) {
    let cbor = hex::decode(hex_str).unwrap();
    let (_, typed): (u16, dijkstra::Block) = minicbor::decode(&cbor).unwrap();
    let listed = typed.block_body.transactions.len();

    let block = MultiEraBlock::decode(&cbor).unwrap();
    let txs = block.txs();
    let subs: usize = txs.iter().map(|tx| tx.sub_transactions().len()).sum();
    let flat_subs = txs.iter().filter(|tx| tx.as_dijkstra_sub().is_some()).count();
    println!(
        "COUNT {name} listed {listed} txs {} tx_count {} subs_via_parent {subs} subs_in_txs {flat_subs} equal {}",
        txs.len(),
        block.tx_count(),
        txs.len() == listed && block.tx_count() == listed
    );
}

#[test]
fn txs_count_probe() {
    run("dijkstra17", include_str!("../../test_data/dijkstra17.block"));
    run("dijkstra18", include_str!("../../test_data/dijkstra18.block"));
}
