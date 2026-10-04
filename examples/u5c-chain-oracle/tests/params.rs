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

fn u5c_ratio(x: &Value) -> Option<u5c::RationalNumber> {
    let r = fields::rational(x);
    Some(u5c::RationalNumber {
        numerator: r.numerator as i32,
        denominator: r.denominator as u32,
    })
}

fn uint(pp: &Value, key: &str) -> u64 {
    pp[key].as_u64().unwrap_or_else(|| panic!("an integer at {key}"))
}

/// Maps the node's parameters as Conway parameters and sets the fields the Conway arm leaves unset, except the nil pledge leverage.
fn conway_with_every_field() -> u5c::PParams {
    let pp = node();
    let conway = fields::conway_params(&pp, &genesis());
    #[allow(deprecated)]
    let mut mapped = mapper().map_pparams(MultiEraProtocolParameters::Conway(conway));
    mapped.pool_retirement_epoch_bound = uint(&pp, "poolRetireMaxEpoch");
    let v4 = pp["costModels"]["PlutusV4"]
        .as_array()
        .expect("a PlutusV4 model");
    mapped.cost_models.get_or_insert_default().plutus_v4 = Some(u5c::CostModel {
        values: v4.iter().map(|c| c.as_i64().expect("an entry")).collect(),
    });
    let units = &pp["maxEndorserBlockExecutionUnits"];
    u5c::PParams {
        max_ref_script_size_per_block: uint(&pp, "maxRefScriptSizePerBlock"),
        max_ref_script_size_per_tx: uint(&pp, "maxRefScriptSizePerTx"),
        ref_script_cost_stride: uint(&pp, "refScriptCostStride"),
        ref_script_cost_multiplier: u5c_ratio(&pp["refScriptCostMultiplier"]),
        min_pool_margin: u5c_ratio(&pp["minPoolMargin"]),
        leios_announcement_period_length: uint(&pp, "leiosAnnouncementPeriodLength"),
        leios_vote_period_length: uint(&pp, "leiosVotePeriodLength"),
        leios_diffusion_period_length: uint(&pp, "leiosDiffusionPeriodLength"),
        leios_committee_size: uint(&pp, "leiosCommitteeSize") as u32,
        leios_quorum_stake_threshold: u5c_ratio(&pp["leiosQuorumStakeThreshold"]),
        max_endorser_block_references_size: uint(&pp, "maxEndorserBlockReferencesSize"),
        max_endorser_block_txs_size: uint(&pp, "maxEndorserBlockTxsSize"),
        max_endorser_block_execution_units: Some(u5c::ExUnits {
            memory: uint(units, "memory"),
            steps: uint(units, "steps"),
        }),
        max_ref_script_size_per_endorser_block: uint(&pp, "maxRefScriptSizePerEndorserBlock"),
        ..mapped
    }
}

fn names(x: Vec<(&'static str, Value, Value)>) -> Vec<&'static str> {
    x.into_iter().map(|m| m.0).collect()
}

#[test]
fn parameters_that_hold_every_node_value_agree_but_the_nil_pledge_leverage() {
    let mapped = conway_with_every_field();
    assert_eq!(fields::param_pairs(&mapped, &node()).len(), 49);
    assert_eq!(
        names(fields::param_mismatches(&mapped, &node())),
        vec!["max_pledge_leverage"]
    );
}

#[test]
fn the_dijkstra_mapping_agrees_with_the_node_on_exactly_the_fields_its_type_holds() {
    let pp = node();
    #[allow(deprecated)]
    let mapped = mapper().map_pparams(MultiEraProtocolParameters::Dijkstra(
        fields::dijkstra_params(&pp, &genesis()),
    ));
    let pairs = names(fields::param_pairs(&mapped, &pp));
    let mismatches = names(fields::param_mismatches(&mapped, &pp));
    let agreeing: Vec<&str> = pairs
        .iter()
        .copied()
        .filter(|f| !mismatches.contains(f))
        .collect();
    let mut held = fields::DIJKSTRA_TYPE_FIELDS.to_vec();
    held.sort();
    let mut agreeing_sorted = agreeing.clone();
    agreeing_sorted.sort();
    assert_eq!(agreeing_sorted, held);
    assert_eq!(mismatches.len() + held.len(), pairs.len());
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
