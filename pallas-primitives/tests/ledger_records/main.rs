//! Dijkstra values decoded by pallas compared leaf by leaf with the ledger's
//! reading of them.

mod cbor;
mod differences;

use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use pallas_codec::minicbor;
use pallas_codec::utils::{KeepRaw, Nullable};
use pallas_crypto::hash::Hasher;
use pallas_primitives::dijkstra;
use pallas_primitives::{BigInt, PlutusData, StakeCredential};
use reference_comparison::tree::{
    Diff, Normal, map, normal, object, ordered_map, ordered_set, set, tagged,
};
use reference_comparison::{
    Allowance, Allowed, Answer, ComparedLayer, Coverage, Disagreement, Expected, Key, Outcome,
    Records, Reference, Row, WholeInput, catch, judge, kinds_from_env,
};
use serde_json::{Value, json};

use cbor::{decimal, negative_decimal, read_leaf};

/// The rules of `defs.cddl` no recorded value credits, with the reason none
/// does.
const UNCREDITED: &[(&str, &str)] = &[
    ("language", "no rule refers to it"),
    ("potential_languages", "no rule refers to it"),
    ("signkey_kes", "no rule refers to it"),
    (
        "eb_references",
        "pallas reads no value of it, see the eb entry of differences::LIST",
    ),
    (
        "eb_reference",
        "pallas reads no value of it, see the eb entry of differences::LIST",
    ),
];

fn encode<T: minicbor::Encode<()>>(x: &T) -> Vec<u8> {
    minicbor::to_vec(x).expect("encode to a vec")
}

fn encoded<T: minicbor::Encode<()>>(x: &T) -> Value {
    cbor::leaf(&encode(x))
}

fn optional<T>(x: Option<&T>, f: impl FnOnce(&T) -> Value) -> Value {
    x.map(f).unwrap_or(Value::Null)
}

fn nullable<T: Clone>(x: &Nullable<T>, f: impl FnOnce(&T) -> Value) -> Value {
    match x {
        Nullable::Some(v) => f(v),
        Nullable::Null | Nullable::Undefined => Value::Null,
    }
}

fn encoded_set<T: minicbor::Encode<()>>(x: Option<&[T]>) -> Value {
    set(x.unwrap_or(&[]).iter().map(encoded))
}

fn encoded_map<'a, K: minicbor::Encode<()> + 'a, V: minicbor::Encode<()> + 'a>(
    x: Option<impl IntoIterator<Item = (&'a K, &'a V)>>,
) -> Value {
    match x {
        None => map([]),
        Some(it) => map(it.into_iter().map(|(k, v)| (encoded(k), encoded(v)))),
    }
}

fn data(x: &PlutusData) -> Value {
    match x {
        PlutusData::Constr(c) => {
            let index = match c.tag {
                121..=127 => (c.tag - 121).to_string(),
                1280..=1400 => (c.tag - 1280 + 7).to_string(),
                102 => c.any_constructor.map(|i| i.to_string()).unwrap_or_default(),
                t => format!("tag {t}"),
            };
            tagged(
                "constr",
                vec![
                    ("ix", json!(index)),
                    ("fields", Value::Array(c.fields.iter().map(data).collect())),
                ],
            )
        }
        PlutusData::Map(entries) => tagged(
            "map",
            vec![(
                "entries",
                Value::Array(
                    entries
                        .iter()
                        .map(|(k, v)| json!([data(k), data(v)]))
                        .collect(),
                ),
            )],
        ),
        PlutusData::Array(xs) => tagged(
            "list",
            vec![("items", Value::Array(xs.iter().map(data).collect()))],
        ),
        PlutusData::BigInt(BigInt::Int(i)) => {
            tagged("int", vec![("v", json!(i128::from(i.0).to_string()))])
        }
        PlutusData::BigInt(BigInt::BigUInt(b)) => tagged("int", vec![("v", json!(decimal(b)))]),
        PlutusData::BigInt(BigInt::BigNInt(b)) => {
            tagged("int", vec![("v", json!(negative_decimal(b)))])
        }
        PlutusData::BoundedBytes(b) => {
            tagged("bytes", vec![("v", json!(hex::encode(b.as_slice())))])
        }
    }
}

fn native(x: &dijkstra::NativeScript) -> Value {
    use dijkstra::NativeScript as S;
    let all = |xs: &[S]| Value::Array(xs.iter().map(native).collect());
    match x {
        S::ScriptPubkey(h) => tagged("sig", vec![("hash", encoded(h))]),
        S::ScriptAll(xs) => tagged("all", vec![("scripts", all(xs))]),
        S::ScriptAny(xs) => tagged("any", vec![("scripts", all(xs))]),
        S::ScriptNOfK(m, xs) => tagged(
            "m_of",
            vec![("m", json!(m.to_string())), ("scripts", all(xs))],
        ),
        S::InvalidBefore(s) => tagged("start", vec![("slot", encoded(s))]),
        S::InvalidHereafter(s) => tagged("expire", vec![("slot", encoded(s))]),
        S::ScriptRequireGuard(c) => tagged("guard", vec![("cred", encoded(c))]),
    }
}

fn native_script(x: &dijkstra::NativeScript) -> Value {
    tagged("native", vec![("script", native(x))])
}

fn plutus<const V: usize>(x: &dijkstra::PlutusScript<V>) -> Value {
    tagged(
        &format!("plutus_v{V}"),
        vec![("bytes", json!(hex::encode(x.0.as_slice())))],
    )
}

fn plutus_hash<const V: usize>(x: &dijkstra::PlutusScript<V>) -> Value {
    encoded(&Hasher::<224>::hash_tagged(x.0.as_slice(), V as u8))
}

fn script_ref(x: &dijkstra::ScriptRef) -> Value {
    match x {
        dijkstra::ScriptRef::NativeScript(s) => native_script(s),
        dijkstra::ScriptRef::PlutusV1Script(s) => plutus(s),
        dijkstra::ScriptRef::PlutusV2Script(s) => plutus(s),
        dijkstra::ScriptRef::PlutusV3Script(s) => plutus(s),
        dijkstra::ScriptRef::PlutusV4Script(s) => plutus(s),
    }
}

fn assets<A: minicbor::Encode<()>>(m: &dijkstra::Multiasset<A>) -> Value {
    map(m
        .iter()
        .map(|(p, names)| (encoded(p), encoded_map(Some(names.iter())))))
}

fn output(x: &dijkstra::TransactionOutput) -> Value {
    use pallas_primitives::{alonzo, conway};
    match x {
        dijkstra::TransactionOutput::Legacy(o) => {
            let (coin, multi) = match &o.amount {
                alonzo::Value::Coin(c) => (encoded(c), map([])),
                alonzo::Value::Multiasset(c, m) => (encoded(c), assets(m)),
            };
            object(vec![
                ("address", encoded(&o.address)),
                ("coin", coin),
                ("assets", multi),
                (
                    "datum",
                    optional(o.datum_hash.as_ref(), |h| {
                        tagged("hash", vec![("hash", encoded(h))])
                    }),
                ),
                ("script", Value::Null),
            ])
        }
        dijkstra::TransactionOutput::PostAlonzo(o) => {
            let (coin, multi) = match &o.value {
                conway::Value::Coin(c) => (encoded(c), map([])),
                conway::Value::Multiasset(c, m) => (encoded(c), assets(m)),
            };
            let datum = optional(o.datum_option.as_ref(), |x| match &**x {
                dijkstra::DatumOption::Hash(h) => tagged("hash", vec![("hash", encoded(h))]),
                dijkstra::DatumOption::Data(w) => tagged("inline", vec![("data", data(&w.0))]),
            });
            object(vec![
                ("address", encoded(&o.address)),
                ("coin", coin),
                ("assets", multi),
                ("datum", datum),
                (
                    "script",
                    optional(o.script_ref.as_ref(), |s| script_ref(&s.0)),
                ),
            ])
        }
    }
}

fn certificate(x: &dijkstra::Certificate) -> Value {
    match x {
        dijkstra::Certificate::PoolRegistration {
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
                ("operator", encoded(operator)),
                ("vrf", encoded(vrf_keyhash)),
                ("bls", optional(bls_key.as_ref(), |k| nullable(k, encoded))),
                ("pledge", encoded(pledge)),
                ("cost", encoded(cost)),
                ("margin", encoded(margin)),
                ("account", encoded(reward_account)),
                ("owners", set(pool_owners.iter().map(encoded))),
                ("relays", Value::Array(relays.iter().map(encoded).collect())),
                ("metadata", optional(pool_metadata.as_ref(), encoded)),
            ],
        ),
        other => encoded(other),
    }
}

fn action(x: &dijkstra::GovAction) -> Value {
    match x {
        dijkstra::GovAction::TreasuryWithdrawals(ws, policy) => tagged(
            "treasury_withdrawals",
            vec![
                ("withdrawals", encoded_map(Some(ws.iter()))),
                ("policy", optional(policy.as_ref(), encoded)),
            ],
        ),
        dijkstra::GovAction::UpdateCommittee(prev, remove, add, threshold) => tagged(
            "update_committee",
            vec![
                ("prev", optional(prev.as_ref(), encoded)),
                ("remove", set(remove.iter().map(encoded))),
                ("add", encoded_map(Some(add.iter()))),
                ("threshold", encoded(threshold)),
            ],
        ),
        other => encoded(other),
    }
}

fn proposal(x: &dijkstra::ProposalProcedure) -> Value {
    object(vec![
        ("deposit", encoded(&x.deposit)),
        ("account", encoded(&x.reward_account)),
        ("action", action(&x.gov_action)),
        ("anchor", encoded(&x.anchor)),
    ])
}

fn votes(x: Option<&dijkstra::VotingProcedures>) -> Value {
    match x {
        None => map([]),
        Some(m) => map(m
            .iter()
            .map(|(voter, vs)| (encoded(voter), encoded_map(Some(vs.iter()))))),
    }
}

fn guards(x: Option<&dijkstra::Guards>) -> Value {
    match x {
        None => ordered_set([]),
        Some(dijkstra::Guards::AddrKeyhashes(hashes)) => ordered_set(
            hashes
                .iter()
                .map(|h| encoded(&StakeCredential::AddrKeyhash(*h))),
        ),
        Some(dijkstra::Guards::Credentials(credentials)) => {
            ordered_set(credentials.iter().map(encoded))
        }
    }
}

fn top_level_guards(x: Option<&dijkstra::RequiredTopLevelGuards>) -> Value {
    match x {
        None => map([]),
        Some(m) => map(m.iter().map(|(c, v)| (encoded(c), nullable(v, data)))),
    }
}

fn donation(x: Option<&dijkstra::PositiveCoin>) -> Value {
    encoded(&x.map(|coin| u64::from(*coin)).unwrap_or(0))
}

/// The fields a top level body and a sub transaction body both hold, by the
/// ledger reading's names.
fn shared<'a>(
    inputs: &[dijkstra::TransactionInput],
    reference_inputs: Option<&dijkstra::NonEmptySet<dijkstra::TransactionInput>>,
    outputs: &[dijkstra::TransactionOutput<'a>],
    certificates: Option<&dijkstra::NonEmptySet<dijkstra::Certificate>>,
    withdrawals: Option<&dijkstra::Withdrawals>,
    ttl: Option<u64>,
    start: Option<u64>,
) -> Vec<(&'static str, Value)> {
    vec![
        ("inputs", set(inputs.iter().map(encoded))),
        (
            "reference_inputs",
            encoded_set(reference_inputs.map(|x| &x[..])),
        ),
        (
            "outputs",
            Value::Array(outputs.iter().map(output).collect()),
        ),
        (
            "certs",
            ordered_set(
                certificates
                    .map(|x| &x[..])
                    .unwrap_or(&[])
                    .iter()
                    .map(certificate),
            ),
        ),
        ("withdrawals", encoded_map(withdrawals.map(|w| w.iter()))),
        ("ttl", optional(ttl.as_ref(), encoded)),
        ("start", optional(start.as_ref(), encoded)),
    ]
}

fn proposals(x: Option<&dijkstra::NonEmptySet<dijkstra::ProposalProcedure>>) -> Value {
    ordered_set(x.map(|x| &x[..]).unwrap_or(&[]).iter().map(proposal))
}

fn mint(x: Option<&dijkstra::Mint>) -> Value {
    x.map(assets).unwrap_or_else(|| map([]))
}

fn body(b: &dijkstra::TransactionBody) -> Value {
    let mut f = shared(
        &b.inputs,
        b.reference_inputs.as_ref(),
        &b.outputs,
        b.certificates.as_ref(),
        b.withdrawals.as_ref(),
        b.ttl,
        b.validity_interval_start,
    );
    let sub_transactions = b.sub_transactions.as_ref().map(|x| &x[..]).unwrap_or(&[]);
    f.extend(vec![
        ("fee", encoded(&b.fee)),
        (
            "collateral",
            encoded_set(b.collateral.as_ref().map(|x| &x[..])),
        ),
        (
            "collateral_return",
            optional(b.collateral_return.as_ref(), output),
        ),
        (
            "total_collateral",
            optional(b.total_collateral.as_ref(), encoded),
        ),
        ("guards", guards(b.guards.as_ref())),
        ("mint", mint(b.mint.as_ref())),
        (
            "script_data_hash",
            optional(b.script_data_hash.as_ref(), encoded),
        ),
        (
            "aux_hash",
            optional(b.auxiliary_data_hash.as_ref(), encoded),
        ),
        ("network", optional(b.network_id.as_ref(), encoded)),
        ("votes", votes(b.voting_procedures.as_ref())),
        ("proposals", proposals(b.proposal_procedures.as_ref())),
        ("treasury", optional(b.treasury_value.as_ref(), encoded)),
        ("donation", donation(b.donation.as_ref())),
        (
            "sub_txs",
            ordered_map(sub_transactions.iter().map(|s| {
                (
                    encoded(&Hasher::<256>::hash(s.sub_transaction_body.raw_cbor())),
                    sub_transaction(s),
                )
            })),
        ),
        (
            "top_guards",
            top_level_guards(b.required_top_level_guards.as_ref()),
        ),
        (
            "deposits",
            encoded_map(b.direct_deposits.as_ref().map(|x| x.iter())),
        ),
        (
            "balance_intervals",
            encoded_map(b.account_balance_intervals.as_ref().map(|x| x.iter())),
        ),
        (
            "starting_balance_intervals",
            encoded_map(
                b.starting_account_balance_intervals
                    .as_ref()
                    .map(|x| x.iter()),
            ),
        ),
    ]);
    object(f)
}

fn sub_transaction_body(b: &dijkstra::SubTransactionBody) -> Value {
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
        (
            "script_data_hash",
            optional(b.script_data_hash.as_ref(), encoded),
        ),
        (
            "aux_hash",
            optional(b.auxiliary_data_hash.as_ref(), encoded),
        ),
        ("network", optional(b.network_id.as_ref(), encoded)),
        ("votes", votes(b.voting_procedures.as_ref())),
        ("proposals", proposals(b.proposal_procedures.as_ref())),
        ("treasury", optional(b.treasury_value.as_ref(), encoded)),
        ("donation", donation(b.donation.as_ref())),
        (
            "top_guards",
            top_level_guards(b.required_top_level_guards.as_ref()),
        ),
        (
            "deposits",
            encoded_map(b.direct_deposits.as_ref().map(|x| x.iter())),
        ),
        (
            "balance_intervals",
            encoded_map(b.account_balance_intervals.as_ref().map(|x| x.iter())),
        ),
    ]);
    object(f)
}

fn witness_set(w: &dijkstra::WitnessSet) -> Value {
    let mut scripts = Vec::new();
    for s in w.native_script.as_ref().map(|x| &x[..]).unwrap_or(&[]) {
        scripts.push((
            encoded(&Hasher::<224>::hash_tagged(s.raw_cbor(), 0)),
            native_script(s),
        ));
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
            (
                encoded(k),
                object(vec![
                    ("data", data(&v.data)),
                    ("units", encoded(&v.ex_units)),
                ]),
            )
        })),
    };
    object(vec![
        ("vkeys", encoded_set(w.vkeywitness.as_ref().map(|x| &x[..]))),
        (
            "boots",
            encoded_set(w.bootstrap_witness.as_ref().map(|x| &x[..])),
        ),
        ("scripts", map(scripts)),
        (
            "datums",
            map(datums
                .iter()
                .map(|x| (encoded(&Hasher::<256>::hash(x.raw_cbor())), data(x)))),
        ),
        ("redeemers", redeemers),
    ])
}

fn metadata(m: Option<&dijkstra::Metadata>) -> Value {
    encoded_map(m.map(|x| x.iter()))
}

/// A native script as an auxiliary data field holds it, with or without its
/// original bytes.
trait AsNative {
    fn native(&self) -> &dijkstra::NativeScript;
}

impl AsNative for dijkstra::NativeScript {
    fn native(&self) -> &dijkstra::NativeScript {
        self
    }
}

impl AsNative for KeepRaw<'_, dijkstra::NativeScript> {
    fn native(&self) -> &dijkstra::NativeScript {
        self
    }
}

fn natives<T: AsNative>(xs: Option<&Vec<T>>) -> Value {
    Value::Array(
        xs.into_iter()
            .flatten()
            .map(|x| native(x.native()))
            .collect(),
    )
}

fn auxiliary(a: &dijkstra::AuxiliaryData) -> Value {
    let (metadata_tree, native_scripts, plutus_scripts) = match a {
        dijkstra::AuxiliaryData::Shelley(m) => (
            metadata(Some(m)),
            natives::<dijkstra::NativeScript>(None),
            map([]),
        ),
        dijkstra::AuxiliaryData::ShelleyMa(x) => (
            metadata(Some(&x.transaction_metadata)),
            natives(Some(&x.auxiliary_scripts)),
            map([]),
        ),
        dijkstra::AuxiliaryData::PostAlonzo(x) => {
            fn language<const V: usize>(
                xs: Option<&Vec<dijkstra::PlutusScript<V>>>,
            ) -> Option<(Value, Value)> {
                xs.filter(|xs| !xs.is_empty()).map(|xs| {
                    (
                        json!(format!("plutus_v{V}")),
                        Value::Array(
                            xs.iter()
                                .map(|s| json!(hex::encode(s.0.as_slice())))
                                .collect(),
                        ),
                    )
                })
            }
            let languages = [
                language(x.plutus_v1_scripts.as_ref()),
                language(x.plutus_v2_scripts.as_ref()),
                language(x.plutus_v3_scripts.as_ref()),
                language(x.plutus_v4_scripts.as_ref()),
            ];
            (
                metadata(x.metadata.as_ref()),
                natives(x.native_scripts.as_ref()),
                map(languages.into_iter().flatten()),
            )
        }
    };
    object(vec![
        ("metadata", metadata_tree),
        ("native", native_scripts),
        ("plutus", plutus_scripts),
    ])
}

fn sub_transaction(s: &dijkstra::SubTransaction) -> Value {
    object(vec![
        ("body", sub_transaction_body(&s.sub_transaction_body)),
        ("wits", witness_set(&s.transaction_witness_set)),
        ("aux", nullable(&s.auxiliary_data, |a| auxiliary(a))),
    ])
}

fn transaction(
    b: &KeepRaw<dijkstra::TransactionBody>,
    w: &dijkstra::WitnessSet,
    valid: bool,
    a: &Nullable<KeepRaw<dijkstra::AuxiliaryData>>,
) -> Value {
    object(vec![
        ("id", encoded(&Hasher::<256>::hash(b.raw_cbor()))),
        ("body", body(b)),
        ("wits", witness_set(w)),
        ("valid", json!(valid)),
        ("aux", nullable(a, |a| auxiliary(a))),
    ])
}

fn block(b: &dijkstra::Block) -> Value {
    let mut fields = match &b.header.header_body {
        dijkstra::HeaderBody::Leios(h) => vec![
            ("block_no", encoded(&h.block_number)),
            ("slot", encoded(&h.slot)),
            ("prev", encoded(&h.prev_hash)),
            ("issuer_vk", encoded(&h.issuer_vkey)),
            ("vrf_vk", encoded(&h.vrf_vkey)),
            ("vrf", encoded(&h.vrf_result)),
            ("body_size", encoded(&h.block_body_size)),
            ("body_hash", encoded(&h.block_body_hash)),
            ("ocert", encoded(&h.operational_cert)),
            ("version", encoded(&h.header_version_info)),
            ("leios_cert", json!(h.block_body_contains_leios_cert)),
            ("eb", nullable(&h.eb_announcement, encoded)),
        ],
        // The pinned ledger's header has no Praos form, so this reading never matches its own.
        dijkstra::HeaderBody::Praos(h) => vec![
            ("block_no", encoded(&h.block_number)),
            ("slot", encoded(&h.slot)),
            ("prev", encoded(&h.prev_hash)),
            ("issuer_vk", encoded(&h.issuer_vkey)),
            ("vrf_vk", encoded(&h.vrf_vkey)),
            ("vrf", encoded(&h.vrf_result)),
            ("body_size", encoded(&h.block_body_size)),
            ("body_hash", encoded(&h.block_body_hash)),
            ("ocert", encoded(&h.operational_cert)),
            ("protocol_version", encoded(&h.protocol_version)),
        ],
    };
    fields.push(("sig", encoded(&b.header.body_signature)));
    fields.push(("hash", encoded(&Hasher::<256>::hash(b.header.raw_cbor()))));
    let header = object(fields);
    let transactions = b
        .block_body
        .transactions
        .iter()
        .map(|t| {
            transaction(
                &t.transaction_body,
                &t.transaction_witness_set,
                t.success,
                &t.auxiliary_data,
            )
        })
        .collect();
    object(vec![
        ("header", header),
        ("txs", Value::Array(transactions)),
        (
            "leios_cert",
            nullable(&b.block_body.leios_certificate, encoded),
        ),
        (
            "peras_cert",
            nullable(&b.block_body.peras_certificate, encoded),
        ),
    ])
}

/// A value as pallas reads it, the bytes pallas encodes it to and the number
/// of bytes after it.
type Reading = Result<(Value, Vec<u8>, usize), String>;

/// Reads the first item of `bytes` and counts the bytes after it.
fn item<'b, T: minicbor::Decode<'b, ()>>(bytes: &'b [u8]) -> Result<(T, usize), String> {
    let mut decoder = minicbor::Decoder::new(bytes);
    let value = decoder.decode().map_err(|e| e.to_string())?;
    Ok((value, bytes.len() - decoder.position()))
}

fn read_transaction(bytes: &[u8]) -> Reading {
    let (t, leftover): (dijkstra::MempoolTransaction, _) = item(bytes)?;
    // A mempool transaction that decodes is phase two valid, since the decoder
    // refuses `false`.
    let dump = transaction(
        &t.transaction_body,
        &t.transaction_witness_set,
        true,
        &t.auxiliary_data,
    );
    Ok((dump, encode(&t), leftover))
}

fn read_body(bytes: &[u8]) -> Reading {
    let (b, leftover): (dijkstra::TransactionBody, _) = item(bytes)?;
    let mut dump = body(&b);
    dump["id"] = encoded(&Hasher::<256>::hash(&bytes[..bytes.len() - leftover]));
    Ok((dump, encode(&b), leftover))
}

fn read_witness_set(bytes: &[u8]) -> Reading {
    let (w, leftover): (dijkstra::WitnessSet, _) = item(bytes)?;
    Ok((witness_set(&w), encode(&w), leftover))
}

fn read_parameter_update(bytes: &[u8]) -> Reading {
    let (u, leftover): (dijkstra::ProtocolParamUpdate, _) = item(bytes)?;
    let again = encode(&u);
    Ok((cbor::leaf(&again), again, leftover))
}

fn read_block(bytes: &[u8]) -> Reading {
    let (b, leftover): (dijkstra::Block, _) = item(bytes)?;
    Ok((block(&b), encode(&b), leftover))
}

fn read_endorser_block(bytes: &[u8]) -> Reading {
    let (e, leftover): (dijkstra::EndorserBlock, _) = item(bytes)?;
    let dump = ordered_map(e.iter().map(|(h, n)| (encoded(h), encoded(n))));
    Ok((dump, encode(&e), leftover))
}

fn reader(kind: &str) -> fn(&[u8]) -> Reading {
    match kind {
        "tx" => read_transaction,
        "body" => read_body,
        "wits" => read_witness_set,
        "ppu" => read_parameter_update,
        "block" => read_block,
        "eb" => read_endorser_block,
        k => panic!("the records declare class {k}, which this layer has no reader for"),
    }
}

/// How a value's own encoding by pallas reads back.
#[derive(Debug, PartialEq)]
enum Reencoding {
    Same,
    Refused(String),
    Differs(String),
}

/// A reading of one value: its normal form, the paths of the duplicates it
/// holds, how its encoding reads back and the number of bytes after it.
#[derive(Debug, PartialEq)]
struct Decoded {
    value: Normal,
    duplicates: Vec<String>,
    reencoding: Reencoding,
    leftover: usize,
}

impl Decoded {
    /// The ledger's reading of a value, as its records write it, with no
    /// bytes after it.
    fn of_ledger(dump: &Value) -> Decoded {
        Decoded {
            value: normal(dump, &read_leaf, &mut Vec::new()),
            duplicates: Vec::new(),
            reencoding: Reencoding::Same,
            leftover: 0,
        }
    }
}

/// Reads `bytes` as a value of `kind` with pallas and reads its encoding back.
fn decode(kind: &str, bytes: &[u8]) -> Result<Decoded, String> {
    let read = reader(kind);
    let (dump, again, leftover) = read(bytes)?;
    let mut duplicates = Vec::new();
    let value = normal(&dump, &read_leaf, &mut duplicates);
    let reread = (again != bytes[..bytes.len() - leftover]).then(|| read(&again));
    let reencoding = match reread {
        None => Reencoding::Same,
        Some(Err(e)) => Reencoding::Refused(e),
        Some(Ok((dump, _, _))) => {
            let back = normal(&dump, &read_leaf, &mut Vec::new());
            let diff = Diff::between(&without_ids(&value), &without_ids(&back));
            match diff.differ.first() {
                None => Reencoding::Same,
                Some((_, path, detail)) => Reencoding::Differs(format!("{path}: {detail}")),
            }
        }
    };
    Ok(Decoded {
        value,
        duplicates,
        reencoding,
        leftover,
    })
}

/// Pallas's reading of the value a recorded change was made to.
#[derive(Debug, PartialEq)]
enum Base {
    Reads,
    Refuses(String),
    Unrecorded,
}

/// A refusal by pallas, with its reading of the value the change was made to.
#[derive(Debug, PartialEq)]
struct Refusal {
    error: String,
    base: Base,
}

/// The bytes of one record, the kind of value they are read as and the bytes
/// of the value the record's change was made to.
struct Encoded {
    kind: String,
    bytes: Vec<u8>,
    base: Option<Vec<u8>>,
}

/// Pallas's reading of one record, a refusal with its reading of the
/// record's base.
fn reading(input: &Encoded) -> Result<Decoded, Refusal> {
    let error = match decode(&input.kind, &input.bytes) {
        Ok(decoded) => return Ok(decoded),
        Err(error) => error,
    };
    let base = match &input.base {
        None => Base::Unrecorded,
        Some(bytes) => match catch(AssertUnwindSafe(|| reader(&input.kind)(bytes))) {
            Ok(Ok(_)) => Base::Reads,
            Ok(Err(e)) | Err(e) => Base::Refuses(e),
        },
    };
    Err(Refusal { error, base })
}

/// Whether pallas refuses for the reference's reason, which holds when it
/// reads the value the change was made to.
fn same_reason(_: &str, refusal: &Refusal) -> bool {
    refusal.base == Base::Reads
}

/// Pallas's decoding of Dijkstra values.
struct Decoding;

impl ComparedLayer for Decoding {
    const LAYER: &'static str = "decode";
    const TIME_LIMIT: Duration = Duration::from_secs(60);
    type Input = Encoded;
    type Axis = WholeInput;
    type Value = Decoded;
    type Refusal = Refusal;

    fn input(&self, row: &Row) -> Encoded {
        let key = Key {
            class: row.class.clone(),
            axis: WholeInput,
        };
        Encoded {
            kind: key.value_kind().to_owned(),
            bytes: row.input.clone(),
            base: row.base.clone(),
        }
    }

    fn reference(&self, row: &Row) -> Expected<WholeInput, Decoded> {
        let reference = match &row.reference {
            Reference::Accepts(accepted) => Reference::Accepts(Decoded::of_ledger(&accepted.dump)),
            Reference::Refuses(reason) => Reference::Refuses(reason.clone()),
        };
        Expected::one(WholeInput, reference)
    }

    fn run(&self, input: &Encoded) -> Vec<(WholeInput, Result<Decoded, Refusal>)> {
        vec![(WholeInput, reading(input))]
    }

    fn same_reason(&self, reason: &str, refusal: &Refusal) -> bool {
        same_reason(reason, refusal)
    }

    fn allowance(&self, _row: &Row, key: &Key<WholeInput>) -> Option<Allowance<Decoded, Refusal>> {
        differences::LIST
            .iter()
            .find(|(class, _)| *class == key.class)
            .map(|(_, a)| Allowance {
                allowed: a.allowed,
                shown: a.shown,
            })
    }

    fn detail(
        &self,
        row: &Row,
        reference: &Reference<Decoded>,
        answer: &Answer<Decoded, Refusal>,
    ) -> String {
        difference(row, reference, answer)
    }
}

/// The value without its transaction ids, which name the bytes rather than the
/// value.
fn without_ids(v: &Normal) -> Normal {
    match v {
        Normal::Record(m) => Normal::Record(
            m.iter()
                .filter(|(k, _)| k.as_str() != "id")
                .map(|(k, x)| (k.clone(), without_ids(x)))
                .collect(),
        ),
        Normal::List(xs) => Normal::List(xs.iter().map(without_ids).collect()),
        Normal::OrderedMap(xs) => Normal::OrderedMap(
            xs.iter()
                .map(|(k, x)| (k.clone(), without_ids(x)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// One line naming where pallas's answer for a row differs from the ledger's.
fn difference(
    row: &Row,
    reference: &Reference<Decoded>,
    answer: &Answer<Decoded, Refusal>,
) -> String {
    let mut at = format!("{} {}", row.class, row.variant.as_deref().unwrap_or(""));
    if let (Some(site), Some(rule)) = (&row.site, &row.mutated) {
        at.push_str(&format!(" ({rule} at {site})"));
    }
    match (reference, answer) {
        (Reference::Accepts(want), Answer::Accepts(got)) => {
            let diff = Diff::between(&want.value, &got.value);
            match diff.differ.first() {
                Some((class, path, detail)) => format!("{at}: {class} at {path}: {detail}"),
                None => format!(
                    "{at}: duplicates {:?}, reencoding {:?}, {} bytes after it",
                    got.duplicates, got.reencoding, got.leftover
                ),
            }
        }
        (_, Answer::Refuses(Refusal { error, base })) => {
            format!("{at}: pallas refuses: {error}, its base: {base:?}")
        }
        (_, Answer::Panics(m)) => format!("{at}: pallas panics: {m}"),
        (Reference::Refuses(r), Answer::Accepts(_)) => {
            let r: String = r.chars().take(160).collect();
            format!("{at}: the ledger refuses ({r}) and pallas accepts")
        }
    }
}

/// The 40 digit commit a vendored CDDL file names on its first line.
fn cddl_pin(cddl: &str) -> Option<&str> {
    let first = cddl.lines().next()?;
    first
        .split(|c: char| !c.is_ascii_hexdigit())
        .find(|word| word.len() == 40)
}

/// The rule names of a CDDL text, generic rules by the name before their
/// parameters.
fn rule_names(cddl: &str) -> Vec<String> {
    cddl.lines()
        .filter_map(|line| {
            let name: String = line
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            let rest = line[name.len()..].trim_start();
            let rest = match rest.strip_prefix('<') {
                Some(r) => r.split_once('>')?.1.trim_start(),
                None => rest,
            };
            (!name.is_empty() && rest.starts_with('=')).then_some(name)
        })
        .collect()
}

/// The failures of a rule credit, against the rules `defs.cddl` defines and the
/// ones listed as uncredited, in a run of every kind.
fn credit_failures(
    rules: &[String],
    credit: &BTreeMap<String, usize>,
    uncredited: &[(&str, &str)],
    kinds: &[String],
) -> Vec<String> {
    let mut failures = Vec::new();
    if !kinds.is_empty() {
        return failures;
    }
    for rule in rules {
        let credited = credit.get(rule).is_some_and(|n| *n > 0);
        let listed = uncredited.iter().any(|(name, _)| name == rule);
        match (credited, listed) {
            (false, false) => failures.push(format!("no value credits {rule}")),
            (true, true) => failures.push(format!("{rule} is credited, so it leaves UNCREDITED")),
            _ => {}
        }
    }
    for name in credit.keys().filter(|n| !rules.contains(n)) {
        failures.push(format!(
            "a value credits {name}, which defs.cddl does not define"
        ));
    }
    for (name, _) in uncredited
        .iter()
        .filter(|(n, _)| !rules.iter().any(|r| r == n))
    {
        failures.push(format!(
            "UNCREDITED names {name}, which defs.cddl does not define"
        ));
    }
    failures
}

fn run() {
    let cddl = include_str!("../../src/dijkstra/defs.cddl");
    let pin = cddl_pin(cddl).expect("defs.cddl names no ledger commit");
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../test_data/ledger-records/decode.jsonl");
    let records = Records::read_file(&path, pin).unwrap_or_else(|e| panic!("{e}"));
    let kinds = kinds_from_env();
    let judged = reference_comparison::compare(&Arc::new(Decoding), &records, &kinds);
    let mut examples = BTreeMap::new();
    let mut credit = BTreeMap::new();
    for j in &judged.judgements {
        if j.outcome == Outcome::Agree {
            for rule in &j.row.rules {
                *credit.entry(rule.clone()).or_insert(0) += 1;
            }
        } else {
            examples
                .entry((j.row.class.clone(), j.row.mutated.clone()))
                .or_insert(&j.detail);
        }
    }
    let rules = rule_names(cddl);
    let counts: Vec<String> = rules
        .iter()
        .map(|r| format!("{r}={}", credit.get(r).copied().unwrap_or(0)))
        .collect();
    let coverage = Coverage::new(Decoding::LAYER, records.targets(), records.rows());
    println!("records: {}", records.generator());
    println!("{judged}");
    print!("{coverage}");
    println!("rules {}: {}", rules.len(), counts.join(" "));
    println!(
        "rules credited {} of {}{}",
        rules.iter().filter(|r| credit.contains_key(*r)).count(),
        rules.len(),
        if kinds.is_empty() {
            ""
        } else {
            ", not checked in a run of the given kinds only"
        }
    );
    examples.values().for_each(|e| println!("{e}"));
    let mut failures = judged.failures();
    failures.extend(coverage.failures());
    failures.extend(credit_failures(&rules, &credit, UNCREDITED, &kinds));
    let listed: Vec<(&str, Allowed)> = differences::LIST
        .iter()
        .map(|(class, a)| (*class, a.allowed))
        .collect();
    failures.extend(judged.tally.unused(&listed, records.classes()));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn pallas_decodes_what_the_ledger_writes() {
    let outcome = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(run)
        .expect("spawn")
        .join();
    if let Err(e) = outcome {
        std::panic::resume_unwind(e);
    }
}

fn refused_change(input: &[u8], base: Option<&[u8]>) -> Outcome {
    let input = Encoded {
        kind: "wits".into(),
        bytes: input.to_vec(),
        base: base.map(<[u8]>::to_vec),
    };
    let answer = match reading(&input) {
        Ok(decoded) => Answer::Accepts(decoded),
        Err(refusal) => Answer::Refuses(refusal),
    };
    judge(
        &Reference::Refuses("unknown tag".into()),
        &answer,
        same_reason,
        None,
    )
}

#[test]
fn the_bytes_after_a_value_are_counted() {
    let leftover = |hex: &str| {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        decode("wits", &bytes).map(|d| d.leftover)
    };
    assert_eq!(leftover("a0"), Ok(0));
    assert_eq!(leftover("a000"), Ok(1));
    assert_eq!(leftover("a0a0ff"), Ok(2));
}

#[test]
fn a_refused_change_to_a_value_pallas_reads_agrees() {
    assert_eq!(refused_change(&[0xff], Some(&[0xa0])), Outcome::Agree);
}

#[test]
fn a_refused_change_to_a_value_pallas_refuses_disagrees() {
    assert_eq!(
        refused_change(&[0xff], Some(&[0xff])),
        Outcome::Disagree(Disagreement::OtherReason)
    );
    assert_eq!(
        refused_change(&[0xff], None),
        Outcome::Disagree(Disagreement::OtherReason)
    );
}

#[test]
fn rules_credited_and_listed_as_defs_names_them_pass() {
    let rules = ["coin", "eb_reference", "language"].map(String::from);
    let credit = BTreeMap::from([("coin".to_string(), 2)]);
    let uncredited = [("eb_reference", "a reason"), ("language", "a reason")];
    assert_eq!(
        credit_failures(&rules, &credit, &uncredited, &[]),
        Vec::<String>::new()
    );
}

#[test]
fn rule_credit_is_judged_only_in_a_run_of_every_kind() {
    let rules = ["coin"].map(String::from);
    let credit = BTreeMap::new();
    assert_eq!(
        credit_failures(&rules, &credit, &[], &[]),
        ["no value credits coin"]
    );
    assert_eq!(
        credit_failures(&rules, &credit, &[], &["block".to_string()]),
        Vec::<String>::new()
    );
}

#[test]
fn rules_credited_or_listed_against_defs_fail() {
    let rules = ["coin", "eb_reference", "language"].map(String::from);
    let credit = BTreeMap::from([("eb_reference".to_string(), 1), ("nonce".to_string(), 1)]);
    let uncredited = [("eb_reference", "a reason"), ("signkey_kes", "a reason")];
    assert_eq!(
        credit_failures(&rules, &credit, &uncredited, &[]),
        [
            "no value credits coin",
            "eb_reference is credited, so it leaves UNCREDITED",
            "no value credits language",
            "a value credits nonce, which defs.cddl does not define",
            "UNCREDITED names signkey_kes, which defs.cddl does not define",
        ]
    );
}

#[test]
fn the_cddl_pin_is_the_first_line_commit() {
    let cddl = "; Copied from cardano-ledger commit 9df9f98cce86c5f2195142b2c02dd9b39dd9dc7c,\n";
    assert_eq!(
        cddl_pin(cddl),
        Some("9df9f98cce86c5f2195142b2c02dd9b39dd9dc7c")
    );
    assert_eq!(cddl_pin("; no commit here\n"), None);
}

#[test]
fn rule_names_are_read_from_definitions() {
    let cddl = "; a comment\nblock = [header]\nnonempty_set<a0> = #6.258([+ a0])\n  foo = 1\nhash32 = bytes .size 32\n";
    assert_eq!(rule_names(cddl), ["block", "nonempty_set", "hash32"]);
}
