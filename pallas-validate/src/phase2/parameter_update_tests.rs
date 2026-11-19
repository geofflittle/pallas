use super::{data::Data, to_plutus_data::ToPlutusData};
use pallas_codec::minicbor;
use pallas_primitives::conway::ProtocolParamUpdate;

fn list(xs: &[i64]) -> pallas_primitives::conway::PlutusData {
    Data::list(xs.iter().map(ToPlutusData::to_plutus_data).collect())
}

#[test]
fn conway_update_encodes_its_cost_models() {
    // {0: 44, 18: {0: [1, 2], 2: [3], 7: [4]}}
    let raw = hex::decode("a200182c12a300820102028103078104").unwrap();
    let update: ProtocolParamUpdate = minicbor::decode(&raw).unwrap();
    let models = Data::map(vec![
        (0u64.to_plutus_data(), list(&[1, 2])),
        (2u64.to_plutus_data(), list(&[3])),
        (7u64.to_plutus_data(), list(&[4])),
    ]);
    let expected = Data::map(vec![
        (0u64.to_plutus_data(), 44u64.to_plutus_data()),
        (18u64.to_plutus_data(), models),
    ]);
    assert_eq!(update.to_plutus_data(), expected);
}
