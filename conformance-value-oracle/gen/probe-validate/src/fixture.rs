//! The captured Dijkstra transaction and its state, rebuilt so that a key this
//! program holds signs it.

use std::path::Path;

use pallas_codec::minicbor;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_crypto::key::ed25519::SecretKey;
use pallas_primitives::{StakeCredential, conway as c, dijkstra as n};
use pallas_traverse::{Era, MultiEraInput, MultiEraOutput, MultiEraTx};
use pallas_validate::phase1;
use pallas_validate::utils::dijkstra_state::DijkstraAccount;
use pallas_validate::utils::{
    CertState, DijkstraProtParams, DijkstraTimeContext, Environment, EraTime, ForecastHorizon,
    MultiEraProtocolParameters, PostAlonzoError, ValidationError,
};
use serde_json::Value;

use crate::outcome::Validator;
use crate::term::{self, Term};

/// Lovelace of the input this program adds to pay deposits.
pub const FUNDS: i128 = 10_000_000_000_000;
/// The fee every planted transaction pays, above the minimum of any of them.
pub const FEE: i128 = 400_000;
/// The transaction id of the input that pays deposits.
pub const FUNDING_TX: [u8; 32] = [0xff; 32];
/// The transaction id of the input planted transactions read as a reference
/// input.
pub const REFERENCE_TX: [u8; 32] = [0xee; 32];

/// The captured transaction, its spent outputs, parameters and time.
pub struct Fixture {
    pub transaction: Vec<u8>,
    inputs: Vec<(n::TransactionInput, Vec<u8>)>,
    env: Environment,
    time: DijkstraTimeContext,
    pub key: SecretKey,
}

fn number(v: &Value) -> u64 {
    match v {
        Value::String(s) => s.parse().unwrap(),
        Value::Object(_) => number(&v["int"]),
        _ => v.as_u64().unwrap(),
    }
}

fn ratio(v: &Value) -> n::RationalNumber {
    n::RationalNumber {
        numerator: number(&v["numerator"]),
        denominator: number(&v["denominator"]),
    }
}

fn units(v: &Value) -> n::ExUnits {
    n::ExUnits {
        mem: number(&v["memory"]),
        steps: number(&v["steps"]),
    }
}

impl Fixture {
    /// Loads `confirmed-ttl` from the `dijkstra-ttl` test data directory
    /// `root`.
    pub fn load(root: &Path) -> Self {
        let case = root.join("confirmed-ttl");
        let json = |path: &Path| -> Value {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        };
        let parameters = json(&case.join("parameters.json"));
        let p = &parameters["values"]["cardano"];
        let g = json(&case.join("genesis.json"));
        let replay = json(&case.join("replay.json"));
        let supplemental = json(&root.join("genesis/dijkstra.json"));
        let whole_ratio = |name: &str| n::RationalNumber {
            numerator: number(&supplemental[name]),
            denominator: 1,
        };
        let pv = &p["poolVotingThresholds"]["thresholds"];
        let dv = &p["drepVotingThresholds"]["thresholds"];
        let cost = |name: &str| {
            p["costModels"][name]["values"].as_array().map(|xs| {
                xs.iter()
                    .map(|x| {
                        x.as_str()
                            .map(|x| x.parse().unwrap())
                            .unwrap_or_else(|| x.as_i64().unwrap())
                    })
                    .collect()
            })
        };
        let pp = DijkstraProtParams {
            system_start: chrono::DateTime::from_timestamp(number(&g["system_start"]) as i64, 0)
                .unwrap()
                .fixed_offset(),
            epoch_length: number(&g["epoch_length"]),
            slot_length: number(&g["slot_length"]),
            minfee_a: number(&p["minFeeCoefficient"]) as u32,
            minfee_b: number(&p["minFeeConstant"]) as u32,
            max_block_body_size: number(&p["maxBlockBodySize"]) as u32,
            max_transaction_size: number(&p["maxTxSize"]) as u32,
            max_block_header_size: number(&p["maxBlockHeaderSize"]) as u32,
            key_deposit: number(&p["stakeKeyDeposit"]),
            pool_deposit: number(&p["poolDeposit"]),
            desired_number_of_stake_pools: number(&p["desiredNumberOfPools"]) as u32,
            protocol_version: (
                number(&p["protocolVersion"]["major"]),
                p["protocolVersion"]["minor"].as_u64().unwrap_or(0),
            ),
            min_pool_cost: number(&p["minPoolCost"]),
            ada_per_utxo_byte: number(&p["coinsPerUtxoByte"]),
            cost_models_for_script_languages: n::CostModels {
                plutus_v1: cost("plutusV1"),
                plutus_v2: cost("plutusV2"),
                plutus_v3: cost("plutusV3"),
                plutus_v4: cost("plutusV4"),
                unknown: Default::default(),
            },
            execution_costs: pallas_primitives::ExUnitPrices {
                mem_price: ratio(&p["prices"]["memory"]),
                step_price: ratio(&p["prices"]["steps"]),
            },
            max_tx_ex_units: units(&p["maxExecutionUnitsPerTransaction"]),
            max_block_ex_units: units(&p["maxExecutionUnitsPerBlock"]),
            max_value_size: number(&p["maxValueSize"]) as u32,
            collateral_percentage: number(&p["collateralPercentage"]) as u32,
            max_collateral_inputs: number(&p["maxCollateralInputs"]) as u32,
            expansion_rate: ratio(&p["monetaryExpansion"]),
            treasury_growth_rate: ratio(&p["treasuryExpansion"]),
            maximum_epoch: number(&p["poolRetirementEpochBound"]),
            pool_pledge_influence: ratio(&p["poolInfluence"]),
            pool_voting_thresholds: c::PoolVotingThresholds {
                motion_no_confidence: ratio(&pv[0]),
                committee_normal: ratio(&pv[1]),
                committee_no_confidence: ratio(&pv[2]),
                hard_fork_initiation: ratio(&pv[3]),
                security_voting_threshold: ratio(&pv[4]),
            },
            drep_voting_thresholds: c::DRepVotingThresholds {
                motion_no_confidence: ratio(&dv[0]),
                committee_normal: ratio(&dv[1]),
                committee_no_confidence: ratio(&dv[2]),
                update_constitution: ratio(&dv[3]),
                hard_fork_initiation: ratio(&dv[4]),
                pp_network_group: ratio(&dv[5]),
                pp_economic_group: ratio(&dv[6]),
                pp_technical_group: ratio(&dv[7]),
                pp_governance_group: ratio(&dv[8]),
                treasury_withdrawal: ratio(&dv[9]),
            },
            min_committee_size: number(&p["minCommitteeSize"]),
            committee_term_limit: number(&p["committeeTermLimit"]),
            governance_action_validity_period: number(&p["governanceActionValidityPeriod"]),
            governance_action_deposit: number(&p["governanceActionDeposit"]),
            drep_deposit: number(&p["drepDeposit"]),
            drep_inactivity_period: number(&p["drepInactivityPeriod"]),
            minfee_refscript_cost_per_byte: ratio(&p["minFeeScriptRefCostPerByte"]),
            max_ref_script_size_per_block: number(&p["maxRefScriptSizePerBlock"]) as u32,
            max_ref_script_size_per_tx: number(&p["maxRefScriptSizePerTx"]) as u32,
            ref_script_cost_stride: number(&p["refScriptCostStride"]) as u32,
            ref_script_cost_multiplier: ratio(&p["refScriptCostMultiplier"]),
            max_pledge_leverage: p.get("maxPledgeLeverage").map(ratio),
            min_pool_margin: ratio(&p["minPoolMargin"]),
            leios_announcement_period_length: number(&p["leiosAnnouncementPeriodLength"]) as u32,
            leios_vote_period_length: number(&p["leiosVotePeriodLength"]) as u32,
            leios_diffusion_period_length: number(&p["leiosDiffusionPeriodLength"]) as u32,
            leios_committee_size: number(&p["leiosCommitteeSize"]) as u16,
            leios_quorum_stake_threshold: ratio(&p["leiosQuorumStakeThreshold"]),
            max_endorser_block_references_size: number(&p["maxEndorserBlockReferencesSize"]) as u32,
            max_endorser_block_txs_size: number(&p["maxEndorserBlockTxsSize"]) as u32,
            max_endorser_block_ex_units: units(&p["maxEndorserBlockExecutionUnits"]),
            max_ref_script_size_per_endorser_block: number(&p["maxRefScriptSizePerEndorserBlock"])
                as u32,
            peras_min_candidate_block_age: number(&supplemental["perasMinCandidateBlockAge"])
                as u32,
            peras_healing_factor: whole_ratio("perasHealingFactor"),
            peras_cert_boost: number(&supplemental["perasCertBoost"]) as u16,
            peras_target_committee_size: number(&supplemental["perasTargetCommitteeSize"]) as u16,
            peras_bootstrap_round: supplemental["perasBootstrapRound"]
                .as_u64()
                .map(|x| x as u32),
            peras_quorum_threshold_safety_margin: whole_ratio("perasQuorumThresholdSafetyMargin"),
            ref_inputs_cost_per_multi_asset_policy: number(
                &supplemental["refInputsCostPerMultiAssetPolicy"],
            ),
            ref_inputs_cost_per_datum_byte: number(&supplemental["refInputsCostPerDatumByte"]),
        };
        let epoch = pp.epoch_length;
        let block_slot = number(&replay["validation_slot"]);
        let stability_window = (3.0 * number(&g["security_param"]) as f64
            / g["active_slots_coefficient"].as_f64().unwrap())
        .ceil() as u64;
        let horizon = (block_slot + 1 + stability_window).div_ceil(epoch) * epoch;
        let time = DijkstraTimeContext::new(
            vec![EraTime {
                start_slot: 0,
                start_time_ms: number(&g["system_start"]) * 1000,
                slot_length_ms: number(&g["slot_length"]) * 1000,
            }],
            ForecastHorizon::Slot(horizon),
        )
        .unwrap();
        let transaction = hex::decode(
            std::fs::read_to_string(case.join("transaction.hex"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        let inputs = json(&case.join("inputs.json"))
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                (
                    n::TransactionInput {
                        transaction_id: x["hash"].as_str().unwrap().parse().unwrap(),
                        index: number(&x["index"]),
                    },
                    hex::decode(x["output_cbor_hex"].as_str().unwrap()).unwrap(),
                )
            })
            .collect();
        Self {
            transaction,
            inputs,
            env: Environment {
                prot_params: MultiEraProtocolParameters::Dijkstra(pp),
                prot_magic: 164,
                block_slot,
                network_id: 0,
                acnt: None,
            },
            time,
            key: SecretKey::from([7; 32]),
        }
    }

    pub fn key_hash(&self) -> Vec<u8> {
        Hasher::<224>::hash(self.key.public_key().as_ref()).to_vec()
    }

    /// The enterprise address of this program's key on the fixture's network.
    pub fn address(&self) -> Vec<u8> {
        [vec![0x60], self.key_hash()].concat()
    }

    fn captured_guard(&self) -> Vec<u8> {
        let mut tx = term::read(&self.transaction).unwrap();
        let guards = tx.items_mut()[0].get_mut(14).unwrap().items_mut();
        match &guards[0] {
            Term::Array(x) => match &x[1] {
                Term::Bytes(h) => h.clone(),
                other => panic!("guard is not a key hash: {other:?}"),
            },
            other => panic!("guard is not a credential: {other:?}"),
        }
    }

    /// The captured transaction with this program's key as its guard and
    /// signer, an input that pays deposits, and the fee `FEE`.
    pub fn base(&self) -> Term {
        let mut tx = term::read(&self.transaction).unwrap();
        let body = &mut tx.items_mut()[0];
        let credential = Term::Array(vec![Term::Int(0), Term::Bytes(self.key_hash())]);
        body.set(14, term::set(vec![credential]));
        body.get_mut(0).unwrap().items_mut().push(Term::Array(vec![
            Term::Bytes(FUNDING_TX.to_vec()),
            Term::Int(0),
        ]));
        let Some(Term::Int(fee)) = body.get_mut(2).cloned() else {
            panic!("no fee")
        };
        body.set(2, Term::Int(FEE));
        pay(body, FUNDS - (FEE - fee));
        self.sign(&mut tx);
        tx
    }

    /// Replaces the key witnesses with this program's signature over the body.
    pub fn sign(&self, tx: &mut Term) {
        let body = term::write(&tx.items_mut()[0]);
        let signature = self.key.sign(Hasher::<256>::hash(&body));
        let witness = Term::Array(vec![
            Term::Bytes(self.key.public_key().as_ref().to_vec()),
            Term::Bytes(signature.as_ref().to_vec()),
        ]);
        tx.items_mut()[1].set(0, term::set(vec![witness]));
    }

    /// Signs transaction bytes whose body may be malformed, changing only the
    /// signature bytes of `base`.
    pub fn resign(&self, base: &[u8], bytes: &[u8]) -> Vec<u8> {
        let old = signature_of(base);
        let mut d = minicbor::Decoder::new(bytes);
        let span = d.array().ok().and_then(|_| {
            let start = d.position();
            d.skip().ok().map(|_| start..d.position())
        });
        let (Some(span), Some(old)) = (span, old) else {
            return bytes.to_vec();
        };
        let new = self.key.sign(Hasher::<256>::hash(&bytes[span]));
        let at: Vec<usize> = bytes
            .windows(64)
            .enumerate()
            .filter(|(_, w)| *w == old.as_slice())
            .map(|(i, _)| i)
            .collect();
        let mut out = bytes.to_vec();
        if let [i] = at[..] {
            out[i..i + 64].copy_from_slice(new.as_ref());
        }
        out
    }

    /// The outputs the transactions spend or read, with the captured signer's
    /// outputs paid to this program's key.
    fn outputs(&self) -> Vec<(n::TransactionInput, Vec<u8>)> {
        let old = self.captured_guard();
        let mut out: Vec<_> = self
            .inputs
            .iter()
            .map(|(i, raw)| (i.clone(), replace(raw, &old, &self.key_hash())))
            .collect();
        let own = |id: [u8; 32], coin: i128| {
            (
                n::TransactionInput {
                    transaction_id: Hash::new(id),
                    index: 0,
                },
                term::write(&Term::Array(vec![
                    Term::Bytes(self.address()),
                    Term::Int(coin),
                ])),
            )
        };
        out.push(own(FUNDING_TX, FUNDS));
        out.push(own(REFERENCE_TX, 2_000_000));
        out
    }

    /// Runs Dijkstra phase one on `bytes` against the fixture outputs, with
    /// this program's stake account registered.
    pub fn validate(&self, bytes: &[u8]) -> Validator {
        let tx = match MultiEraTx::decode_for_era(Era::Dijkstra, bytes) {
            Ok(tx) => tx,
            Err(e) => return Validator::RefusesMalformed(format!("decode: {e}")),
        };
        let outputs = self.outputs();
        let utxos = outputs
            .iter()
            .map(|(i, raw)| {
                (
                    MultiEraInput::from_alonzo_compatible(i),
                    MultiEraOutput::decode(Era::Dijkstra, raw).unwrap(),
                )
            })
            .collect();
        let mut state = CertState::default();
        let own = StakeCredential::AddrKeyhash(Hash::from(&self.key_hash()[..]));
        let account = DijkstraAccount {
            balance: 0,
            deposit: 2_000_000,
        };
        state.dijkstra.accounts.insert(own, Some(account));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            phase1::validate_tx_with_time(&tx, 0, &self.env, &utxos, &mut state, &self.time)
        }));
        match result {
            Ok(Ok(())) => Validator::Accepts,
            Ok(Err(e @ ValidationError::DijkstraMalformed(_)))
            | Ok(Err(e @ ValidationError::PostAlonzo(PostAlonzoError::AddressDecoding))) => {
                Validator::RefusesMalformed(format!("{e:?}"))
            }
            Ok(Err(e)) => Validator::RefusesOther(format!("{e:?}")),
            Err(p) => {
                let message = p.downcast_ref::<String>().cloned();
                let message = message.or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
                Validator::RefusesOther(format!("panic {}", message.unwrap_or_default()))
            }
        }
    }
}

/// Adds `coin` to the first output of a body.
pub fn pay(body: &mut Term, coin: i128) {
    let output = &mut body.get_mut(1).unwrap().items_mut()[0];
    match output {
        Term::Array(x) => match &mut x[1] {
            Term::Int(c) => *c += coin,
            other => panic!("output coin is not an integer: {other:?}"),
        },
        Term::Map(_) => match output.get_mut(1) {
            Some(Term::Int(c)) => *c += coin,
            other => panic!("output coin is not an integer: {other:?}"),
        },
        other => panic!("not an output: {other:?}"),
    }
}

fn signature_of(tx: &[u8]) -> Option<Vec<u8>> {
    let mut t = term::read(tx).ok()?;
    let w = t.items_mut().get_mut(1)?.get_mut(0)?.items_mut();
    match w.first()? {
        Term::Array(x) => match x.get(1)? {
            Term::Bytes(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn replace(raw: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = raw.to_vec();
    if let Some(i) = raw.windows(old.len()).position(|w| w == old) {
        out[i..i + old.len()].copy_from_slice(new);
    }
    out
}
