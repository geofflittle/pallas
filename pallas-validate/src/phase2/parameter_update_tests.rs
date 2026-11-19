use super::{data::Data, to_plutus_data::ToPlutusData};
use pallas_codec::minicbor;
use pallas_primitives::RationalNumber;
use pallas_primitives::conway::{GovAction, ProtocolParamUpdate};
use std::collections::BTreeMap;

fn list(xs: &[i64]) -> pallas_primitives::conway::PlutusData {
    Data::list(xs.iter().map(ToPlutusData::to_plutus_data).collect())
}

fn expansion_rate(numerator: u8, denominator: u8) -> ProtocolParamUpdate {
    // {10: 30([numerator, denominator])}
    minicbor::decode(&[0xa1, 0x0a, 0xd8, 0x1e, 0x82, numerator, denominator]).unwrap()
}

fn expansion_rate_data(numerator: i64, denominator: i64) -> pallas_primitives::conway::PlutusData {
    Data::map(vec![(
        10u64.to_plutus_data(),
        list(&[numerator, denominator]),
    )])
}

#[test]
fn a_rational_in_lowest_terms_is_written_as_read() {
    assert_eq!(
        expansion_rate(1, 3).to_plutus_data(),
        expansion_rate_data(1, 3)
    );
}

#[test]
fn a_rational_with_a_zero_denominator_is_written_as_read() {
    for (numerator, denominator) in [(2, 0), (0, 0)] {
        assert_eq!(
            expansion_rate(numerator, denominator).to_plutus_data(),
            expansion_rate_data(numerator.into(), denominator.into())
        );
    }
}

#[test]
fn a_conway_update_writes_its_rational_in_lowest_terms() {
    assert_eq!(
        expansion_rate(2, 4).to_plutus_data(),
        expansion_rate_data(1, 2)
    );
}

#[test]
fn a_conway_committee_update_writes_its_quorum_in_lowest_terms() {
    let update = |numerator, denominator| {
        GovAction::UpdateCommittee(
            None,
            vec![].into(),
            BTreeMap::new(),
            RationalNumber {
                numerator,
                denominator,
            },
        )
        .to_plutus_data()
    };
    assert_eq!(update(2, 4), update(1, 2));
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
