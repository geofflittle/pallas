//! Builds the node query `CostModels` by struct literal with the fields main has.

use pallas_network::miniprotocols::localstate::queries_v16::CostModels;

fn main() {
    let models = CostModels {
        plutus_v1: None,
        plutus_v2: None,
        plutus_v3: Some(vec![1]),
        unknown: Vec::new().into(),
    };

    println!("{models:?}");
}
