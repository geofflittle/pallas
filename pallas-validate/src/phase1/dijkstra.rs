//! Native protocol-12 phase one for transfers, batches, account and governance state, and Plutus V1 to V3.
use super::dijkstra_ledger::{self as ledger, Flows, Level, WithdrawalRule};
use crate::utils::{
    ContextFailure, DIJKSTRA_MAJOR_VERSION, DijkstraProtParams, MalformedField, ParameterFailure,
    PostAlonzoError,
    PostAlonzoError::*,
    UTxOs, UnsupportedFeature, ValidationError,
    ValidationError::*,
    ValidationResult, WithdrawalFailure,
    dijkstra_context::check_translatable,
    dijkstra_state::{DijkstraLedgerDelta, DijkstraLedgerOverlay, DijkstraLedgerState},
};
use pallas_addresses::{
    Address, Network, ShelleyDelegationPart, ShelleyPaymentPart, byron::ByronAddress,
};
use pallas_codec::{
    minicbor,
    utils::{KeepRaw, Nullable},
};
use pallas_crypto::{
    hash::{Hash, Hasher},
    key::ed25519::{PublicKey, Signature},
};
use pallas_primitives::{conway::LanguageViews, dijkstra::*};
use pallas_traverse::{
    ComputeHash, MultiEraInput, MultiEraNativeScript, MultiEraOutput, MultiEraScriptRef,
    OriginalHash,
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashSet},
};

type Keys = HashSet<Hash<28>>;
const MALFORMED_VALUE: ValidationError = DijkstraMalformed(MalformedField::MultiAssetValue);
const MALFORMED_METADATA: ValidationError = DijkstraMalformed(MalformedField::Metadata);
type Spent = HashSet<(Hash<32>, u64)>;

/// Validates native phase one against `state` and returns the changes the transaction makes to it.
pub fn validate_dijkstra_tx<S: DijkstraLedgerState + ?Sized>(
    tx: &BlockTransaction<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    block_slot: &u64,
    network_id: &u8,
    state: &S,
) -> Result<DijkstraLedgerDelta, ValidationError> {
    validate_dijkstra_tx_inner(tx, utxos, pp, block_slot, network_id, state, None)
}

/// Native phase one with the historical time context required by bounded Plutus.
pub fn validate_dijkstra_tx_with_time<S: DijkstraLedgerState + ?Sized>(
    tx: &BlockTransaction<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    block_slot: &u64,
    network_id: &u8,
    state: &S,
    time: &crate::utils::DijkstraTimeContext,
) -> Result<DijkstraLedgerDelta, ValidationError> {
    validate_dijkstra_tx_inner(tx, utxos, pp, block_slot, network_id, state, Some(time))
}

pub(super) fn validate_dijkstra_tx_inner<S: DijkstraLedgerState + ?Sized>(
    tx: &BlockTransaction<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    block_slot: &u64,
    network_id: &u8,
    state: &S,
    time: Option<&crate::utils::DijkstraTimeContext>,
) -> Result<DijkstraLedgerDelta, ValidationError> {
    if pp.protocol_version.0 != DIJKSTRA_MAJOR_VERSION {
        return Err(TxAndProtParamsDiffer);
    }
    check_supported(tx)?;
    let b = &tx.transaction_body;
    for sub in b.sub_transactions.iter().flatten() {
        check_sub_supported(sub)?;
    }
    check_auxiliary_hash(b.auxiliary_data_hash, &tx.auxiliary_data)?;
    let guards = guard_credentials(b.guards.as_ref());
    let mut keys = guard_keys(&guards);
    let top_guards: BTreeSet<_> = guards.iter().cloned().collect();
    check_required_guards(b.required_top_level_guards.as_ref(), &top_guards)?;
    let level = Level::top(b, Hasher::<256>::hash(b.raw_cbor()));
    keys.extend(ledger::needs(&level)?.keys);
    let scripts = ledger::redeemer_scripts(&level, |x| spent(x, utxos))?;
    check_interval(
        level.validity_interval_start,
        level.ttl,
        b.network_id,
        *block_slot,
        *network_id,
    )?;
    if tx_size(tx)? > u64::from(pp.max_transaction_size) {
        return Err(PostAlonzo(MaxTxSizeExceeded));
    }
    let subs = sub_scripts(b, utxos)?;
    let provided = check_scripts(tx, utxos, pp, &scripts, &subs, &mut keys, *network_id, time)?;
    check_guard_datums(b.required_top_level_guards.as_ref(), &provided)?;
    let rule = withdrawal_rule(&provided.languages);
    let legacy = rule == WithdrawalRule::Legacy;
    let (flows, top_flows, delta) = ledger_rules(tx, pp, *network_id, rule, state)?;
    if b.fee < dijkstra_min_fee(tx, utxos, pp)? {
        return Err(PostAlonzo(FeeBelowMin));
    }
    let mut spent = Spent::new();
    let mut consumed = flows.consumed;
    let mut output_coin = 0;
    for (index, sub) in (0..).zip(b.sub_transactions.iter().flat_map(|x| x.iter())) {
        let sb = &sub.sub_transaction_body;
        let w = &sub.transaction_witness_set;
        let level = Level::sub(sb, Hasher::<256>::hash(sb.raw_cbor()), index);
        check_auxiliary_hash(sb.auxiliary_data_hash, &sub.auxiliary_data)?;
        check_required_guards(sb.required_top_level_guards.as_ref(), &top_guards)?;
        check_guard_datums(sb.required_top_level_guards.as_ref(), &provided)?;
        check_interval(
            level.validity_interval_start,
            level.ttl,
            sb.network_id,
            *block_slot,
            *network_id,
        )?;
        let mut sub_keys = guard_keys(&guard_credentials(sb.guards.as_ref()));
        sub_keys.extend(ledger::needs(&level)?.keys);
        let spending = check_inputs(&sb.inputs, utxos, &mut spent, &mut sub_keys)?;
        consumed = add(consumed, spending)?;
        let scripts = &subs.levels[index as usize];
        let languages = check_level(
            &level,
            w,
            sb.script_data_hash,
            scripts,
            &provided,
            HashSet::new(),
            utxos,
            pp,
            time,
        )?;
        check_contexts(&level, &languages, utxos)?;
        output_coin = add(output_coin, check_outputs(&sb.outputs, pp, network_id)?)?;
        check_signatures(w, &encode(&sub.sub_transaction_body)?, sub_keys)?;
    }
    let spending = check_inputs(&b.inputs, utxos, &mut spent, &mut keys)?;
    consumed = add(consumed, spending)?;
    let top_output_coin = check_outputs(&b.outputs, pp, network_id)?;
    output_coin = add(output_coin, top_output_coin)?;
    if add(add(output_coin, b.fee)?, flows.produced)? != consumed {
        return Err(PostAlonzo(PreservationOfValue));
    }
    let sub_bodies = || {
        b.sub_transactions
            .iter()
            .flatten()
            .map(|s| &s.sub_transaction_body)
    };
    check_assets(
        b.inputs
            .iter()
            .chain(sub_bodies().flat_map(|s| s.inputs.iter())),
        b.outputs
            .iter()
            .chain(sub_bodies().flat_map(|s| s.outputs.iter())),
        b.mint
            .iter()
            .chain(sub_bodies().flat_map(|s| s.mint.iter())),
        utxos,
    )?;
    // In legacy mode the top level balances on its own, with deposits judged after the sub transactions.
    if legacy {
        if add(add(top_output_coin, b.fee)?, top_flows.produced)?
            != add(spending, top_flows.consumed)?
        {
            return Err(PostAlonzo(PreservationOfValue));
        }
        check_assets(b.inputs.iter(), b.outputs.iter(), b.mint.iter(), utxos)?;
    }
    check_signatures(&tx.transaction_witness_set, &encode(b)?, keys)?;
    check_contexts(&level, &provided.languages, utxos)?;
    Ok(delta)
}

fn check_contexts(
    b: &Level<'_>,
    languages: &BTreeSet<Language>,
    utxos: &UTxOs<'_>,
) -> ValidationResult {
    let inputs = resolve(b.inputs.iter(), utxos, InputNotInUTxO)?;
    let references = resolve(
        b.reference_inputs.into_iter().flatten(),
        utxos,
        ReferenceInputNotInUTxO,
    )?;
    for language in languages {
        check_translatable(language, b, &inputs, &references)
            .map_err(|x| DijkstraScriptContext(x.0))?;
    }
    Ok(())
}

fn resolve<'a, 'b>(
    inputs: impl Iterator<Item = &'a TransactionInput>,
    utxos: &'a UTxOs<'b>,
    missing: PostAlonzoError,
) -> Result<Vec<&'a MultiEraOutput<'b>>, ValidationError> {
    inputs
        .map(|x| {
            utxos
                .get(&MultiEraInput::from_alonzo_compatible(x))
                .ok_or(PostAlonzo(missing.clone()))
        })
        .collect()
}

fn spent<'a, 'b>(
    input: &TransactionInput,
    utxos: &'a UTxOs<'b>,
) -> Result<&'a MultiEraOutput<'b>, ValidationError> {
    utxos
        .get(&MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(
            input.clone(),
        ))))
        .ok_or(PostAlonzo(InputNotInUTxO))
}

/// The ledger state changes of a transaction the ledger has accepted, computed against `state`.
pub fn dijkstra_ledger_effects<S: DijkstraLedgerState + ?Sized>(
    tx: &BlockTransaction<'_>,
    pp: &DijkstraProtParams,
    network_id: u8,
    state: &S,
) -> Result<DijkstraLedgerDelta, ValidationError> {
    if !tx.success {
        return Ok(DijkstraLedgerDelta::default());
    }
    Ok(ledger_rules(tx, pp, network_id, WithdrawalRule::Accepted, state)?.2)
}

fn ledger_rules<S: DijkstraLedgerState + ?Sized>(
    tx: &BlockTransaction<'_>,
    pp: &DijkstraProtParams,
    network: u8,
    rule: WithdrawalRule,
    state: &S,
) -> Result<(Flows, Flows, DijkstraLedgerDelta), ValidationError> {
    let b = &tx.transaction_body;
    let mut current = DijkstraLedgerOverlay::new(state);
    let mut flows = Flows::default();
    for (index, sub) in (0..).zip(b.sub_transactions.iter().flatten()) {
        let sb = &sub.sub_transaction_body;
        let level = Level::sub(sb, Hasher::<256>::hash(sb.raw_cbor()), index);
        ledger::check_treasury(&level, state)?;
        flows.add(ledger::entities(
            &level,
            None,
            pp,
            network,
            state,
            &mut current,
        )?)?;
        flows.add(ledger::governance(&level, pp, network, &mut current)?)?;
    }
    let level = Level::top(b, Hasher::<256>::hash(b.raw_cbor()));
    ledger::check_treasury(&level, state)?;
    let subs = ledger::sub_withdrawals(tx)?;
    let mut top = ledger::entities(
        &level,
        Some((rule, &subs)),
        pp,
        network,
        state,
        &mut current,
    )?;
    top.add(ledger::governance(&level, pp, network, &mut current)?)?;
    flows.add(top)?;
    Ok((flows, top, current.delta))
}

/// The minimum fee of a transaction whose inputs resolve in `utxos`, pricing the redeemers of its top level and sub transactions together.
pub fn dijkstra_min_fee(
    tx: &BlockTransaction<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
) -> Result<u64, ValidationError> {
    let size_fee = u64::from(pp.minfee_a)
        .checked_mul(tx_size(tx)?)
        .and_then(|x| x.checked_add(u64::from(pp.minfee_b)))
        .ok_or(PostAlonzo(NegativeValue))?;
    let (mem, steps) = declared_units(tx)?;
    let fee = add(size_fee, execution_fee(mem, steps, pp)?)?;
    add(
        fee,
        reference_fee(reference_script_size(&tx.transaction_body, utxos)?, pp)?,
    )
}

fn tx_size(tx: &BlockTransaction<'_>) -> Result<u64, ValidationError> {
    Ok(encode(tx.to_mempool_transaction())?.len() as u64)
}

fn declared_units(tx: &BlockTransaction<'_>) -> Result<(u64, u64), ValidationError> {
    let subs = tx
        .transaction_body
        .sub_transactions
        .iter()
        .flat_map(|x| x.iter())
        .map(|sub| &sub.transaction_witness_set);
    let mut units = (0, 0);
    for (_, value) in std::iter::once(&tx.transaction_witness_set)
        .chain(subs)
        .flat_map(|w| w.redeemer.iter())
        .flat_map(|x| x.iter())
    {
        units = (
            add(units.0, value.ex_units.mem)?,
            add(units.1, value.ex_units.steps)?,
        );
    }
    Ok(units)
}

fn reference_script_size(
    b: &TransactionBody<'_>,
    utxos: &UTxOs<'_>,
) -> Result<u64, ValidationError> {
    let inputs: BTreeSet<_> = b
        .reference_inputs
        .iter()
        .flat_map(|x| x.iter())
        .chain(b.inputs.iter())
        .collect();
    let mut bytes = 0;
    for input in inputs {
        let script = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .and_then(|output| output.multi_era_script_ref());
        let size = match &script {
            None => 0,
            Some(script) => match script.native_script() {
                Some(native) => native.encode().len(),
                None => script.plutus_bytes().map_or(0, <[u8]>::len),
            },
        };
        bytes = add(bytes, size as u64)?;
    }
    Ok(bytes)
}

fn encode<T: minicbor::Encode<()>>(value: T) -> Result<Vec<u8>, ValidationError> {
    minicbor::to_vec(value).map_err(|_| PostAlonzo(UnknownTxSize))
}
fn add(a: u64, b: u64) -> Result<u64, ValidationError> {
    a.checked_add(b).ok_or(PostAlonzo(NegativeValue))
}
fn check_interval(
    lo: Option<u64>,
    hi: Option<u64>,
    net: Option<NetworkId>,
    slot: u64,
    network: u8,
) -> ValidationResult {
    if lo.is_some_and(|x| slot < x) {
        return Err(PostAlonzo(BlockPrecedesValInt));
    }
    // Allegra inInterval inherited by Dijkstra: upper bound is exclusive.
    if hi.is_some_and(|x| slot >= x) {
        return Err(PostAlonzo(BlockExceedsValInt));
    }
    if net.is_some_and(|x| u8::from(x) != network) {
        return Err(PostAlonzo(TxWrongNetworkID));
    }
    Ok(())
}
fn guard_credentials(guards: Option<&Guards>) -> Vec<StakeCredential> {
    match guards {
        None => vec![],
        Some(Guards::AddrKeyhashes(xs)) => xs
            .iter()
            .map(|x| StakeCredential::AddrKeyhash(*x))
            .collect(),
        Some(Guards::Credentials(xs)) => xs.to_vec(),
    }
}
fn guard_keys(guards: &[StakeCredential]) -> Keys {
    guards
        .iter()
        .filter_map(|x| match x {
            StakeCredential::AddrKeyhash(key) => Some(*key),
            StakeCredential::ScriptHash(_) => None,
        })
        .collect()
}
fn check_required_guards(
    required: Option<&RequiredTopLevelGuards>,
    top: &BTreeSet<StakeCredential>,
) -> ValidationResult {
    for credential in required.into_iter().flat_map(|x| x.keys()) {
        if !top.contains(credential) {
            return Err(PostAlonzo(ReqSignerMissing));
        }
    }
    Ok(())
}
fn check_guard_datums(
    required: Option<&RequiredTopLevelGuards>,
    provided: &Provided,
) -> ValidationResult {
    for (credential, datum) in required.into_iter().flatten() {
        let takes = match credential {
            StakeCredential::AddrKeyhash(_) => Some(false),
            StakeCredential::ScriptHash(x) if provided.natives.contains_key(x) => Some(false),
            StakeCredential::ScriptHash(x) => provided.plutus.get(x).map(|_| true),
        };
        if takes.is_some_and(|x| x == matches!(datum, Nullable::Null)) {
            return Err(DijkstraMalformedGuardDatum);
        }
    }
    Ok(())
}
fn check_inputs(
    inputs: &[TransactionInput],
    utxos: &UTxOs<'_>,
    spent: &mut Spent,
    keys: &mut Keys,
) -> Result<u64, ValidationError> {
    if inputs.is_empty() {
        return Err(PostAlonzo(TxInsEmpty));
    }
    let mut coin = 0;
    for input in inputs {
        let id = (input.transaction_id, input.index);
        // SUBUTXO requires membership in both original and evolving UTxO.
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(InputNotInUTxO))?;
        if !spent.insert(id) {
            return Err(DijkstraInputAlreadySpent);
        }
        coin = add(coin, output.value().coin())?;
        if let Spender::Key(key) = spender(output)? {
            keys.insert(key);
        }
    }
    Ok(coin)
}
pub(crate) enum Spender {
    Key(Hash<28>),
    Script(Hash<28>),
}
pub(crate) fn spender(output: &MultiEraOutput<'_>) -> Result<Spender, ValidationError> {
    match output.address().map_err(|_| PostAlonzo(AddressDecoding))? {
        Address::Shelley(address) => Ok(match address.payment() {
            ShelleyPaymentPart::Key(key) => Spender::Key(*key),
            ShelleyPaymentPart::Script(hash) => Spender::Script(*hash),
        }),
        Address::Byron(address) => Ok(Spender::Key(
            byron_root(&address).map_err(|_| PostAlonzo(InputDecoding))?,
        )),
        Address::Stake(_) => Err(PostAlonzo(InputDecoding)),
    }
}
fn byron_root(address: &ByronAddress) -> Result<Hash<28>, minicbor::decode::Error> {
    let mut d = minicbor::Decoder::new(&address.payload.0);
    d.array()?;
    d.decode()
}
const BYRON_CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
struct ByronAttributes {
    network_magic: Option<u32>,
    size: usize,
}
impl ByronAttributes {
    fn of_address(bytes: &[u8]) -> Result<Self, minicbor::decode::Error> {
        if bytes.first() != Some(&0x82) {
            return Err(minicbor::decode::Error::message("not a Byron address"));
        }
        whole(bytes, |d| {
            d.array()?;
            if d.tag()? != minicbor::data::Tag::new(24) {
                return Err(minicbor::decode::Error::message("payload tag is not 24"));
            }
            let payload = d.bytes()?;
            if d.u32()? != BYRON_CRC.checksum(payload) {
                return Err(minicbor::decode::Error::message("checksum mismatch"));
            }
            whole(payload, |d| {
                if d.array()? != Some(3) {
                    return Err(minicbor::decode::Error::message("payload is not a triple"));
                }
                if d.bytes()?.len() != 28 {
                    return Err(minicbor::decode::Error::message("root is not 28 bytes"));
                }
                let attributes = d.decode()?;
                if !matches!(canonical_u32(d)?, 0 | 2) {
                    return Err(minicbor::decode::Error::message("unknown address type"));
                }
                Ok(attributes)
            })
        })
    }
    fn network(&self) -> u8 {
        match self.network_magic {
            Some(_) => Network::Testnet.value(),
            None => Network::Mainnet.value(),
        }
    }
}
impl<'b, C> minicbor::Decode<'b, C> for ByronAttributes {
    fn decode(d: &mut minicbor::Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let entries = d
            .map()?
            .ok_or_else(|| minicbor::decode::Error::message("indefinite attributes map"))?;
        let mut attributes = Self {
            network_magic: None,
            size: 0,
        };
        let mut last = None;
        for _ in 0..entries {
            let key = d.u8()?;
            if last.is_some_and(|last| last >= key) {
                return Err(minicbor::decode::Error::message(
                    "attribute keys do not increase",
                ));
            }
            last = Some(key);
            let value = d.bytes()?;
            match key {
                1 => attributes.size += whole(value, canonical_bytes)?.len(),
                2 => attributes.network_magic = Some(whole(value, canonical_u32)?),
                _ => attributes.size += value.len(),
            }
        }
        Ok(attributes)
    }
}
fn whole<'b, T>(
    bytes: &'b [u8],
    decode: impl FnOnce(&mut minicbor::Decoder<'b>) -> Result<T, minicbor::decode::Error>,
) -> Result<T, minicbor::decode::Error> {
    let mut d = minicbor::Decoder::new(bytes);
    let value = decode(&mut d)?;
    if d.position() != bytes.len() {
        return Err(minicbor::decode::Error::message("trailing bytes"));
    }
    Ok(value)
}
fn head_len(n: u64) -> usize {
    match n {
        0..=23 => 1,
        24..=0xff => 2,
        0x100..=0xffff => 3,
        0x1_0000..=0xffff_ffff => 5,
        _ => 9,
    }
}
fn canonical_u32(d: &mut minicbor::Decoder<'_>) -> Result<u32, minicbor::decode::Error> {
    let start = d.position();
    let value = d.u32()?;
    if d.position() - start != head_len(value.into()) {
        return Err(minicbor::decode::Error::message(
            "integer head is not shortest",
        ));
    }
    Ok(value)
}
fn canonical_bytes<'b>(d: &mut minicbor::Decoder<'b>) -> Result<&'b [u8], minicbor::decode::Error> {
    let start = d.position();
    let value = d.bytes()?;
    if d.position() - start - value.len() != head_len(value.len() as u64) {
        return Err(minicbor::decode::Error::message(
            "byte string head is not shortest",
        ));
    }
    Ok(value)
}
fn verifies(vkey: &[u8], signature: &[u8], message: &[u8]) -> bool {
    match (PublicKey::try_from(vkey), Signature::try_from(signature)) {
        (Ok(key), Ok(signature)) => key.verify(message, &signature),
        _ => false,
    }
}
fn check_signatures(w: &WitnessSet<'_>, body: &[u8], mut required: Keys) -> ValidationResult {
    let hash = Hasher::<256>::hash(body);
    for witness in w.vkeywitness.iter().flat_map(|x| x.iter()) {
        if !verifies(&witness.vkey, &witness.signature, hash.as_ref()) {
            return Err(PostAlonzo(VKWrongSignature));
        }
        required.remove(&Hasher::<224>::hash(&witness.vkey));
    }
    for witness in w.bootstrap_witness.iter().flat_map(|x| x.iter()) {
        if !verifies(&witness.public_key, &witness.signature, hash.as_ref()) {
            return Err(PostAlonzo(VKWrongSignature));
        }
        required.remove(&super::conway::bootstrap_wit_root(witness));
    }
    if !required.is_empty() {
        return Err(PostAlonzo(VKWitnessMissing));
    }
    Ok(())
}
fn witness_keys(w: &WitnessSet<'_>) -> Keys {
    w.vkeywitness
        .iter()
        .flat_map(|x| x.iter())
        .map(|w| Hasher::<224>::hash(&w.vkey))
        .collect()
}
fn output_address(output: &TransactionOutput<'_>) -> Result<(u8, usize), ValidationError> {
    let bytes = match output {
        TransactionOutput::Legacy(x) => &x.address,
        TransactionOutput::PostAlonzo(x) => &x.address,
    };
    match Address::from_bytes(bytes).map_err(|_| PostAlonzo(AddressDecoding))? {
        Address::Shelley(address) => Ok((address.network().value(), 0)),
        Address::Byron(_) => {
            let attributes =
                ByronAttributes::of_address(bytes).map_err(|_| PostAlonzo(AddressDecoding))?;
            Ok((attributes.network(), attributes.size))
        }
        Address::Stake(_) => Err(PostAlonzo(AddressDecoding)),
    }
}
fn check_output_encodings<'a, 'b: 'a>(
    outputs: impl Iterator<Item = &'a TransactionOutput<'b>>,
) -> ValidationResult {
    for output in outputs {
        let raw = encode(output)?;
        match output {
            TransactionOutput::PostAlonzo(_) => {
                check_map_keys(&raw, &[0, 1, 2, 3], MalformedField::Output)?;
            }
            TransactionOutput::Legacy(_) => {
                let length = minicbor::Decoder::new(&raw)
                    .array()
                    .map_err(|_| DijkstraMalformed(MalformedField::Output))?;
                if !matches!(length, Some(2 | 3)) {
                    return Err(DijkstraMalformed(MalformedField::Output));
                }
            }
        }
        output_address(output)?;
        check_output_value_encoding(&raw)?;
    }
    Ok(())
}
fn check_outputs(
    outputs: &[TransactionOutput<'_>],
    pp: &DijkstraProtParams,
    network_id: &u8,
) -> Result<u64, ValidationError> {
    let mut total = 0;
    for output in outputs {
        let view = MultiEraOutput::from_dijkstra(output);
        let coin = view.value().coin();
        if let Some(script) = view.multi_era_script_ref() {
            check_new_script(&script)?;
        }
        let (network, attributes_size) = output_address(output)?;
        if attributes_size > 64 {
            return Err(DijkstraBootstrapAttributesTooLarge);
        }
        if network != *network_id {
            return Err(PostAlonzo(OutputWrongNetworkID));
        }
        // Sized TxOut counts the original serialization, not just the value.
        let output_bytes = minicbor::to_vec(output).map_err(|_| PostAlonzo(UnknownTxSize))?;
        let minimum = pp
            .ada_per_utxo_byte
            .checked_mul(160 + output_bytes.len() as u64)
            .ok_or(PostAlonzo(NegativeValue))?;
        if coin < minimum {
            return Err(PostAlonzo(MinLovelaceUnreached));
        }
        let value_bytes =
            minicbor::to_vec(view.value().into_alonzo()).map_err(|_| PostAlonzo(UnknownTxSize))?;
        if value_bytes.len() > pp.max_value_size as usize {
            return Err(PostAlonzo(MaxValSizeExceeded));
        }
        total = add(total, coin)?;
    }
    Ok(total)
}
fn check_supported(tx: &BlockTransaction<'_>) -> ValidationResult {
    let b = &tx.transaction_body;
    if !tx.success {
        return Err(DijkstraUnsupported(
            UnsupportedFeature::UnsuccessfulBlockTransaction,
        ));
    }
    let raw = encode(b)?;
    let mut allowed = vec![0, 1, 2, 3, 8, 15];
    for (key, present) in [
        (4, b.certificates.is_some()),
        (5, b.withdrawals.is_some()),
        (7, b.auxiliary_data_hash.is_some()),
        (9, b.mint.is_some()),
        (11, b.script_data_hash.is_some()),
        (13, b.collateral.is_some()),
        (14, holds_value(&raw, 14)),
        (16, b.collateral_return.is_some()),
        (17, b.total_collateral.is_some()),
        (18, b.reference_inputs.is_some()),
        (19, b.voting_procedures.is_some()),
        (20, b.proposal_procedures.is_some()),
        (21, b.treasury_value.is_some()),
        (22, b.donation.is_some()),
        (23, b.sub_transactions.is_some()),
        (24, b.required_top_level_guards.is_some()),
        (25, b.direct_deposits.is_some()),
        (26, b.account_balance_intervals.is_some()),
        (27, b.starting_account_balance_intervals.is_some()),
    ] {
        if present {
            allowed.push(key);
        }
    }
    check_map_keys(&raw, &allowed, MalformedField::Body)?;
    let fields: pallas_codec::utils::KeyValuePairs<u64, pallas_codec::utils::AnyCbor> =
        minicbor::decode(&raw).map_err(|_| DijkstraMalformed(MalformedField::Body))?;
    for (key, value) in fields.iter() {
        check_field_present(*key, value)?;
        if *key == 9 {
            check_asset_encoding(value)?;
        }
        if *key == 24 {
            check_required_guard_keys(value)?;
        }
        if *key == 5 {
            check_withdrawal_accounts(value)?;
        }
    }
    check_distinct(b.inputs.iter(), MalformedField::DuplicateInputs)?;
    check_distinct(
        b.reference_inputs.iter().flat_map(|x| x.iter()),
        MalformedField::DuplicateReferenceInputs,
    )?;
    check_distinct(
        b.collateral.iter().flat_map(|x| x.iter()),
        MalformedField::DuplicateCollateralInputs,
    )?;
    check_output_encodings(b.outputs.iter().chain(b.collateral_return.iter()))?;
    check_guards(b.guards.as_ref())?;
    check_witness_fields(&tx.transaction_witness_set)?;
    check_auxiliary_encoding(&tx.auxiliary_data)
}
fn check_distinct<'a>(
    inputs: impl IntoIterator<Item = &'a TransactionInput>,
    field: MalformedField,
) -> ValidationResult {
    let mut seen = Spent::new();
    match inputs
        .into_iter()
        .all(|x| seen.insert((x.transaction_id, x.index)))
    {
        true => Ok(()),
        false => Err(DijkstraMalformed(field)),
    }
}
fn check_guards(guards: Option<&Guards>) -> ValidationResult {
    let guards = guard_credentials(guards);
    match guards.iter().collect::<BTreeSet<_>>().len() == guards.len() {
        true => Ok(()),
        false => Err(DijkstraMalformed(MalformedField::DuplicateGuards)),
    }
}
fn check_witness_fields(w: &KeepRaw<'_, WitnessSet<'_>>) -> ValidationResult {
    let mut keys = HashSet::new();
    for x in w.vkeywitness.iter().flat_map(|x| x.iter()) {
        if x.vkey.len() != 32 || x.signature.len() != 64 {
            return Err(DijkstraMalformed(MalformedField::KeyWitness));
        }
        if !keys.insert((x.vkey.to_vec(), x.signature.to_vec())) {
            return Err(DijkstraMalformed(MalformedField::DuplicateKeyWitnesses));
        }
    }
    let mut roots = HashSet::new();
    for x in w.bootstrap_witness.iter().flat_map(|x| x.iter()) {
        if x.public_key.len() != 32 || x.signature.len() != 64 || x.chain_code.len() != 32 {
            return Err(DijkstraMalformed(MalformedField::BootstrapWitness));
        }
        if !roots.insert(super::conway::bootstrap_wit_root(x)) {
            return Err(DijkstraMalformed(
                MalformedField::DuplicateBootstrapWitnesses,
            ));
        }
    }
    let datums = w.plutus_data.iter().flat_map(|x| x.iter());
    if repeats(datums.map(|x| x.raw_cbor())) {
        return Err(DijkstraMalformed(MalformedField::DuplicateDatums));
    }
    // The ledger names a script by its hash, over the bytes a native script was
    // sent in and over a Plutus script's bytes under its language.
    let native = w.native_script.iter().flat_map(|x| x.iter());
    let v1 = w.plutus_v1_script.iter().flat_map(|x| x.iter());
    let v2 = w.plutus_v2_script.iter().flat_map(|x| x.iter());
    let v3 = w.plutus_v3_script.iter().flat_map(|x| x.iter());
    let v4 = w.plutus_v4_script.iter().flat_map(|x| x.iter());
    if repeats(native.map(|x| x.raw_cbor()))
        || repeats(v1.map(|x| x.as_ref()))
        || repeats(v2.map(|x| x.as_ref()))
        || repeats(v3.map(|x| x.as_ref()))
        || repeats(v4.map(|x| x.as_ref()))
    {
        return Err(DijkstraMalformed(MalformedField::DuplicateScripts));
    }
    if let Some(redeemers) = &w.redeemer {
        if redeemers.is_empty() {
            return Err(DijkstraMalformed(MalformedField::EmptyRedeemers));
        }
        let limit = i64::MAX as u64;
        if redeemers
            .iter()
            .any(|(_, x)| x.ex_units.mem > limit || x.ex_units.steps > limit)
        {
            return Err(DijkstraMalformed(MalformedField::ExecutionUnits));
        }
    }
    check_map_keys(
        &encode(w)?,
        &[0, 1, 2, 3, 4, 5, 6, 7, 8],
        MalformedField::WitnessSet,
    )
}
fn repeats<'a>(mut items: impl Iterator<Item = &'a [u8]>) -> bool {
    let mut seen = HashSet::new();
    items.any(|x| !seen.insert(x))
}
fn check_sub_supported(tx: &SubTransaction<'_>) -> ValidationResult {
    let b = &tx.sub_transaction_body;
    let raw = encode(b)?;
    let mut allowed = vec![0, 1, 3, 8, 15];
    for (key, present) in [
        (4, b.certificates.is_some()),
        (5, b.withdrawals.is_some()),
        (7, b.auxiliary_data_hash.is_some()),
        (9, b.mint.is_some()),
        (11, b.script_data_hash.is_some()),
        (14, holds_value(&raw, 14)),
        (18, b.reference_inputs.is_some()),
        (19, b.voting_procedures.is_some()),
        (20, b.proposal_procedures.is_some()),
        (21, b.treasury_value.is_some()),
        (22, b.donation.is_some()),
        (24, b.required_top_level_guards.is_some()),
        (25, b.direct_deposits.is_some()),
        (26, b.account_balance_intervals.is_some()),
    ] {
        if present {
            allowed.push(key);
        }
    }
    check_map_keys(&raw, &allowed, MalformedField::SubtransactionBody)?;
    let fields: pallas_codec::utils::KeyValuePairs<u64, pallas_codec::utils::AnyCbor> =
        minicbor::decode(&raw)
            .map_err(|_| DijkstraMalformed(MalformedField::SubtransactionBody))?;
    for (key, value) in fields.iter() {
        check_field_present(*key, value)?;
        match key {
            5 => check_withdrawal_accounts(value)?,
            9 => check_asset_encoding(value)?,
            24 => check_required_guard_keys(value)?,
            _ => (),
        }
    }
    check_distinct(b.inputs.iter(), MalformedField::DuplicateInputs)?;
    check_distinct(
        b.reference_inputs.iter().flat_map(|x| x.iter()),
        MalformedField::DuplicateReferenceInputs,
    )?;
    check_output_encodings(b.outputs.iter())?;
    check_guards(b.guards.as_ref())?;
    check_witness_fields(&tx.transaction_witness_set)?;
    check_auxiliary_encoding(&tx.auxiliary_data)
}
fn check_withdrawal_accounts(raw: &[u8]) -> ValidationResult {
    let withdrawals: pallas_codec::utils::KeyValuePairs<Bytes, u64> = minicbor::decode(raw)
        .map_err(|_| DijkstraInvalidWithdrawal(WithdrawalFailure::Encoding))?;
    let mut seen = HashSet::new();
    match withdrawals
        .iter()
        .all(|(account, _)| seen.insert(account.to_vec()))
    {
        true => Ok(()),
        false => Err(DijkstraMalformed(
            MalformedField::DuplicateWithdrawalAccounts,
        )),
    }
}
fn check_required_guard_keys(raw: &[u8]) -> ValidationResult {
    let guards: pallas_codec::utils::KeyValuePairs<StakeCredential, pallas_codec::utils::AnyCbor> =
        minicbor::decode(raw)
            .map_err(|_| DijkstraMalformed(MalformedField::RequiredTopLevelGuards))?;
    let mut seen = BTreeSet::new();
    if guards.iter().any(|(key, _)| !seen.insert(key)) {
        return Err(DijkstraMalformed(
            MalformedField::DuplicateRequiredTopLevelGuards,
        ));
    }
    Ok(())
}
const NON_EMPTY_FIELDS: [(u64, MalformedField); 6] = [
    (5, MalformedField::EmptyWithdrawals),
    (19, MalformedField::EmptyVotingProcedures),
    (24, MalformedField::EmptyRequiredTopLevelGuards),
    (25, MalformedField::EmptyDirectDeposits),
    (26, MalformedField::EmptyAccountBalanceIntervals),
    (27, MalformedField::EmptyStartingAccountBalanceIntervals),
];
fn check_field_present(key: u64, value: &[u8]) -> ValidationResult {
    if key == 22 && minicbor::decode::<u64>(value).is_ok_and(|x| x == 0) {
        return Err(DijkstraMalformed(MalformedField::ZeroDonation));
    }
    match NON_EMPTY_FIELDS.iter().find(|(k, _)| *k == key) {
        Some((_, error)) if is_empty_map(value) => Err(DijkstraMalformed(*error)),
        _ => Ok(()),
    }
}
fn is_empty_map(raw: &[u8]) -> bool {
    let mut d = minicbor::Decoder::new(raw);
    match d.map() {
        Ok(Some(n)) => n == 0,
        Ok(None) => matches!(d.datatype(), Ok(minicbor::data::Type::Break)),
        Err(_) => false,
    }
}
fn holds_value(raw: &[u8], key: u64) -> bool {
    minicbor::decode::<pallas_codec::utils::KeyValuePairs<u64, pallas_codec::utils::AnyCbor>>(raw)
        .is_ok_and(|fields| {
            fields
                .iter()
                .any(|(k, value)| *k == key && value.raw_bytes() != [0xf6])
        })
}
fn check_map_keys(raw: &[u8], allowed: &[u64], field: MalformedField) -> ValidationResult {
    let malformed = |_| DijkstraMalformed(field);
    let mut d = minicbor::Decoder::new(raw);
    let mut remaining = d.map().map_err(malformed)?;
    let mut seen = HashSet::new();
    loop {
        match remaining {
            Some(0) => break,
            Some(ref mut n) => *n -= 1,
            None if d.datatype().map_err(malformed)? == minicbor::data::Type::Break => {
                d.skip().map_err(malformed)?;
                break;
            }
            None => (),
        }
        let key = d.u64().map_err(malformed)?;
        if !allowed.contains(&key) || !seen.insert(key) {
            return Err(DijkstraMalformed(field));
        }
        d.skip().map_err(malformed)?;
    }
    if d.position() != raw.len() {
        return Err(DijkstraMalformed(field));
    }
    Ok(())
}

type Natives = BTreeMap<Hash<28>, NativeScript>;

#[derive(Default)]
struct SubScripts {
    levels: Vec<BTreeMap<RedeemersKey, Hash<28>>>,
    needed: HashSet<Hash<28>>,
    witnessed: HashSet<Hash<28>>,
    plutus_witnessed: Vec<(Hash<28>, Language)>,
    referenced: HashSet<Hash<28>>,
    natives: Natives,
    plutus: Plutus,
    reference_bytes: u64,
}
fn sub_scripts(b: &TransactionBody<'_>, utxos: &UTxOs<'_>) -> Result<SubScripts, ValidationError> {
    let mut subs = SubScripts::default();
    for (index, sub) in (0..).zip(b.sub_transactions.iter().flatten()) {
        let sb = &sub.sub_transaction_body;
        let level = Level::sub(sb, Hasher::<256>::hash(sb.raw_cbor()), index);
        let scripts = ledger::redeemer_scripts(&level, |x| spent(x, utxos))?;
        subs.needed.extend(scripts.values());
        subs.levels.push(scripts);
        let mut outputs = Vec::new();
        for input in sb.inputs.iter() {
            outputs.push(
                utxos
                    .get(&MultiEraInput::from_alonzo_compatible(input))
                    .ok_or(PostAlonzo(InputNotInUTxO))?,
            );
        }
        // A UTxO that a sub-transaction both spends and references counts once.
        for input in sb
            .reference_inputs
            .iter()
            .flatten()
            .filter(|x| !sb.inputs.contains(x))
        {
            outputs.push(
                utxos
                    .get(&MultiEraInput::from_alonzo_compatible(input))
                    .ok_or(PostAlonzo(ReferenceInputNotInUTxO))?,
            );
        }
        for script in outputs.iter().filter_map(|x| x.multi_era_script_ref()) {
            subs.referenced.insert(script.hash());
            let size = match reference(&script)? {
                Reference::Native(native) => {
                    let raw = native.encode();
                    let decoded =
                        minicbor::decode(&raw).map_err(|_| PostAlonzo(UnsupportedNativeScript))?;
                    subs.natives.insert(script.hash(), decoded);
                    raw.len()
                }
                Reference::Plutus(language, bytes) => {
                    subs.plutus.insert(script.hash(), language);
                    bytes.len()
                }
            };
            subs.reference_bytes = add(subs.reference_bytes, size as u64)?;
        }
        let w = &sub.transaction_witness_set;
        for script in w.native_script.iter().flatten() {
            subs.witnessed.insert(script.original_hash());
            subs.natives
                .insert(script.original_hash(), (**script).clone());
        }
        for (hash, language, bytes) in plutus_witnesses(w) {
            well_formed_plutus(&language, bytes)?;
            subs.plutus_witnessed.push((hash, language.clone()));
            subs.plutus.insert(hash, language);
        }
    }
    Ok(subs)
}

type Plutus = BTreeMap<Hash<28>, Language>;

fn plutus_scripts<'a>(
    v1: impl IntoIterator<Item = &'a PlutusScript<1>>,
    v2: impl IntoIterator<Item = &'a PlutusScript<2>>,
    v3: impl IntoIterator<Item = &'a PlutusScript<3>>,
    v4: impl IntoIterator<Item = &'a PlutusScript<4>>,
) -> impl Iterator<Item = (Hash<28>, Language, &'a [u8])> {
    let v1 = v1
        .into_iter()
        .map(|x| (x.compute_hash(), Language::PlutusV1, x.as_ref()));
    let v2 = v2
        .into_iter()
        .map(|x| (x.compute_hash(), Language::PlutusV2, x.as_ref()));
    let v3 = v3
        .into_iter()
        .map(|x| (x.compute_hash(), Language::PlutusV3, x.as_ref()));
    let v4 = v4
        .into_iter()
        .map(|x| (x.compute_hash(), Language::PlutusV4, x.as_ref()));
    v1.chain(v2).chain(v3).chain(v4)
}

fn plutus_witnesses<'a>(
    w: &'a WitnessSet<'_>,
) -> impl Iterator<Item = (Hash<28>, Language, &'a [u8])> {
    plutus_scripts(
        w.plutus_v1_script.iter().flatten(),
        w.plutus_v2_script.iter().flatten(),
        w.plutus_v3_script.iter().flatten(),
        w.plutus_v4_script.iter().flatten(),
    )
}

fn legacy_mode(languages: &BTreeSet<Language>) -> bool {
    // A top level Plutus script up to V3 puts the batch in legacy mode, native and V4 scripts do not.
    languages.iter().any(|x| match x {
        Language::PlutusV1 | Language::PlutusV2 | Language::PlutusV3 => true,
        Language::PlutusV4 => false,
    })
}

pub(super) fn withdrawal_rule(languages: &BTreeSet<Language>) -> WithdrawalRule {
    match legacy_mode(languages) {
        true => WithdrawalRule::Legacy,
        false => WithdrawalRule::Partial,
    }
}

struct Provided {
    natives: Natives,
    plutus: Plutus,
    languages: BTreeSet<Language>,
}

#[allow(clippy::too_many_arguments)]
fn check_scripts(
    tx: &BlockTransaction<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    scripts: &BTreeMap<RedeemersKey, Hash<28>>,
    subs: &SubScripts,
    keys: &mut Keys,
    network: u8,
    time: Option<&crate::utils::DijkstraTimeContext>,
) -> Result<Provided, ValidationError> {
    let b = &tx.transaction_body;
    let w = &tx.transaction_witness_set;
    let mut natives = BTreeMap::new();
    let mut plutus = subs.plutus.clone();
    let mut reference_scripts = HashSet::new();
    for input in b
        .reference_inputs
        .iter()
        .flat_map(|x| x.iter())
        .chain(b.inputs.iter())
    {
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(ReferenceInputNotInUTxO))?;
        if let Some(script) = output.multi_era_script_ref() {
            reference_scripts.insert(script.hash());
            match reference(&script)? {
                Reference::Native(native) => {
                    let raw = native.encode();
                    let decoded: NativeScript =
                        minicbor::decode(&raw).map_err(|_| PostAlonzo(UnsupportedNativeScript))?;
                    natives.insert(script.hash(), decoded);
                }
                Reference::Plutus(language, _) => {
                    plutus.insert(script.hash(), language);
                }
            }
        }
    }
    let unneeded = |hash: &Hash<28>| {
        !(scripts.values().any(|x| x == hash) || subs.needed.contains(hash))
            || reference_scripts.contains(hash)
            || subs.referenced.contains(hash)
    };
    for script in w.native_script.iter().flat_map(|x| x.iter()) {
        let hash = script.original_hash();
        if unneeded(&hash) {
            return Err(PostAlonzo(UnneededNativeScript));
        }
        natives.insert(hash, (**script).clone());
    }
    if subs.witnessed.iter().any(unneeded) {
        return Err(PostAlonzo(UnneededNativeScript));
    }
    let unneeded_plutus = |language| match language {
        Language::PlutusV1 => PostAlonzo(UnneededPlutusV1Script),
        Language::PlutusV2 => PostAlonzo(UnneededPlutusV2Script),
        Language::PlutusV3 | Language::PlutusV4 => PostAlonzo(UnneededPlutusV3Script),
    };
    for (hash, language, bytes) in plutus_witnesses(w) {
        if unneeded(&hash) {
            return Err(unneeded_plutus(language));
        }
        well_formed_plutus(&language, bytes)?;
        plutus.insert(hash, language);
    }
    if let Some((_, language)) = subs.plutus_witnessed.iter().find(|(x, _)| unneeded(x)) {
        return Err(unneeded_plutus(language.clone()));
    }
    for (hash, script) in &subs.natives {
        natives.entry(*hash).or_insert_with(|| script.clone());
    }
    // The size cap covers the whole batch, and the fee counts top-level references only.
    let bytes = reference_script_size(b, utxos)?;
    if add(bytes, subs.reference_bytes)? > u64::from(pp.max_ref_script_size_per_tx) {
        return Err(DijkstraReferenceScriptsTooLarge);
    }
    let provided = Provided {
        natives,
        plutus,
        languages: BTreeSet::new(),
    };
    let level = Level::top(b, Hasher::<256>::hash(b.raw_cbor()));
    let languages = check_level(
        &level,
        w,
        b.script_data_hash,
        scripts,
        &provided,
        sub_supplemental_datums(b, utxos)?,
        utxos,
        pp,
        time,
    )?;
    let (mem, steps) = declared_units(tx)?;
    if mem > pp.max_tx_ex_units.mem || steps > pp.max_tx_ex_units.steps {
        return Err(PostAlonzo(TxExUnitsExceeded));
    }
    let levels = std::iter::once(w).chain(
        b.sub_transactions
            .iter()
            .flatten()
            .map(|s| &s.transaction_witness_set),
    );
    let redeemers = levels
        .into_iter()
        .any(|w| w.redeemer.as_ref().is_some_and(|x| !x.is_empty()));
    check_collateral(b, utxos, pp, keys, network, redeemers)?;
    Ok(Provided {
        languages,
        ..provided
    })
}

fn check_collateral(
    b: &TransactionBody<'_>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    keys: &mut Keys,
    network: u8,
    redeemers: bool,
) -> ValidationResult {
    let collateral: Vec<_> = b.collateral.iter().flatten().collect();
    if redeemers && collateral.is_empty() {
        return Err(PostAlonzo(CollateralMissing));
    }
    if collateral.len() > pp.max_collateral_inputs as usize {
        return Err(PostAlonzo(TooManyCollaterals));
    }
    let mut collateral_assets = BTreeMap::new();
    let mut total = 0;
    for input in collateral {
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(CollateralNotInUTxO))?;
        match spender(output)? {
            Spender::Key(key) => {
                keys.insert(key);
            }
            Spender::Script(_) if redeemers => return Err(PostAlonzo(CollateralNotVKeyLocked)),
            Spender::Script(_) => {}
        }
        if redeemers {
            total = add(total, output.value().coin())?;
            accumulate_assets(&mut collateral_assets, output, 1)?;
        }
    }
    let returned = match &b.collateral_return {
        Some(output) => check_outputs(std::slice::from_ref(output), pp, &network)?,
        None => 0,
    };
    if let Some(output) = &b.collateral_return
        && let Ok(Address::Shelley(address)) = MultiEraOutput::from_dijkstra(output).address()
        && matches!(address.delegation(), ShelleyDelegationPart::Pointer(_))
    {
        return Err(DijkstraPointerInCollateralReturn);
    }
    if !redeemers {
        return Ok(());
    }
    if let Some(output) = &b.collateral_return {
        accumulate_assets(
            &mut collateral_assets,
            &MultiEraOutput::from_dijkstra(output),
            -1,
        )?;
    }
    if collateral_assets.values().any(|x| *x != 0) {
        return Err(PostAlonzo(NonLovelaceCollateral));
    }
    let paid = i128::from(total) - i128::from(returned);
    if b.total_collateral.is_some_and(|x| i128::from(x) != paid) {
        return Err(PostAlonzo(CollateralAnnotation));
    }
    if paid * 100 < i128::from(b.fee) * i128::from(pp.collateral_percentage) {
        return Err(PostAlonzo(CollateralMinLovelace));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_level(
    b: &Level<'_>,
    w: &WitnessSet<'_>,
    declared: Option<Hash<32>>,
    scripts: &BTreeMap<RedeemersKey, Hash<28>>,
    provided: &Provided,
    sub_datums: HashSet<Hash<32>>,
    utxos: &UTxOs<'_>,
    pp: &DijkstraProtParams,
    time: Option<&crate::utils::DijkstraTimeContext>,
) -> Result<BTreeSet<Language>, ValidationError> {
    let witnesses = witness_keys(w);
    let guards: BTreeSet<_> = guard_credentials(b.guards).into_iter().collect();
    for (hash, script) in &provided.natives {
        if scripts.values().any(|x| x == hash)
            && !crate::utils::dijkstra_native::evaluate(
                script,
                &witnesses,
                b.validity_interval_start,
                b.ttl,
                &guards,
            )
        {
            return Err(PostAlonzo(NativeScriptDenial));
        }
    }
    // Retain original purpose indices: native scripts need no redeemer or datum.
    let scripts: BTreeMap<_, _> = scripts
        .iter()
        .filter(|(_, hash)| !provided.natives.contains_key(hash))
        .map(|(key, hash)| (key.clone(), *hash))
        .collect();
    let mut languages = BTreeSet::new();
    for hash in scripts.values() {
        let language = provided
            .plutus
            .get(hash)
            .ok_or(PostAlonzo(ScriptWitnessMissing))?;
        languages.insert(language.clone());
    }
    if !scripts.is_empty() {
        match time {
            Some(time) => {
                time.validity_bounds(b.validity_interval_start, b.ttl)?;
            }
            None if b.ttl.is_some() => {
                return Err(DijkstraUnsupported(
                    UnsupportedFeature::PlutusUpperBoundWithoutForecast,
                ));
            }
            None => {}
        }
    }
    let mut allowed = supplemental_datums(b, utxos)?;
    // In legacy mode the top level may witness only the datums its own body names.
    if !legacy_mode(&languages) {
        allowed.extend(sub_datums);
    }
    check_datums(b, w, utxos, &scripts, &provided.plutus, allowed)?;
    if scripts.is_empty() {
        if w.redeemer.as_ref().is_some_and(|x| !x.is_empty()) {
            return Err(PostAlonzo(UnneededRedeemer));
        }
        // An empty redeemer map and language view still participate when datums exist.
        check_integrity(w, declared, None)?;
        return Ok(languages);
    }
    let models = &pp.cost_models_for_script_languages;
    let mut views = BTreeMap::new();
    for language in &languages {
        let (key, model) = match language {
            Language::PlutusV1 => (0, &models.plutus_v1),
            Language::PlutusV2 => (1, &models.plutus_v2),
            Language::PlutusV3 => (2, &models.plutus_v3),
            Language::PlutusV4 => (3, &models.plutus_v4),
        };
        let model = model
            .clone()
            .ok_or(DijkstraScriptContext(ContextFailure::CostModel))?;
        views.insert(key, model);
    }
    let redeemers = w.redeemer.as_ref().ok_or(PostAlonzo(RedeemerMissing))?;
    for index in scripts.keys() {
        if !redeemers.contains_key(index) {
            return Err(PostAlonzo(RedeemerMissing));
        }
    }
    if redeemers.iter().any(|(key, _)| !scripts.contains_key(key)) {
        return Err(PostAlonzo(UnneededRedeemer));
    }
    check_integrity(w, declared, Some(views))?;
    Ok(languages)
}

fn execution_fee(mem: u64, steps: u64, pp: &DijkstraProtParams) -> Result<u64, ValidationError> {
    if mem == 0 && steps == 0 {
        return Ok(0);
    }
    let (n, d) = fraction_add(
        fraction_scale(price(&pp.execution_costs.mem_price)?, mem)?,
        fraction_scale(price(&pp.execution_costs.step_price)?, steps)?,
    )?;
    u64::try_from(n / d + u128::from(n % d != 0)).map_err(|_| PostAlonzo(NegativeValue))
}

// Exact nonnegative arithmetic: execution fees round up once, and the total tiered
// reference fee rounds down once.
type Fraction = (u128, u128);
fn price(x: &RationalNumber) -> Result<Fraction, ValidationError> {
    if x.denominator == 0 {
        return Err(DijkstraMissingParameters(
            ParameterFailure::NonzeroPriceDenominator,
        ));
    }
    Ok((u128::from(x.numerator), u128::from(x.denominator)))
}
fn reduce((n, d): Fraction) -> Fraction {
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (n / a, d / a)
}
fn fraction_scale((n, d): Fraction, x: u64) -> Result<Fraction, ValidationError> {
    Ok(reduce((
        n.checked_mul(u128::from(x))
            .ok_or(PostAlonzo(NegativeValue))?,
        d,
    )))
}
fn fraction_add((a, b): Fraction, (c, d): Fraction) -> Result<Fraction, ValidationError> {
    let n = a
        .checked_mul(d)
        .and_then(|x| c.checked_mul(b).and_then(|y| x.checked_add(y)))
        .ok_or(PostAlonzo(NegativeValue))?;
    Ok(reduce((
        n,
        b.checked_mul(d).ok_or(PostAlonzo(NegativeValue))?,
    )))
}
fn reference_fee(mut bytes: u64, p: &DijkstraProtParams) -> Result<u64, ValidationError> {
    if bytes == 0 {
        return Ok(0);
    }
    if p.ref_script_cost_stride == 0 {
        return Err(DijkstraMissingParameters(
            ParameterFailure::ReferenceFeeStride,
        ));
    }
    let mut rate = price(&p.minfee_refscript_cost_per_byte)?;
    let multiplier = price(&p.ref_script_cost_multiplier)?;
    let mut total = (0, 1);
    while bytes != 0 {
        let count = bytes.min(u64::from(p.ref_script_cost_stride));
        total = fraction_add(total, fraction_scale(rate, count)?)?;
        bytes -= count;
        if bytes != 0 {
            rate = reduce((
                rate.0
                    .checked_mul(multiplier.0)
                    .ok_or(PostAlonzo(NegativeValue))?,
                rate.1
                    .checked_mul(multiplier.1)
                    .ok_or(PostAlonzo(NegativeValue))?,
            ));
        }
    }
    u64::try_from(total.0 / total.1).map_err(|_| PostAlonzo(NegativeValue))
}

fn check_auxiliary_hash(
    hash: Option<Hash<32>>,
    auxiliary: &Nullable<KeepRaw<'_, AuxiliaryData>>,
) -> ValidationResult {
    match auxiliary {
        Nullable::Null if hash.is_none() => Ok(()),
        Nullable::Some(data) if hash == Some(Hasher::<256>::hash(&encode(data)?)) => Ok(()),
        _ => Err(PostAlonzo(MetadataHash)),
    }
}
fn check_auxiliary_encoding(auxiliary: &Nullable<KeepRaw<'_, AuxiliaryData>>) -> ValidationResult {
    let Nullable::Some(data) = auxiliary else {
        return Ok(());
    };
    check_metadata_encoding(data)?;
    if let AuxiliaryData::PostAlonzo(m) = &**data {
        let scripts = plutus_scripts(
            m.plutus_v1_scripts.iter().flatten(),
            m.plutus_v2_scripts.iter().flatten(),
            m.plutus_v3_scripts.iter().flatten(),
            m.plutus_v4_scripts.iter().flatten(),
        );
        for (_, language, script) in scripts {
            match well_formed_plutus(&language, script) {
                Err(DijkstraMalformedScript) => return Err(DijkstraInvalidMetadata),
                result => result?,
            }
        }
    }
    Ok(())
}
fn check_metadata_encoding(data: &KeepRaw<'_, AuxiliaryData>) -> ValidationResult {
    let metadata = match &**data {
        AuxiliaryData::Shelley(m) => Some(m),
        AuxiliaryData::ShelleyMa(m) => Some(&m.transaction_metadata),
        AuxiliaryData::PostAlonzo(m) => m.metadata.as_ref(),
    };
    // Map decoding into BTreeMap must not hide duplicate metadata labels.
    let raw = encode(data)?;
    let mut decoder = minicbor::Decoder::new(&raw);
    let metadata_bytes = match &**data {
        AuxiliaryData::Shelley(_) => Some(raw.clone()),
        AuxiliaryData::ShelleyMa(_) => {
            let length = decoder.array().map_err(|_| MALFORMED_METADATA)?;
            let value: pallas_codec::utils::AnyCbor =
                decoder.decode().map_err(|_| MALFORMED_METADATA)?;
            decoder.skip().map_err(|_| MALFORMED_METADATA)?;
            let two_elements = match length {
                Some(n) => n == 2,
                None => decoder
                    .datatype()
                    .is_ok_and(|t| t == minicbor::data::Type::Break),
            };
            if !two_elements {
                return Err(MALFORMED_METADATA);
            }
            Some(value.to_vec())
        }
        AuxiliaryData::PostAlonzo(_) => {
            decoder.tag().map_err(|_| MALFORMED_METADATA)?;
            check_map_keys(
                &raw[decoder.position()..],
                &[0, 1, 2, 3, 4, 5],
                MalformedField::Metadata,
            )?;
            let fields: pallas_codec::utils::KeyValuePairs<u64, pallas_codec::utils::AnyCbor> =
                decoder.decode().map_err(|_| MALFORMED_METADATA)?;
            fields
                .iter()
                .find(|(key, _)| *key == 0)
                .map(|(_, value)| value.to_vec())
        }
    };
    if let Some(bytes) = metadata_bytes {
        let entries: pallas_codec::utils::KeyValuePairs<u64, Metadatum> =
            minicbor::decode(&bytes).map_err(|_| MALFORMED_METADATA)?;
        let mut labels = HashSet::new();
        if entries.iter().any(|(label, _)| !labels.insert(*label)) {
            return Err(MALFORMED_METADATA);
        }
    }
    let mut pending: Vec<_> = metadata.into_iter().flat_map(|m| m.values()).collect();
    while let Some(item) = pending.pop() {
        match item {
            Metadatum::Text(x) if x.len() > 64 => return Err(MALFORMED_METADATA),
            Metadatum::Bytes(x) if x.len() > 64 => return Err(MALFORMED_METADATA),
            Metadatum::Array(xs) => pending.extend(xs),
            Metadatum::Map(xs) => {
                for (k, v) in xs.iter() {
                    pending.extend([k, v]);
                }
            }
            _ => (),
        }
    }
    Ok(())
}

type AssetBalance = BTreeMap<(Hash<28>, Vec<u8>), i128>;
fn accumulate_assets(
    total: &mut AssetBalance,
    output: &MultiEraOutput<'_>,
    sign: i128,
) -> ValidationResult {
    if let pallas_primitives::alonzo::Value::Multiasset(_, assets) = output.value().into_alonzo() {
        for (policy, assets) in assets {
            for (name, quantity) in assets {
                if name.len() > 32 {
                    return Err(PostAlonzo(NegativeValue));
                }
                let entry = total.entry((policy, name.to_vec())).or_default();
                *entry = entry
                    .checked_add(i128::from(quantity) * sign)
                    .ok_or(PostAlonzo(NegativeValue))?;
            }
        }
    }
    Ok(())
}
fn check_assets<'a, 'b: 'a>(
    inputs: impl Iterator<Item = &'a TransactionInput>,
    outputs: impl Iterator<Item = &'a TransactionOutput<'b>>,
    mint: impl Iterator<Item = &'a Mint>,
    utxos: &UTxOs<'_>,
) -> ValidationResult {
    let mut balance = AssetBalance::new();
    for input in inputs {
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(InputNotInUTxO))?;
        accumulate_assets(&mut balance, output, 1)?;
    }
    for output in outputs {
        accumulate_assets(&mut balance, &MultiEraOutput::from_dijkstra(output), -1)?;
    }
    for (policy, assets) in mint.flat_map(|m| m.iter()) {
        for (name, quantity) in assets {
            let entry = balance.entry((*policy, name.to_vec())).or_default();
            *entry = entry
                .checked_add(i128::from(i64::from(quantity)))
                .ok_or(PostAlonzo(NegativeValue))?;
        }
    }
    if balance.values().any(|x| *x != 0) {
        return Err(PostAlonzo(PreservationOfValue));
    }
    Ok(())
}

fn supplemental_datums(
    b: &Level<'_>,
    utxos: &UTxOs<'_>,
) -> Result<HashSet<Hash<32>>, ValidationError> {
    let mut hashes = HashSet::new();
    for output in b
        .outputs
        .iter()
        .chain(b.collateral_return)
        .map(MultiEraOutput::from_dijkstra)
    {
        if let Some(DatumOption::Hash(hash)) = output.datum() {
            hashes.insert(hash);
        }
    }
    for input in b.reference_inputs.into_iter().flatten() {
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(ReferenceInputNotInUTxO))?;
        if let Some(DatumOption::Hash(hash)) = output.datum() {
            hashes.insert(hash);
        }
    }
    Ok(hashes)
}

fn sub_supplemental_datums(
    b: &TransactionBody<'_>,
    utxos: &UTxOs<'_>,
) -> Result<HashSet<Hash<32>>, ValidationError> {
    let mut hashes = HashSet::new();
    for (index, sub) in (0..).zip(b.sub_transactions.iter().flatten()) {
        let sb = &sub.sub_transaction_body;
        let level = Level::sub(sb, Hasher::<256>::hash(sb.raw_cbor()), index);
        hashes.extend(supplemental_datums(&level, utxos)?);
    }
    Ok(hashes)
}

fn check_datums(
    b: &Level<'_>,
    w: &WitnessSet<'_>,
    utxos: &UTxOs<'_>,
    scripts: &BTreeMap<RedeemersKey, Hash<28>>,
    plutus: &Plutus,
    mut allowed: HashSet<Hash<32>>,
) -> ValidationResult {
    let mut required = HashSet::new();
    for (index, input) in b
        .inputs
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .enumerate()
    {
        if let Some(hash) = scripts.get(&RedeemersKey {
            tag: RedeemerTag::Spend,
            index: index as u32,
        }) {
            let output = utxos
                .get(&MultiEraInput::from_alonzo_compatible(input))
                .ok_or(PostAlonzo(InputNotInUTxO))?;
            match output.datum() {
                Some(DatumOption::Hash(hash)) => {
                    required.insert(hash);
                    allowed.insert(hash);
                }
                Some(DatumOption::Data(_)) => (),
                // CIP-0069: only V3 may spend an output without a datum. A
                // present hash still requires its matching witness datum.
                None => {
                    if matches!(
                        plutus.get(hash),
                        Some(Language::PlutusV1 | Language::PlutusV2)
                    ) {
                        return Err(DijkstraInputWithoutDatum);
                    }
                }
            }
        }
    }
    for datum in w.plutus_data.iter().flat_map(|x| x.iter()) {
        let hash = Hasher::<256>::hash(&encode(datum)?);
        if !allowed.contains(&hash) {
            return Err(PostAlonzo(UnneededDatum));
        }
        required.remove(&hash);
    }
    if !required.is_empty() {
        return Err(PostAlonzo(DatumMissing));
    }
    Ok(())
}

fn check_integrity(
    w: &WitnessSet<'_>,
    declared: Option<Hash<32>>,
    views: Option<BTreeMap<u8, Vec<i64>>>,
) -> ValidationResult {
    let has_data = w.plutus_data.as_ref().is_some_and(|x| !x.is_empty());
    let has_redeemers = w.redeemer.as_ref().is_some_and(|x| !x.is_empty());
    let expected = if views.is_none() && !has_data && !has_redeemers {
        None
    } else {
        let mut bytes = match &w.redeemer {
            Some(r) => encode(r)?,
            None => vec![0xa0],
        };
        if let Some(data) = w.plutus_data.as_ref().filter(|_| has_data) {
            bytes.extend(encode(data)?);
        }
        bytes.extend(encode(LanguageViews(views.unwrap_or_default()))?);
        Some(Hasher::<256>::hash(&bytes))
    };
    if declared != expected {
        return Err(PostAlonzo(ScriptIntegrityHash));
    }
    Ok(())
}

enum Reference<'a> {
    Native(MultiEraNativeScript<'a>),
    Plutus(Language, &'a [u8]),
}

fn reference<'a>(script: &'a MultiEraScriptRef<'_>) -> Result<Reference<'a>, ValidationError> {
    use pallas_primitives::conway::ScriptRef as C;
    match script {
        MultiEraScriptRef::Dijkstra(x) => match x.as_ref() {
            ScriptRef::NativeScript(s) => {
                Ok(Reference::Native(MultiEraNativeScript::from_dijkstra(s)))
            }
            ScriptRef::PlutusV1Script(s) => Ok(Reference::Plutus(Language::PlutusV1, s.as_ref())),
            ScriptRef::PlutusV2Script(s) => Ok(Reference::Plutus(Language::PlutusV2, s.as_ref())),
            ScriptRef::PlutusV3Script(s) => Ok(Reference::Plutus(Language::PlutusV3, s.as_ref())),
            ScriptRef::PlutusV4Script(s) => Ok(Reference::Plutus(Language::PlutusV4, s.as_ref())),
        },
        MultiEraScriptRef::Conway(x) => match x.as_ref() {
            C::NativeScript(s) => Ok(Reference::Native(
                MultiEraNativeScript::from_alonzo_compatible(s),
            )),
            C::PlutusV1Script(s) => Ok(Reference::Plutus(Language::PlutusV1, s.as_ref())),
            C::PlutusV2Script(s) => Ok(Reference::Plutus(Language::PlutusV2, s.as_ref())),
            C::PlutusV3Script(s) => Ok(Reference::Plutus(Language::PlutusV3, s.as_ref())),
        },
        _ => Err(DijkstraUnsupported(
            UnsupportedFeature::ReferenceScriptLanguage,
        )),
    }
}

fn check_new_script(script: &MultiEraScriptRef<'_>) -> ValidationResult {
    match script {
        MultiEraScriptRef::Dijkstra(s) => match s.as_ref() {
            ScriptRef::NativeScript(_) => Ok(()),
            ScriptRef::PlutusV1Script(s) => well_formed_plutus(&Language::PlutusV1, s.as_ref()),
            ScriptRef::PlutusV2Script(s) => well_formed_plutus(&Language::PlutusV2, s.as_ref()),
            ScriptRef::PlutusV3Script(s) => well_formed_plutus(&Language::PlutusV3, s.as_ref()),
            ScriptRef::PlutusV4Script(s) => well_formed_plutus(&Language::PlutusV4, s.as_ref()),
        },
        _ => Err(DijkstraUnsupported(UnsupportedFeature::NewScriptLanguage)),
    }
}
fn well_formed_plutus(language: &Language, script: &[u8]) -> ValidationResult {
    #[cfg(all(feature = "phase2", pallas_dijkstra_evaluator))]
    {
        crate::phase2::native_evaluator::check_plutus_script(language, script)
            .map_err(|_| DijkstraMalformedScript)
    }
    #[cfg(not(all(feature = "phase2", pallas_dijkstra_evaluator)))]
    {
        let _ = (language, script);
        Err(DijkstraUnsupported(UnsupportedFeature::PlutusWithoutPhase2))
    }
}

fn check_asset_encoding(raw: &[u8]) -> ValidationResult {
    use pallas_codec::utils::KeyValuePairs;
    let assets: KeyValuePairs<Hash<28>, KeyValuePairs<Bytes, minicbor::data::Int>> =
        minicbor::decode(raw).map_err(|_| MALFORMED_VALUE)?;
    let mut policies = HashSet::new();
    if assets.is_empty() {
        return Err(MALFORMED_VALUE);
    }
    for (policy, names) in assets.iter() {
        if !policies.insert(*policy) || names.is_empty() {
            return Err(MALFORMED_VALUE);
        }
        let mut seen = HashSet::new();
        for (name, quantity) in names.iter() {
            if name.len() > 32 || !seen.insert(name.to_vec()) || i128::from(*quantity) == 0 {
                return Err(MALFORMED_VALUE);
            }
        }
    }
    Ok(())
}
fn check_output_value_encoding(raw: &[u8]) -> ValidationResult {
    use pallas_codec::utils::{AnyCbor, KeyValuePairs};
    let mut decoder = minicbor::Decoder::new(raw);
    let value = match decoder.datatype().map_err(|_| MALFORMED_VALUE)? {
        minicbor::data::Type::Map | minicbor::data::Type::MapIndef => {
            let fields: KeyValuePairs<u64, AnyCbor> =
                decoder.decode().map_err(|_| MALFORMED_VALUE)?;
            fields
                .iter()
                .find(|(k, _)| *k == 1)
                .map(|(_, v)| v.clone())
                .ok_or(MALFORMED_VALUE)?
        }
        _ => {
            decoder.array().map_err(|_| MALFORMED_VALUE)?;
            decoder.skip().map_err(|_| MALFORMED_VALUE)?;
            decoder.decode::<AnyCbor>().map_err(|_| MALFORMED_VALUE)?
        }
    };
    let mut d = minicbor::Decoder::new(&value);
    if matches!(
        d.datatype(),
        Ok(minicbor::data::Type::Array | minicbor::data::Type::ArrayIndef)
    ) {
        let values: Vec<AnyCbor> = d.decode().map_err(|_| MALFORMED_VALUE)?;
        if values.len() != 2 {
            return Err(MALFORMED_VALUE);
        }
        check_asset_encoding(&values[1])?;
    }
    Ok(())
}
