//! Decodes generated Dijkstra values with pallas and prints each one as a value dump.
//!
//! Reads JSON lines carrying a `hex` field on stdin. For each line it prints one
//! JSON line with `dump`, the value pallas decoded in the form the ledger oracle
//! prints, the pallas re-encoding under `reencode_hex` when it differs from the
//! input, and under `schemas` the serde JSON of the v1alpha, v1beta and v1
//! messages the mappers build, or the decode error or panic text.

use std::borrow::Cow;
use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use pallas_codec::minicbor;
use pallas_codec::utils::{KeepRaw, Nullable};
use pallas_crypto::hash::Hasher;
use pallas_primitives::dijkstra as d;
use pallas_primitives::{PlutusData, StakeCredential};
use pallas_traverse::{Era, MultiEraBlock, MultiEraParamUpdate, MultiEraTx};
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

fn enc<T: minicbor::Encode<()>>(x: &T) -> Vec<u8> {
    minicbor::to_vec(x).expect("encode to a vec")
}

// Dump forms ------------------------------------------------------------------

fn cb<T: minicbor::Encode<()>>(x: &T) -> Value {
    raw(&enc(x))
}

fn raw(bytes: &[u8]) -> Value {
    json!({ "cbor": hex::encode(bytes) })
}

fn opt<T>(x: Option<&T>, f: impl FnOnce(&T) -> Value) -> Value {
    x.map(f).unwrap_or(Value::Null)
}

fn nullable<T: Clone>(x: &Nullable<T>, f: impl FnOnce(&T) -> Value) -> Value {
    match x {
        Nullable::Some(v) => f(v),
        Nullable::Null | Nullable::Undefined => Value::Null,
    }
}

fn set(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "set": items.into_iter().collect::<Vec<_>>() })
}

fn oset(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "oset": items.into_iter().collect::<Vec<_>>() })
}

fn map(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    json!({ "map": pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>() })
}

fn omap(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    json!({ "omap": pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>() })
}

fn tagged(tag: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    m.insert("tag".into(), json!(tag));
    for (k, v) in fields {
        m.insert(k.into(), v);
    }
    Value::Object(m)
}

fn obj(fields: Vec<(&str, Value)>) -> Value {
    Value::Object(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn set_cb<T: minicbor::Encode<()>>(x: Option<&[T]>) -> Value {
    set(x.unwrap_or(&[]).iter().map(cb))
}

fn map_cb<'a, K: minicbor::Encode<()> + 'a, V: minicbor::Encode<()> + 'a>(
    x: Option<impl IntoIterator<Item = (&'a K, &'a V)>>,
) -> Value {
    match x {
        None => map([]),
        Some(it) => map(it.into_iter().map(|(k, v)| (cb(k), cb(v)))),
    }
}

// Integers --------------------------------------------------------------------

/// The decimal digits of a big endian unsigned magnitude.
fn decimal(be: &[u8]) -> String {
    let mut n: Vec<u8> = be.iter().copied().skip_while(|b| *b == 0).collect();
    if n.is_empty() {
        return "0".into();
    }
    let mut out = Vec::new();
    while !n.is_empty() {
        let mut rem: u32 = 0;
        let mut q = Vec::with_capacity(n.len());
        for &b in &n {
            let cur = rem * 256 + b as u32;
            let digit = cur / 10;
            rem = cur % 10;
            if !(q.is_empty() && digit == 0) {
                q.push(digit as u8);
            }
        }
        out.push(b'0' + rem as u8);
        n = q;
    }
    out.reverse();
    String::from_utf8(out).expect("ascii digits")
}

fn plus_one(be: &[u8]) -> Vec<u8> {
    let mut v = be.to_vec();
    let mut i = v.len();
    loop {
        if i == 0 {
            v.insert(0, 1);
            return v;
        }
        i -= 1;
        if v[i] == 255 {
            v[i] = 0;
        } else {
            v[i] += 1;
            return v;
        }
    }
}

// Plutus data and scripts -------------------------------------------------------

fn data(x: &PlutusData) -> Value {
    use pallas_primitives::BigInt;
    match x {
        PlutusData::Constr(c) => {
            let ix = match c.tag {
                121..=127 => (c.tag - 121).to_string(),
                1280..=1400 => (c.tag - 1280 + 7).to_string(),
                102 => c.any_constructor.map(|i| i.to_string()).unwrap_or_default(),
                t => format!("tag {t}"),
            };
            tagged("constr", vec![("ix", json!(ix)), ("fields", Value::Array(c.fields.iter().map(data).collect()))])
        }
        PlutusData::Map(kvs) => tagged(
            "map",
            vec![("entries", Value::Array(kvs.iter().map(|(k, v)| json!([data(k), data(v)])).collect()))],
        ),
        PlutusData::Array(xs) => tagged("list", vec![("items", Value::Array(xs.iter().map(data).collect()))]),
        PlutusData::BigInt(BigInt::Int(i)) => tagged("int", vec![("v", json!(i128::from(i.0).to_string()))]),
        PlutusData::BigInt(BigInt::BigUInt(b)) => tagged("int", vec![("v", json!(decimal(b)))]),
        PlutusData::BigInt(BigInt::BigNInt(b)) => {
            tagged("int", vec![("v", json!(format!("-{}", decimal(&plus_one(b)))))])
        }
        PlutusData::BoundedBytes(b) => tagged("bytes", vec![("v", json!(hex::encode(b.as_slice())))]),
    }
}

fn native(x: &d::NativeScript) -> Value {
    use d::NativeScript as N;
    let all = |xs: &[N]| Value::Array(xs.iter().map(native).collect());
    match x {
        N::ScriptPubkey(h) => tagged("sig", vec![("hash", cb(h))]),
        N::ScriptAll(xs) => tagged("all", vec![("scripts", all(xs))]),
        N::ScriptAny(xs) => tagged("any", vec![("scripts", all(xs))]),
        N::ScriptNOfK(m, xs) => tagged("m_of", vec![("m", json!(m.to_string())), ("scripts", all(xs))]),
        N::InvalidBefore(s) => tagged("start", vec![("slot", cb(s))]),
        N::InvalidHereafter(s) => tagged("expire", vec![("slot", cb(s))]),
        N::ScriptRequireGuard(c) => tagged("guard", vec![("cred", cb(c))]),
    }
}

fn native_script(x: &d::NativeScript) -> Value {
    tagged("native", vec![("script", native(x))])
}

fn plutus<const N: usize>(x: &d::PlutusScript<N>) -> Value {
    tagged(&format!("plutus_v{N}"), vec![("bytes", json!(hex::encode(x.0.as_slice())))])
}

fn plutus_hash<const N: usize>(x: &d::PlutusScript<N>) -> Value {
    cb(&Hasher::<224>::hash_tagged(x.0.as_slice(), N as u8))
}

fn script_ref(x: &d::ScriptRef) -> Value {
    match x {
        d::ScriptRef::NativeScript(s) => native_script(s),
        d::ScriptRef::PlutusV1Script(s) => plutus(s),
        d::ScriptRef::PlutusV2Script(s) => plutus(s),
        d::ScriptRef::PlutusV3Script(s) => plutus(s),
        d::ScriptRef::PlutusV4Script(s) => plutus(s),
    }
}

// Outputs -----------------------------------------------------------------------

fn assets<A: minicbor::Encode<()>>(m: &d::Multiasset<A>) -> Value {
    map(m.iter().map(|(p, names)| (cb(p), map_cb(Some(names.iter())))))
}

fn output(x: &d::TransactionOutput) -> Value {
    use pallas_primitives::{alonzo, babbage::GenTransactionOutput, conway};
    match x {
        GenTransactionOutput::Legacy(o) => {
            let (coin, multi) = match &o.amount {
                alonzo::Value::Coin(c) => (cb(c), map([])),
                alonzo::Value::Multiasset(c, m) => (cb(c), assets(m)),
            };
            obj(vec![
                ("address", cb(&o.address)),
                ("coin", coin),
                ("assets", multi),
                ("datum", opt(o.datum_hash.as_ref(), |h| tagged("hash", vec![("hash", cb(h))]))),
                ("script", Value::Null),
            ])
        }
        GenTransactionOutput::PostAlonzo(o) => {
            let (coin, multi) = match &o.value {
                conway::Value::Coin(c) => (cb(c), map([])),
                conway::Value::Multiasset(c, m) => (cb(c), assets(m)),
            };
            let datum = opt(o.datum_option.as_ref(), |x| match &**x {
                d::DatumOption::Hash(h) => tagged("hash", vec![("hash", cb(h))]),
                d::DatumOption::Data(w) => tagged("inline", vec![("data", data(&w.0))]),
            });
            obj(vec![
                ("address", cb(&o.address)),
                ("coin", coin),
                ("assets", multi),
                ("datum", datum),
                ("script", opt(o.script_ref.as_ref(), |s| script_ref(&s.0))),
            ])
        }
    }
}

// Certificates and governance -----------------------------------------------------

fn cert(x: &d::Certificate) -> Value {
    match x {
        d::Certificate::PoolRegistration {
            operator,
            vrf_keyhash,
            bls_key,
            pledge,
            cost,
            margin,
            reward_account,
            pool_owners,
            relays,
            pool_metadata,
        } => tagged(
            "pool_reg",
            vec![
                ("operator", cb(operator)),
                ("vrf", cb(vrf_keyhash)),
                ("bls", opt(bls_key.as_ref(), |k| nullable(k, cb))),
                ("pledge", cb(pledge)),
                ("cost", cb(cost)),
                ("margin", cb(margin)),
                ("account", cb(reward_account)),
                ("owners", set(pool_owners.iter().map(cb))),
                ("relays", Value::Array(relays.iter().map(cb).collect())),
                ("metadata", opt(pool_metadata.as_ref(), cb)),
            ],
        ),
        other => cb(other),
    }
}

fn action(x: &d::GovAction) -> Value {
    match x {
        d::GovAction::TreasuryWithdrawals(ws, policy) => tagged(
            "treasury_withdrawals",
            vec![("withdrawals", map_cb(Some(ws.iter()))), ("policy", opt(policy.as_ref(), cb))],
        ),
        d::GovAction::UpdateCommittee(prev, remove, add, threshold) => tagged(
            "update_committee",
            vec![
                ("prev", opt(prev.as_ref(), cb)),
                ("remove", set(remove.iter().map(cb))),
                ("add", map_cb(Some(add.iter()))),
                ("threshold", cb(threshold)),
            ],
        ),
        other => cb(other),
    }
}

fn proposal(x: &d::ProposalProcedure) -> Value {
    obj(vec![
        ("deposit", cb(&x.deposit)),
        ("account", cb(&x.reward_account)),
        ("action", action(&x.gov_action)),
        ("anchor", cb(&x.anchor)),
    ])
}

fn votes(x: Option<&d::VotingProcedures>) -> Value {
    match x {
        None => map([]),
        Some(m) => map(m.iter().map(|(voter, vs)| (cb(voter), map_cb(Some(vs.iter()))))),
    }
}

fn guards(x: Option<&d::Guards>) -> Value {
    match x {
        None => oset([]),
        Some(d::Guards::AddrKeyhashes(hs)) => {
            oset(hs.iter().map(|h| cb(&StakeCredential::AddrKeyhash(*h))))
        }
        Some(d::Guards::Credentials(cs)) => oset(cs.iter().map(cb)),
    }
}

fn top_guards(x: Option<&d::RequiredTopLevelGuards>) -> Value {
    match x {
        None => map([]),
        Some(m) => map(m.iter().map(|(c, v)| (cb(c), nullable(v, data)))),
    }
}

fn donation(x: Option<&pallas_codec::utils::PositiveCoin>) -> Value {
    cb(&x.map(u64::from).unwrap_or(0))
}

// Bodies, witnesses, auxiliary data, transactions ------------------------------------

/// The fields a top level body and a sub transaction body both hold, by the oracle's names.
#[allow(clippy::too_many_arguments)]
fn shared<'a>(
    inputs: &[d::TransactionInput],
    reference_inputs: Option<&d::NonEmptySet<d::TransactionInput>>,
    outputs: &[d::TransactionOutput<'a>],
    certs: Option<&d::NonEmptySet<d::Certificate>>,
    withdrawals: Option<&d::Withdrawals>,
    ttl: Option<u64>,
    start: Option<u64>,
) -> Vec<(&'static str, Value)> {
    vec![
        ("inputs", set(inputs.iter().map(cb))),
        ("reference_inputs", set_cb(reference_inputs.map(|x| &x[..]))),
        ("outputs", Value::Array(outputs.iter().map(output).collect())),
        ("certs", oset(certs.map(|x| &x[..]).unwrap_or(&[]).iter().map(cert))),
        ("withdrawals", map_cb(withdrawals.map(|w| w.iter()))),
        ("ttl", opt(ttl.as_ref(), cb)),
        ("start", opt(start.as_ref(), cb)),
    ]
}

fn proposals(x: Option<&d::NonEmptySet<d::ProposalProcedure>>) -> Value {
    oset(x.map(|x| &x[..]).unwrap_or(&[]).iter().map(proposal))
}

fn mint(x: Option<&d::Mint>) -> Value {
    x.map(assets).unwrap_or_else(|| map([]))
}

fn body(b: &d::TransactionBody) -> Value {
    let mut f = shared(
        &b.inputs,
        b.reference_inputs.as_ref(),
        &b.outputs,
        b.certificates.as_ref(),
        b.withdrawals.as_ref(),
        b.ttl,
        b.validity_interval_start,
    );
    let subs = b.sub_transactions.as_ref().map(|x| &x[..]).unwrap_or(&[]);
    f.extend(vec![
        ("fee", cb(&b.fee)),
        ("collateral", set_cb(b.collateral.as_ref().map(|x| &x[..]))),
        ("collateral_return", opt(b.collateral_return.as_ref(), output)),
        ("total_collateral", opt(b.total_collateral.as_ref(), cb)),
        ("guards", guards(b.guards.as_ref())),
        ("mint", mint(b.mint.as_ref())),
        ("script_data_hash", opt(b.script_data_hash.as_ref(), cb)),
        ("aux_hash", opt(b.auxiliary_data_hash.as_ref(), cb)),
        ("network", opt(b.network_id.as_ref(), cb)),
        ("votes", votes(b.voting_procedures.as_ref())),
        ("proposals", proposals(b.proposal_procedures.as_ref())),
        ("treasury", opt(b.treasury_value.as_ref(), cb)),
        ("donation", donation(b.donation.as_ref())),
        (
            "sub_txs",
            omap(subs.iter().map(|s| {
                (cb(&Hasher::<256>::hash(s.sub_transaction_body.raw_cbor())), sub_tx(s))
            })),
        ),
        ("top_guards", top_guards(b.required_top_level_guards.as_ref())),
        ("deposits", map_cb(b.direct_deposits.as_ref().map(|x| x.iter()))),
        ("balance_intervals", map_cb(b.account_balance_intervals.as_ref().map(|x| x.iter()))),
        (
            "starting_balance_intervals",
            map_cb(b.starting_account_balance_intervals.as_ref().map(|x| x.iter())),
        ),
    ]);
    obj(f)
}

fn sub_body(b: &d::SubTransactionBody) -> Value {
    let mut f = shared(
        &b.inputs,
        b.reference_inputs.as_ref(),
        &b.outputs,
        b.certificates.as_ref(),
        b.withdrawals.as_ref(),
        b.ttl,
        b.validity_interval_start,
    );
    f.extend(vec![
        ("guards", guards(b.guards.as_ref())),
        ("mint", mint(b.mint.as_ref())),
        ("script_data_hash", opt(b.script_data_hash.as_ref(), cb)),
        ("aux_hash", opt(b.auxiliary_data_hash.as_ref(), cb)),
        ("network", opt(b.network_id.as_ref(), cb)),
        ("votes", votes(b.voting_procedures.as_ref())),
        ("proposals", proposals(b.proposal_procedures.as_ref())),
        ("treasury", opt(b.treasury_value.as_ref(), cb)),
        ("donation", donation(b.donation.as_ref())),
        ("top_guards", top_guards(b.required_top_level_guards.as_ref())),
        ("deposits", map_cb(b.direct_deposits.as_ref().map(|x| x.iter()))),
        ("balance_intervals", map_cb(b.account_balance_intervals.as_ref().map(|x| x.iter()))),
    ]);
    obj(f)
}

fn wits(w: &d::WitnessSet) -> Value {
    let mut scripts = Vec::new();
    for s in w.native_script.as_ref().map(|x| &x[..]).unwrap_or(&[]) {
        scripts.push((cb(&Hasher::<224>::hash_tagged(s.raw_cbor(), 0)), native_script(s)));
    }
    macro_rules! push_plutus {
        ($field:ident) => {
            for s in w.$field.as_ref().map(|x| &x[..]).unwrap_or(&[]) {
                scripts.push((plutus_hash(s), plutus(s)));
            }
        };
    }
    push_plutus!(plutus_v1_script);
    push_plutus!(plutus_v2_script);
    push_plutus!(plutus_v3_script);
    push_plutus!(plutus_v4_script);
    let datums: &[KeepRaw<PlutusData>] = w.plutus_data.as_ref().map(|x| &x[..]).unwrap_or(&[]);
    let redeemers = match &w.redeemer {
        None => map([]),
        Some(r) => map(r.0.iter().map(|(k, v)| {
            (cb(k), obj(vec![("data", data(&v.data)), ("units", cb(&v.ex_units))]))
        })),
    };
    obj(vec![
        ("vkeys", set_cb(w.vkeywitness.as_ref().map(|x| &x[..]))),
        ("boots", set_cb(w.bootstrap_witness.as_ref().map(|x| &x[..]))),
        ("scripts", map(scripts)),
        (
            "datums",
            map(datums.iter().map(|x| (cb(&Hasher::<256>::hash(x.raw_cbor())), data(x)))),
        ),
        ("redeemers", redeemers),
    ])
}

fn metadata(m: Option<&d::Metadata>) -> Value {
    map_cb(m.map(|x| x.iter()))
}

fn aux(a: &d::AuxiliaryData) -> Value {
    let natives = |xs: Option<&Vec<d::NativeScript>>| {
        Value::Array(xs.map(|x| &x[..]).unwrap_or(&[]).iter().map(native).collect())
    };
    let (md, ns, ps) = match a {
        d::AuxiliaryData::Shelley(md) => (metadata(Some(md)), natives(None), map([])),
        d::AuxiliaryData::ShelleyMa(x) => (
            metadata(Some(&x.transaction_metadata)),
            natives(x.auxiliary_scripts.as_ref()),
            map([]),
        ),
        d::AuxiliaryData::PostAlonzo(x) => {
            fn lang<const N: usize>(xs: Option<&Vec<d::PlutusScript<N>>>) -> Option<(Value, Value)> {
                xs.filter(|xs| !xs.is_empty()).map(|xs| {
                    (
                        json!(format!("plutus_v{N}")),
                        Value::Array(xs.iter().map(|s| json!(hex::encode(s.0.as_slice()))).collect()),
                    )
                })
            }
            let ps = [
                lang(x.plutus_v1_scripts.as_ref()),
                lang(x.plutus_v2_scripts.as_ref()),
                lang(x.plutus_v3_scripts.as_ref()),
                lang(x.plutus_v4_scripts.as_ref()),
            ];
            (
                metadata(x.metadata.as_ref()),
                natives(x.native_scripts.as_ref()),
                map(ps.into_iter().flatten()),
            )
        }
    };
    obj(vec![("metadata", md), ("native", ns), ("plutus", ps)])
}

fn sub_tx(s: &d::SubTransaction) -> Value {
    obj(vec![
        ("body", sub_body(&s.sub_transaction_body)),
        ("wits", wits(&s.transaction_witness_set)),
        ("aux", nullable(&s.auxiliary_data, |a| aux(a))),
    ])
}

fn tx(
    b: &d::TransactionBody,
    w: &d::WitnessSet,
    valid: bool,
    a: &Nullable<KeepRaw<d::AuxiliaryData>>,
) -> Value {
    obj(vec![
        ("body", body(b)),
        ("wits", wits(w)),
        ("valid", json!(valid)),
        ("aux", nullable(a, |a| aux(a))),
    ])
}

fn block(b: &d::Block) -> Value {
    let h = &b.header.header_body;
    let header = obj(vec![
        ("block_no", cb(&h.block_number)),
        ("slot", cb(&h.slot)),
        ("prev", cb(&h.prev_hash)),
        ("issuer_vk", cb(&h.issuer_vkey)),
        ("vrf_vk", cb(&h.vrf_vkey)),
        ("vrf", cb(&h.vrf_result)),
        ("body_size", cb(&h.block_body_size)),
        ("body_hash", cb(&h.block_body_hash)),
        ("ocert", cb(&h.operational_cert)),
        ("version", cb(&h.header_version_info)),
        ("leios_cert", json!(h.block_body_contains_leios_cert)),
        ("eb", nullable(&h.eb_announcement, cb)),
        ("sig", cb(&b.header.body_signature)),
        ("hash", cb(&Hasher::<256>::hash(b.header.raw_cbor()))),
    ]);
    obj(vec![
        ("header", header),
        (
            "txs",
            Value::Array(
                b.block_body
                    .transactions
                    .iter()
                    .map(|t| tx(&t.transaction_body, &t.transaction_witness_set, t.success, &t.auxiliary_data))
                    .collect(),
            ),
        ),
        ("leios_cert", nullable(&b.block_body.leios_certificate, cb)),
        ("peras_cert", nullable(&b.block_body.peras_certificate, cb)),
    ])
}

// Running -------------------------------------------------------------------------

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "non string panic".to_string()
    }
}

/// Runs one stage, turning a panic into an error string.
fn stage<T>(run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(r) => r,
        Err(e) => Err(format!("panic: {}", panic_text(e))),
    }
}

fn message<T: serde::Serialize>(x: &T) -> Result<Value, String> {
    serde_json::to_value(x).map_err(|e| e.to_string())
}

struct Mappers {
    v1alpha: pallas_utxorpc::v1alpha::Mapper<NoLedger>,
    v1beta: pallas_utxorpc::v1beta::Mapper<NoLedger>,
    v1: pallas_utxorpc::v1::Mapper<NoLedger>,
}

/// One message per schema, in v1alpha, v1beta, v1 order.
type Mapped = [Result<Value, String>; 3];

const SCHEMAS: [&str; 3] = ["v1alpha", "v1beta", "v1"];

struct Outcome {
    /// The value dump and the pallas re-encoding of the value.
    decode: Result<(Value, Vec<u8>), String>,
    mapped: Mapped,
}

fn map_tx_all(m: &Mappers, tx_bytes: &[u8]) -> Mapped {
    let decode = || MultiEraTx::decode_for_era(Era::Dijkstra, tx_bytes).map_err(|e| e.to_string());
    [
        stage(|| message(&m.v1alpha.map_tx(&decode()?))),
        stage(|| message(&m.v1beta.map_tx(&decode()?))),
        stage(|| message(&m.v1.map_tx(&decode()?))),
    ]
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

fn run_tx(m: &Mappers, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let t: d::MempoolTransaction = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        // A mempool transaction that decodes is phase two valid, since the decoder refuses `false`.
        let dump = tx(&t.transaction_body, &t.transaction_witness_set, true, &t.auxiliary_data);
        Ok((dump, enc(&t)))
    });
    Outcome { decode, mapped: map_tx_all(m, bytes) }
}

fn run_body(m: &Mappers, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let b: d::TransactionBody = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((body(&b), enc(&b)))
    });
    Outcome { decode, mapped: map_tx_all(m, &wrap_tx(bytes, &[0xa0])) }
}

fn run_wits(m: &Mappers, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let w: d::WitnessSet = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((wits(&w), enc(&w)))
    });
    Outcome { decode, mapped: map_tx_all(m, &wrap_tx(&MIN_BODY, bytes)) }
}

fn run_ppu(m: &Mappers, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let u: d::ProtocolParamUpdate = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        let again = enc(&u);
        Ok((raw(&again), again))
    });
    let update = || -> Result<MultiEraParamUpdate<'static>, String> {
        let u: d::ProtocolParamUpdate = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok(MultiEraParamUpdate::Dijkstra(Box::new(Cow::Owned(u))))
    };
    let mapped = [
        stage(|| message(&m.v1alpha.map_pparams_update(&update()?))),
        stage(|| message(&m.v1beta.map_pparams_update(&update()?))),
        stage(|| message(&m.v1.map_pparams_update(&update()?))),
    ];
    Outcome { decode, mapped }
}

fn run_block(m: &Mappers, bytes: &[u8]) -> Outcome {
    let decode = stage(|| {
        let b: d::Block = minicbor::decode(bytes).map_err(|e| e.to_string())?;
        Ok((block(&b), enc(&b)))
    });
    let mut wrapped = vec![0x82, 0x08];
    wrapped.extend_from_slice(bytes);
    let decoded = || MultiEraBlock::decode_dijkstra(&wrapped).map_err(|e| e.to_string());
    let mapped = [
        stage(|| message(&m.v1alpha.map_block(&decoded()?))),
        stage(|| message(&m.v1beta.map_block(&decoded()?))),
        stage(|| message(&m.v1.map_block(&decoded()?))),
    ];
    Outcome { decode, mapped }
}

fn main_inner() {
    std::panic::set_hook(Box::new(|_| {}));
    let kind = std::env::args().nth(1).expect("usage: pallas-probe-value tx|body|wits|ppu|block");
    let run = match kind.as_str() {
        "tx" => run_tx,
        "body" => run_body,
        "wits" => run_wits,
        "ppu" => run_ppu,
        "block" => run_block,
        k => panic!("unknown kind {k}"),
    };
    let mappers = Mappers {
        v1alpha: pallas_utxorpc::v1alpha::Mapper::new(NoLedger),
        v1beta: pallas_utxorpc::v1beta::Mapper::new(NoLedger),
        v1: pallas_utxorpc::v1::Mapper::new(NoLedger),
    };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.expect("stdin line");
        if line.trim().is_empty() {
            continue;
        }
        let input: Value = serde_json::from_str(&line).expect("json line");
        let bytes = hex::decode(input["hex"].as_str().expect("hex field")).expect("hex");
        let o = run(&mappers, &bytes);
        let mut rec = Map::new();
        rec.insert("i".into(), input["i"].clone());
        match o.decode {
            Ok((dump, again)) => {
                rec.insert("ok".into(), json!(true));
                rec.insert("dump".into(), dump);
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
        let mut schemas = Map::new();
        for (name, r) in SCHEMAS.iter().zip(o.mapped) {
            let v = match r {
                Ok(msg) => json!({ "ok": true, "msg": msg }),
                Err(e) => json!({ "ok": false, "err": e }),
            };
            schemas.insert(name.to_string(), v);
        }
        rec.insert("schemas".into(), Value::Object(schemas));
        writeln!(out, "{}", Value::Object(rec)).expect("stdout");
    }
}

fn main() {
    // Plutus data nests without a bound, so the walk runs on a large stack.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(main_inner)
        .expect("spawn")
        .join()
        .expect("join");
}
