//! Decodes generated Dijkstra values with pallas and prints comparable facts.
//!
//! Reads JSON lines carrying a `hex` field on stdin. For each line it prints
//! one JSON line with the pallas decode facts, whether re-encoding gives the
//! input back, and the facts of the v1 utxorpc mapper output.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use pallas_codec::minicbor;
use pallas_codec::utils::Nullable;
use pallas_primitives::dijkstra;
use pallas_traverse::{Era, MultiEraBlock, MultiEraParamUpdate, MultiEraTx};
use pallas_utxorpc::v1::{Mapper, spec::cardano as u5c};
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use serde_json::{Map, Value, json};

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

type Facts = Map<String, Value>;

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "non string panic".to_string()
    }
}

fn enc<T: minicbor::Encode<()>>(x: &T) -> Vec<u8> {
    minicbor::to_vec(x).expect("encode to a vec")
}

fn distinct<'a, T: minicbor::Encode<()> + 'a>(items: impl IntoIterator<Item = &'a T>) -> usize {
    items.into_iter().map(enc).collect::<BTreeSet<_>>().len()
}

fn count<T>(items: Option<&[T]>) -> usize {
    items.map(|x| x.len()).unwrap_or(0)
}

fn put(f: &mut Facts, k: &str, v: Value) {
    f.insert(k.to_string(), v);
}

/// Puts the distinct count under `k` and the count as written under `raw.k`.
fn put_set<T: minicbor::Encode<()>>(f: &mut Facts, k: &str, items: Option<&[T]>) {
    put(f, k, json!(items.map(|x| distinct(x.iter())).unwrap_or(0)));
    put(f, &format!("raw.{k}"), json!(count(items)));
}

fn output_coin(o: &dijkstra::TransactionOutput) -> u128 {
    use pallas_primitives::{alonzo, babbage::GenTransactionOutput, conway};
    match o {
        GenTransactionOutput::Legacy(x) => match &x.amount {
            alonzo::Value::Coin(c) => *c as u128,
            alonzo::Value::Multiasset(c, _) => *c as u128,
        },
        GenTransactionOutput::PostAlonzo(x) => match &x.value {
            conway::Value::Coin(c) => *c as u128,
            conway::Value::Multiasset(c, _) => *c as u128,
        },
    }
}

fn num(x: u128) -> Value {
    match u64::try_from(x) {
        Ok(v) => json!(v),
        Err(_) => json!(x.to_string()),
    }
}

fn body_facts(b: &dijkstra::TransactionBody) -> Facts {
    let mut f = Facts::new();
    put(&mut f, "fee", json!(b.fee));
    put(&mut f, "ttl", json!(b.ttl));
    put(&mut f, "validity_start", json!(b.validity_interval_start));
    put_set(&mut f, "inputs", Some(&b.inputs[..]));
    put(&mut f, "outputs", json!(b.outputs.len()));
    put(
        &mut f,
        "outputs_coin",
        num(b.outputs.iter().map(output_coin).sum()),
    );
    put_set(&mut f, "certs", b.certificates.as_ref().map(|x| &x[..]));
    put(&mut f, "withdrawals", json!(b.withdrawals.as_ref().map(|x| x.len()).unwrap_or(0)));
    put(&mut f, "mint_policies", json!(b.mint.as_ref().map(|x| x.len()).unwrap_or(0)));
    put(&mut f, "aux_data_hash", json!(b.auxiliary_data_hash.is_some()));
    put(&mut f, "script_data_hash", json!(b.script_data_hash.is_some()));
    put_set(&mut f, "collateral", b.collateral.as_ref().map(|x| &x[..]));
    match &b.guards {
        None => put_set::<dijkstra::AddrKeyhash>(&mut f, "guards", None),
        Some(dijkstra::Guards::AddrKeyhashes(x)) => put_set(&mut f, "guards", Some(&x[..])),
        Some(dijkstra::Guards::Credentials(x)) => put_set(&mut f, "guards", Some(&x[..])),
    }
    put(
        &mut f,
        "network_id",
        match b.network_id {
            None => Value::Null,
            Some(pallas_primitives::NetworkId::Testnet) => json!(0),
            Some(pallas_primitives::NetworkId::Mainnet) => json!(1),
        },
    );
    put(&mut f, "collateral_return", json!(b.collateral_return.is_some()));
    put(&mut f, "total_collateral", json!(b.total_collateral));
    put_set(&mut f, "reference_inputs", b.reference_inputs.as_ref().map(|x| &x[..]));
    put(&mut f, "voters", json!(b.voting_procedures.as_ref().map(|x| x.len()).unwrap_or(0)));
    put_set(&mut f, "proposals", b.proposal_procedures.as_ref().map(|x| &x[..]));
    put(&mut f, "treasury", json!(b.treasury_value));
    put(
        &mut f,
        "donation",
        json!(b.donation.as_ref().map(u64::from).unwrap_or(0)),
    );
    let subs = b.sub_transactions.as_ref();
    put(
        &mut f,
        "sub_transactions",
        json!(
            subs.map(|x| x
                .iter()
                .map(|s| s.sub_transaction_body.raw_cbor().to_vec())
                .collect::<BTreeSet<_>>()
                .len())
                .unwrap_or(0)
        ),
    );
    put(&mut f, "raw.sub_transactions", json!(subs.map(|x| x.len()).unwrap_or(0)));
    put(
        &mut f,
        "required_top_level_guards",
        json!(b.required_top_level_guards.as_ref().map(|x| x.len()).unwrap_or(0)),
    );
    put(&mut f, "direct_deposits", json!(b.direct_deposits.as_ref().map(|x| x.len()).unwrap_or(0)));
    put(
        &mut f,
        "account_balance_intervals",
        json!(b.account_balance_intervals.as_ref().map(|x| x.len()).unwrap_or(0)),
    );
    put(
        &mut f,
        "starting_account_balance_intervals",
        json!(b.starting_account_balance_intervals.as_ref().map(|x| x.len()).unwrap_or(0)),
    );
    f
}

fn distinct_raw<'a, T: 'a>(
    items: impl IntoIterator<Item = &'a pallas_codec::utils::KeepRaw<'a, T>>,
) -> usize {
    items.into_iter().map(|x| x.raw_cbor().to_vec()).collect::<BTreeSet<_>>().len()
}

fn wits_facts(w: &dijkstra::WitnessSet) -> Facts {
    let mut f = Facts::new();
    put_set(&mut f, "vkey", w.vkeywitness.as_ref().map(|x| &x[..]));
    put_set(&mut f, "bootstrap", w.bootstrap_witness.as_ref().map(|x| &x[..]));
    put(&mut f, "native", json!(w.native_script.as_ref().map(|x| distinct_raw(x.iter())).unwrap_or(0)));
    put(&mut f, "raw.native", json!(w.native_script.as_ref().map(|x| x.len()).unwrap_or(0)));
    put_set(&mut f, "plutus_v1", w.plutus_v1_script.as_ref().map(|x| &x[..]));
    put_set(&mut f, "plutus_v2", w.plutus_v2_script.as_ref().map(|x| &x[..]));
    put_set(&mut f, "plutus_v3", w.plutus_v3_script.as_ref().map(|x| &x[..]));
    put_set(&mut f, "plutus_v4", w.plutus_v4_script.as_ref().map(|x| &x[..]));
    put(&mut f, "datums", json!(w.plutus_data.as_ref().map(|x| distinct_raw(x.iter())).unwrap_or(0)));
    put(&mut f, "raw.datums", json!(w.plutus_data.as_ref().map(|x| x.len()).unwrap_or(0)));
    put(&mut f, "redeemers", json!(w.redeemer.as_ref().map(|x| x.0.len()).unwrap_or(0)));
    f
}

fn prefixed(p: &str, src: Facts, into: &mut Facts) {
    for (k, v) in src {
        let key = match k.strip_prefix("raw.") {
            Some(rest) => format!("raw.{p}{rest}"),
            None => format!("{p}{k}"),
        };
        into.insert(key, v);
    }
}

/// Each key of the pallas re-encoding of the update, with the value's CBOR in hex.
fn ppu_facts(u: &dijkstra::ProtocolParamUpdate) -> Facts {
    let bytes = enc(u);
    let mut f = Facts::new();
    let mut d = minicbor::Decoder::new(&bytes);
    let n = d.map().expect("map").expect("definite map");
    for _ in 0..n {
        let k = d.u64().expect("uint key");
        let start = d.position();
        d.skip().expect("value");
        f.insert(format!("k{k}"), json!(hex::encode(&bytes[start..d.position()])));
    }
    f
}

fn block_facts(b: &dijkstra::Block) -> Facts {
    let h = &b.header.header_body;
    let mut f = Facts::new();
    put(
        &mut f,
        "hash",
        json!(hex::encode(pallas_crypto_hash(b.header.raw_cbor()))),
    );
    put(&mut f, "block_no", json!(h.block_number));
    put(&mut f, "slot", json!(h.slot));
    put(&mut f, "prev_genesis", json!(h.prev_hash.is_none()));
    put(&mut f, "body_size", json!(h.block_body_size));
    put(&mut f, "body_hash", json!(hex::encode(h.block_body_hash.as_ref())));
    put(&mut f, "contains_leios_cert", json!(h.block_body_contains_leios_cert));
    put(&mut f, "eb_announcement", json!(matches!(h.eb_announcement, Nullable::Some(_))));
    put(&mut f, "protocol_major", json!(h.protocol_version.0));
    put(&mut f, "txs", json!(b.block_body.transactions.len()));
    put(&mut f, "leios_certificate", json!(matches!(b.block_body.leios_certificate, Nullable::Some(_))));
    put(&mut f, "peras_certificate", json!(matches!(b.block_body.peras_certificate, Nullable::Some(_))));
    f
}

fn pallas_crypto_hash(raw: &[u8]) -> Vec<u8> {
    pallas_crypto::hash::Hasher::<256>::hash(raw).to_vec()
}

fn bigint(x: &Option<u5c::BigInt>) -> Value {
    use u5c::big_int::BigInt as B;
    match x.as_ref().and_then(|b| b.big_int.as_ref()) {
        None => Value::Null,
        Some(B::Int(i)) => json!(i),
        Some(B::BigUInt(bytes)) => {
            num(bytes.iter().fold(0u128, |acc, b| (acc << 8) | *b as u128))
        }
        Some(B::BigNInt(bytes)) => json!(format!("-{}", hex::encode(bytes))),
    }
}

fn bigint_sum(xs: impl Iterator<Item = Value>) -> Value {
    let mut total: u128 = 0;
    for v in xs {
        match v {
            Value::Number(n) => total += n.as_u64().unwrap_or(0) as u128,
            Value::String(s) => total += s.parse::<u128>().unwrap_or(0),
            _ => {}
        }
    }
    num(total)
}

fn common_facts(c: &u5c::TxCommon, f: &mut Facts) {
    put(f, "body.ttl", json!(c.validity.as_ref().and_then(|v| v.ttl)));
    put(f, "body.validity_start", json!(c.validity.as_ref().and_then(|v| v.start)));
    put(f, "body.inputs", json!(c.inputs.len()));
    put(f, "body.outputs", json!(c.outputs.len()));
    put(f, "body.outputs_coin", bigint_sum(c.outputs.iter().map(|o| bigint(&o.coin))));
    put(f, "body.certs", json!(c.certificates.len()));
    put(f, "body.withdrawals", json!(c.withdrawals.len()));
    put(f, "body.mint_policies", json!(c.mint.len()));
    put(f, "body.aux_data_hash", json!(!c.auxiliary_data_hash.is_empty()));
    put(f, "body.script_data_hash", json!(!c.script_data_hash.is_empty()));
    put(f, "body.guards", json!(c.guards.len()));
    put(f, "body.network_id", json!(c.network_id));
    put(f, "body.reference_inputs", json!(c.reference_inputs.len()));
    put(f, "body.voters", json!(c.votes.len()));
    put(f, "body.proposals", json!(c.proposals.len()));
    put(f, "body.treasury", bigint(&c.treasury_value));
    let donation = bigint(&c.treasury_donation);
    put(f, "body.donation", if donation.is_null() { json!(0) } else { donation });
    put(f, "body.required_top_level_guards", json!(c.required_top_level_guards.len()));
    put(f, "body.direct_deposits", json!(c.direct_deposits.len()));
    put(f, "body.account_balance_intervals", json!(c.account_balance_intervals.len()));
    let w = c.witnesses.clone().unwrap_or_default();
    put(f, "wits.vkey", json!(w.vkeywitness.len()));
    put(f, "wits.bootstrap", json!(w.bootstrap_witnesses.len()));
    let lang = |pick: fn(&u5c::script::Script) -> bool| {
        w.script.iter().filter(|s| s.script.as_ref().is_some_and(pick)).count()
    };
    put(f, "wits.native", json!(lang(|s| matches!(s, u5c::script::Script::Native(_)))));
    put(f, "wits.plutus_v1", json!(lang(|s| matches!(s, u5c::script::Script::PlutusV1(_)))));
    put(f, "wits.plutus_v2", json!(lang(|s| matches!(s, u5c::script::Script::PlutusV2(_)))));
    put(f, "wits.plutus_v3", json!(lang(|s| matches!(s, u5c::script::Script::PlutusV3(_)))));
    put(f, "wits.plutus_v4", json!(lang(|s| matches!(s, u5c::script::Script::PlutusV4(_)))));
    put(f, "wits.datums", json!(w.plutus_datums.len()));
    put(f, "wits.redeemers", json!(w.redeemers.len()));
    put(f, "aux_data", json!(c.auxiliary.is_some()));
}

fn tx_mapper_facts(t: &u5c::Tx) -> Facts {
    let mut f = Facts::new();
    if let Some(c) = &t.common {
        common_facts(c, &mut f);
    }
    put(&mut f, "body.fee", bigint(&t.fee));
    let col = t.collateral.clone().unwrap_or_default();
    put(&mut f, "body.collateral", json!(col.collateral.len()));
    put(&mut f, "body.collateral_return", json!(col.collateral_return.is_some()));
    put(&mut f, "body.total_collateral", bigint(&col.total_collateral));
    put(&mut f, "body.sub_transactions", json!(t.sub_transactions.len()));
    put(
        &mut f,
        "body.starting_account_balance_intervals",
        json!(t.starting_account_balance_intervals.len()),
    );
    put(&mut f, "successful", json!(t.successful));
    f
}

fn ppu_mapper_facts(u: &u5c::PParamsUpdate) -> Facts {
    let mut f = Facts::new();
    macro_rules! scalar {
        ($($k:literal => $field:ident),* $(,)?) => {
            $( if let Some(v) = u.$field { f.insert(format!("k{}", $k), json!(v)); } )*
        };
    }
    macro_rules! big {
        ($($k:literal => $field:ident),* $(,)?) => {
            $( if u.$field.is_some() { f.insert(format!("k{}", $k), bigint(&u.$field)); } )*
        };
    }
    macro_rules! structured {
        ($($k:literal => $field:ident),* $(,)?) => {
            $( if let Some(v) = &u.$field { f.insert(format!("k{}", $k), serde_json::to_value(v).unwrap_or(Value::Null)); } )*
        };
    }
    big! {
        0 => min_fee_coefficient, 1 => min_fee_constant, 5 => stake_key_deposit,
        6 => pool_deposit, 16 => min_pool_cost, 17 => coins_per_utxo_byte,
        30 => governance_action_deposit, 31 => drep_deposit,
        55 => ref_inputs_cost_per_multi_asset_policy, 56 => ref_inputs_cost_per_datum_byte,
    }
    scalar! {
        2 => max_block_body_size, 3 => max_tx_size, 4 => max_block_header_size,
        7 => pool_retirement_epoch_bound, 8 => desired_number_of_pools,
        22 => max_value_size, 23 => collateral_percentage, 24 => max_collateral_inputs,
        27 => min_committee_size, 28 => committee_term_limit,
        29 => governance_action_validity_period, 32 => drep_inactivity_period,
        34 => max_ref_script_size_per_block, 35 => max_ref_script_size_per_tx,
        36 => ref_script_cost_stride, 40 => leios_announcement_period_length,
        41 => leios_vote_period_length, 42 => leios_diffusion_period_length,
        43 => leios_committee_size, 45 => max_endorser_block_references_size,
        46 => max_endorser_block_txs_size, 48 => max_ref_script_size_per_endorser_block,
        49 => peras_min_candidate_block_age, 51 => peras_cert_boost,
        52 => peras_target_committee_size,
    }
    structured! {
        9 => pool_influence, 10 => monetary_expansion, 11 => treasury_expansion,
        18 => cost_models, 19 => prices, 20 => max_execution_units_per_transaction,
        21 => max_execution_units_per_block, 25 => pool_voting_thresholds,
        26 => drep_voting_thresholds, 33 => min_fee_script_ref_cost_per_byte,
        37 => ref_script_cost_multiplier, 38 => max_pledge_leverage, 39 => min_pool_margin,
        44 => leios_quorum_stake_threshold, 47 => max_endorser_block_execution_units,
        50 => peras_healing_factor, 53 => peras_bootstrap_round,
        54 => peras_quorum_threshold_safety_margin,
    }
    f
}

fn block_mapper_facts(b: &u5c::Block) -> Facts {
    let mut f = Facts::new();
    let h = b.header.clone().unwrap_or_default();
    let body = b.body.clone().unwrap_or_default();
    put(&mut f, "hash", json!(hex::encode(&h.hash)));
    put(&mut f, "block_no", json!(h.height));
    put(&mut f, "slot", json!(h.slot));
    put(&mut f, "contains_leios_cert", json!(h.body_contains_leios_certificate));
    put(&mut f, "eb_announcement", json!(h.endorser_block_announcement.is_some()));
    put(&mut f, "txs", json!(body.tx.len()));
    put(&mut f, "leios_certificate", json!(body.leios_certificate.is_some()));
    put(&mut f, "peras_certificate", json!(!body.peras_certificate.is_empty()));
    f
}

/// Runs one stage, turning a panic into an error string.
fn stage<T>(run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(r) => r,
        Err(e) => Err(format!("panic: {}", panic_text(e))),
    }
}

fn wrap_tx(body: &[u8], wits: &[u8]) -> Vec<u8> {
    let mut v = vec![0x83];
    v.extend_from_slice(body);
    v.extend_from_slice(wits);
    v.push(0xf6);
    v
}

/// A body with no inputs, no outputs and a zero fee.
const MIN_BODY: [u8; 7] = [0xa3, 0x00, 0x80, 0x01, 0x80, 0x02, 0x00];

struct Outcome {
    /// The decode facts and the pallas re-encoding of the value.
    decode: Result<(Facts, Vec<u8>), String>,
    mapper: Result<Facts, String>,
}

fn run_tx(mapper: &Mapper<NoLedger>, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let tx: dijkstra::MempoolTransaction =
            minicbor::decode(bytes).map_err(|e| e.to_string())?;
        let mut f = Facts::new();
        prefixed("body.", body_facts(&tx.transaction_body), &mut f);
        prefixed("wits.", wits_facts(&tx.transaction_witness_set), &mut f);
        put(&mut f, "aux_data", json!(matches!(tx.auxiliary_data, Nullable::Some(_))));
        Ok((f, enc(&tx)))
    });
    let mapper = stage(|| {
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, bytes).map_err(|e| e.to_string())?;
        Ok(tx_mapper_facts(&mapper.map_tx(&tx)))
    });
    Outcome { decode, mapper }
}

fn strip(prefix: &str, f: Facts) -> Facts {
    f.into_iter()
        .filter_map(|(k, v)| {
            if let Some(rest) = k.strip_prefix(prefix) {
                Some((rest.to_string(), v))
            } else {
                k.strip_prefix(&format!("raw.{prefix}"))
                    .map(|rest| (format!("raw.{rest}"), v))
            }
        })
        .collect()
}

fn run_body(mapper: &Mapper<NoLedger>, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let b: dijkstra::TransactionBody = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((body_facts(&b), enc(&b)))
    });
    let mapper = stage(|| {
        let wrapped = wrap_tx(bytes, &[0xa0]);
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &wrapped).map_err(|e| e.to_string())?;
        Ok(strip("body.", tx_mapper_facts(&mapper.map_tx(&tx))))
    });
    Outcome { decode, mapper }
}

fn run_wits(mapper: &Mapper<NoLedger>, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let w: dijkstra::WitnessSet = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((wits_facts(&w), enc(&w)))
    });
    let mapper = stage(|| {
        let wrapped = wrap_tx(&MIN_BODY, bytes);
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &wrapped).map_err(|e| e.to_string())?;
        Ok(strip("wits.", tx_mapper_facts(&mapper.map_tx(&tx))))
    });
    Outcome { decode, mapper }
}

fn run_ppu(mapper: &Mapper<NoLedger>, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let u: dijkstra::ProtocolParamUpdate =
            minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((ppu_facts(&u), enc(&u)))
    });
    let mapper = stage(|| {
        let u: dijkstra::ProtocolParamUpdate =
            minicbor::decode(bytes).map_err(|e| e.to_string())?;
        let m = MultiEraParamUpdate::Dijkstra(Box::new(Cow::Owned(u)));
        Ok(ppu_mapper_facts(&mapper.map_pparams_update(&m)))
    });
    Outcome { decode, mapper }
}

fn run_block(mapper: &Mapper<NoLedger>, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let b: dijkstra::Block = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((block_facts(&b), enc(&b)))
    });
    let mapper = stage(|| {
        let mut wrapped = vec![0x82, 0x08];
        wrapped.extend_from_slice(bytes);
        let b = MultiEraBlock::decode_dijkstra(&wrapped).map_err(|e| e.to_string())?;
        Ok(block_mapper_facts(&mapper.map_block(&b)))
    });
    Outcome { decode, mapper }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let kind = std::env::args().nth(1).expect("usage: pallas-probe tx|body|wits|ppu|block");
    let run = match kind.as_str() {
        "tx" => run_tx,
        "body" => run_body,
        "wits" => run_wits,
        "ppu" => run_ppu,
        "block" => run_block,
        k => panic!("unknown kind {k}"),
    };
    let mapper = Mapper::new(NoLedger);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin line");
        if line.trim().is_empty() {
            continue;
        }
        let input: Value = serde_json::from_str(&line).expect("json line");
        let hex_in = input["hex"].as_str().expect("hex field");
        let bytes = hex::decode(hex_in).expect("hex");
        let o = run(&mapper, &bytes);
        let mut rec = Map::new();
        rec.insert("i".into(), input["i"].clone());
        match o.decode {
            Ok((facts, again)) => {
                rec.insert("ok".into(), json!(true));
                rec.insert("facts".into(), Value::Object(facts));
                rec.insert("reencode_equal".into(), json!(again == bytes));
                if again != bytes {
                    rec.insert("reencode_hex".into(), json!(hex::encode(&again)));
                }
            }
            Err(e) => {
                rec.insert("ok".into(), json!(false));
                rec.insert("err".into(), json!(e));
            }
        }
        match o.mapper {
            Ok(facts) => {
                rec.insert("mapper_ok".into(), json!(true));
                rec.insert("mapper".into(), Value::Object(facts));
            }
            Err(e) => {
                rec.insert("mapper_ok".into(), json!(false));
                rec.insert("mapper_err".into(), json!(e));
            }
        }
        writeln!(out, "{}", Value::Object(rec)).expect("stdout");
    }
}
