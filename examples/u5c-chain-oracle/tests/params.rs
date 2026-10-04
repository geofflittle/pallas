use pallas_primitives::RationalNumber;
use pallas_utxorpc::v1beta::spec::cardano as u5c;
use pallas_validate::utils::MultiEraProtocolParameters;
use serde_json::{Value, json};
use u5c_chain_oracle::{fields, mapper};

fn node() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/protocol-parameters.json"
    );
    serde_json::from_slice(&std::fs::read(path).expect("the parameter fixture")).expect("json")
}

fn genesis() -> Value {
    json!({"systemStart": "2026-09-07T00:00:00Z", "epochLength": 21600, "slotLength": 1})
}

fn ratio(numerator: u64, denominator: u64) -> RationalNumber {
    RationalNumber {
        numerator,
        denominator,
    }
}

#[test]
fn rational_reads_the_decimal_the_node_prints() {
    assert_eq!(fields::rational(&json!(0.0577)), ratio(577, 10000));
    assert_eq!(fields::rational(&json!(7.21e-05)), ratio(721, 10_000_000));
    assert_eq!(fields::rational(&json!(0.5)), ratio(1, 2));
    assert_eq!(fields::rational(&json!(3)), ratio(3, 1));
    assert_eq!(fields::rational(&json!(0)), ratio(0, 1));
}

/// Maps the node's parameters as Conway parameters and sets the two fields the Conway arm leaves unset.
fn conway_with_every_field() -> u5c::PParams {
    let pp = node();
    let conway = fields::conway_params(&pp, &genesis());
    #[allow(deprecated)]
    let mut mapped = mapper().map_pparams(MultiEraProtocolParameters::Conway(conway));
    mapped.pool_retirement_epoch_bound = pp["poolRetireMaxEpoch"].as_u64().expect("an epoch");
    let v4 = pp["costModels"]["PlutusV4"]
        .as_array()
        .expect("a PlutusV4 model");
    mapped.cost_models.get_or_insert_default().plutus_v4 = Some(u5c::CostModel {
        values: v4.iter().map(|c| c.as_i64().expect("an entry")).collect(),
    });
    mapped
}

#[test]
fn parameters_that_hold_every_node_value_agree() {
    let mapped = conway_with_every_field();
    assert_eq!(fields::param_pairs(&mapped, &node()).len(), 34);
    assert_eq!(fields::param_mismatches(&mapped, &node()), vec![]);
}

#[test]
fn a_parameter_that_differs_from_the_node_is_named() {
    let mut mapped = conway_with_every_field();
    mapped.max_tx_size += 1;
    mapped.drep_voting_thresholds = None;
    let named: Vec<&str> = fields::param_mismatches(&mapped, &node())
        .into_iter()
        .map(|m| m.0)
        .collect();
    assert_eq!(named, vec!["max_tx_size", "drep_voting_thresholds"]);
}

#[test]
fn node_keys_without_a_field_leave_out_every_compared_key() {
    let pp = node();
    let without = fields::node_keys_without_field(&pp);
    let keys = pp.as_object().expect("an object").len();
    assert_eq!(without.len() + fields::NODE_KEYS_WITH_FIELD.len(), keys);
    assert!(
        without
            .iter()
            .all(|k| !fields::NODE_KEYS_WITH_FIELD.contains(&k.as_str()))
    );
}
