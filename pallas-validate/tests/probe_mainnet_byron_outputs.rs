use pallas_addresses::Address;
use pallas_traverse::MultiEraBlock;

// Prints, for every transaction with a bootstrap witness, the kind of each input
// address it would need and each output address it creates.
#[test]
fn mainnet_bootstrap_spends_and_their_outputs() {
    let mut lines = vec![];
    for name in ["allegra1.block", "mary1.block"] {
        let path = format!("{}/../test_data/{name}", env!("CARGO_MANIFEST_DIR"));
        let raw = hex::decode(std::fs::read_to_string(path).unwrap().trim()).unwrap();
        let block = MultiEraBlock::decode(&raw).unwrap();
        for (index, tx) in block.txs().iter().enumerate() {
            let Some(alonzo) = tx.as_alonzo() else { continue };
            let bootstrap = alonzo.transaction_witness_set.bootstrap_witness.as_ref();
            let Some(bootstrap) = bootstrap else { continue };
            let vkeys = alonzo
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map_or(0, |wits| wits.len());
            let outputs: Vec<&str> = tx
                .outputs()
                .iter()
                .map(|output| match output.address() {
                    Ok(Address::Byron(_)) => "byron",
                    Ok(Address::Shelley(_)) => "shelley",
                    Ok(Address::Stake(_)) => "stake",
                    Err(_) => "undecodable",
                })
                .collect();
            lines.push(format!(
                "{name} tx {index}: {} bootstrap, {vkeys} vkey, outputs {outputs:?}",
                bootstrap.len()
            ));
        }
    }
    println!("{}", lines.join("\n"));
}
