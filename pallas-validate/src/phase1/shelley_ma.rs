//! Utilities required for ShelleyMA-era transaction validation.

use crate::utils::{
    AccountState, CertPointer, CertState, DState, PState, PoolParam,
    ShelleyMAError::*,
    ShelleyProtParams, SpendingCredential, UTxOs,
    ValidationError::{self, *},
    ValidationResult, add_minted_value, add_values, aux_data_from_alonzo_tx, balance_coins,
    certificate_key_hashes, certificate_scripts, certificates, check_bootstrap_wit,
    check_remaining_bootstrap_wits, check_remaining_vk_wits, compute_native_script_hash,
    empty_value, get_alonzo_comp_tx_size, get_lovelace_from_alonzo_val, get_shelley_address,
    get_val_size_in_words, mk_wits_check_list, needed_certificate_scripts, reference_scripts,
    spending_credential, values_are_equal, verify_signature,
};
use pallas_addresses::{PaymentKeyHash, ScriptHash, ShelleyAddress};
use pallas_codec::minicbor::encode;
use pallas_codec::utils::KeepRaw;
use pallas_crypto::hash::Hasher as PallasHasher;
use pallas_primitives::{
    alonzo::{
        BootstrapWitness,
        Certificate::{self, *},
        Coin, Epoch, GenesisDelegateHash, Genesishash,
        InstantaneousRewardSource::*,
        InstantaneousRewardTarget::*,
        MoveInstantaneousReward, NativeScript, PolicyId, PoolKeyhash,
        StakeCredential::{self},
        TransactionBody, TransactionIndex, TransactionOutput, Tx, VKeyWitness, Value, VrfKeyhash,
        WitnessSet,
    },
    byron::TxOut,
    conway::ScriptRef,
};
use pallas_traverse::{
    Era, MultiEraInput, MultiEraOutput, MultiEraTx, OriginalHash, time::Slot,
    wellknown::GenesisValues,
};

use std::{cmp::max, collections::HashMap, ops::Deref};
// TODO: remove when fixed missing args

#[allow(clippy::too_many_arguments)]
pub fn validate_shelley_ma_tx(
    mtx: &Tx,
    txix: TransactionIndex,
    utxos: &UTxOs,
    cert_state: &mut CertState,
    prot_pps: &ShelleyProtParams,
    acnt: &AccountState,
    block_slot: &u64,
    network_id: &u8,
    era: &Era,
) -> ValidationResult {
    let tx_body: &TransactionBody = &mtx.transaction_body;
    let minted_tx_body: &KeepRaw<'_, TransactionBody> = &mtx.transaction_body;
    let tx_wits: &WitnessSet = &mtx.transaction_witness_set;
    let size: u32 = get_alonzo_comp_tx_size(mtx);
    let stk_dep_count: &mut u64 = &mut 0; // count of key registrations (for deposits)
    let stk_refund_count: &mut u64 = &mut 0; // count of key deregs (for refunds)
    let pool_count: &mut u64 = &mut 0; // count of pool regs (for deposits)

    let stab_win = 129600; // FIXME: Found as "1.5 days" in unreliable sources.

    check_ins_not_empty(tx_body)?;
    check_ins_in_utxos(tx_body, utxos)?;
    check_ttl(tx_body, block_slot)?;
    check_tx_size(&size, prot_pps)?;
    check_min_lovelace(tx_body, prot_pps, era)?;
    check_certificates(
        &tx_body.certificates,
        txix,
        cert_state,
        stk_dep_count,
        stk_refund_count,
        pool_count,
        acnt,
        block_slot,
        &stab_win,
        prot_pps,
    )?;
    check_preservation_of_value(
        tx_body,
        utxos,
        stk_dep_count,
        stk_refund_count,
        pool_count,
        era,
        prot_pps,
    )?;
    check_fees(tx_body, &size, prot_pps)?;
    check_network_id(tx_body, network_id)?;
    check_metadata(tx_body, mtx)?;
    check_witnesses(minted_tx_body, tx_wits, utxos)?;
    check_minting(tx_body, mtx)
}

fn check_ins_not_empty(tx_body: &TransactionBody) -> ValidationResult {
    if tx_body.inputs.is_empty() {
        return Err(ShelleyMA(TxInsEmpty));
    }
    Ok(())
}

fn check_ins_in_utxos(tx_body: &TransactionBody, utxos: &UTxOs) -> ValidationResult {
    for input in tx_body.inputs.iter() {
        if !(utxos.contains_key(&MultiEraInput::from_alonzo_compatible(input))) {
            return Err(ShelleyMA(InputNotInUTxO));
        }
    }
    Ok(())
}

fn check_ttl(tx_body: &TransactionBody, block_slot: &u64) -> ValidationResult {
    match tx_body.ttl {
        Some(ttl) => {
            if ttl < *block_slot {
                Err(ShelleyMA(TTLExceeded))
            } else {
                Ok(())
            }
        }
        None => Err(ShelleyMA(AlonzoCompNotShelley)),
    }
}

fn check_tx_size(size: &u32, prot_pps: &ShelleyProtParams) -> ValidationResult {
    if *size > prot_pps.max_transaction_size {
        return Err(ShelleyMA(MaxTxSizeExceeded));
    }
    Ok(())
}

fn check_min_lovelace(
    tx_body: &TransactionBody,
    prot_pps: &ShelleyProtParams,
    era: &Era,
) -> ValidationResult {
    for output in &tx_body.outputs {
        match era {
            Era::Shelley | Era::Allegra | Era::Mary => {
                if get_lovelace_from_alonzo_val(&output.amount)
                    < compute_min_lovelace(output, prot_pps)
                {
                    return Err(ShelleyMA(MinLovelaceUnreached));
                }
            }
            _ => return Err(ShelleyMA(ValueNotShelley)),
        }
    }
    Ok(())
}

fn compute_min_lovelace(output: &TransactionOutput, prot_pps: &ShelleyProtParams) -> u64 {
    match &output.amount {
        Value::Coin(_) => prot_pps.min_utxo_value,
        Value::Multiasset(lovelace, _) => {
            let utxo_entry_size: u64 = 27 + get_val_size_in_words(&output.amount);
            let coins_per_utxo_word: u64 = prot_pps.min_utxo_value / 27;
            max(*lovelace, utxo_entry_size * coins_per_utxo_word)
        }
    }
}

fn check_preservation_of_value(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    stk_dep_count: &u64,
    stk_refund_count: &u64,
    pool_count: &u64,
    era: &Era,
    prot_pps: &ShelleyProtParams,
) -> ValidationResult {
    let err = ShelleyMA(NegativeValue);
    let deposits = prot_pps.pool_deposit * *pool_count + prot_pps.key_deposit * *stk_dep_count;
    let refunds = prot_pps.key_deposit * *stk_refund_count;
    let (consumed, produced) = balance_coins(
        [(deposits, refunds)],
        tx_body.withdrawals.as_ref(),
        [tx_body.fee],
        &err,
    )?;
    let consumed: Value = add_values(
        &get_consumed(tx_body, utxos, era)?,
        &Value::Coin(consumed),
        &err,
    )?;
    let produced: Value = add_values(&get_produced(tx_body, era)?, &Value::Coin(produced), &err)?;
    if !values_are_equal(&consumed, &produced) {
        Err(ShelleyMA(PreservationOfValue))
    } else {
        Ok(())
    }
}

fn get_consumed(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    era: &Era,
) -> Result<Value, ValidationError> {
    let neg_val_err: ValidationError = ShelleyMA(NegativeValue);
    let mut res: Value = empty_value();
    for input in tx_body.inputs.iter() {
        let utxo_value: &MultiEraOutput = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(ShelleyMA(InputNotInUTxO))?;
        match MultiEraOutput::as_alonzo(utxo_value) {
            Some(TransactionOutput { amount, .. }) => match (amount, era) {
                (Value::Coin(..), _) => res = add_values(&res, amount, &neg_val_err)?,
                (Value::Multiasset(..), Era::Shelley) => return Err(ShelleyMA(ValueNotShelley)),
                _ => res = add_values(&res, amount, &neg_val_err)?,
            },
            None => match MultiEraOutput::as_byron(utxo_value) {
                Some(TxOut { amount, .. }) => {
                    res = add_values(&res, &Value::Coin(*amount), &neg_val_err)?
                }
                _ => return Err(ShelleyMA(InputNotInUTxO)),
            },
        }
    }
    if let Some(m) = &tx_body.mint {
        res = add_minted_value(&res, m, &neg_val_err)?;
    }
    Ok(res)
}

fn get_produced(tx_body: &TransactionBody, era: &Era) -> Result<Value, ValidationError> {
    let neg_val_err: ValidationError = ShelleyMA(NegativeValue);
    let mut res: Value = empty_value();
    for TransactionOutput { amount, .. } in tx_body.outputs.iter() {
        match (amount, era) {
            (Value::Coin(..), _) => res = add_values(&res, amount, &neg_val_err)?,
            (Value::Multiasset(..), Era::Shelley) => return Err(ShelleyMA(WrongEraOutput)),
            _ => res = add_values(&res, amount, &neg_val_err)?,
        }
    }
    Ok(res)
}

fn check_fees(
    tx_body: &TransactionBody,
    size: &u32,
    prot_pps: &ShelleyProtParams,
) -> ValidationResult {
    if tx_body.fee < (prot_pps.minfee_b + prot_pps.minfee_a * size) as u64 {
        return Err(ShelleyMA(FeesBelowMin));
    }
    Ok(())
}

fn check_network_id(tx_body: &TransactionBody, network_id: &u8) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let addr: ShelleyAddress =
            get_shelley_address(&output.address).ok_or(ShelleyMA(AddressDecoding))?;
        if addr.network().value() != *network_id {
            return Err(ShelleyMA(WrongNetworkID));
        }
    }
    Ok(())
}

fn check_metadata(tx_body: &TransactionBody, mtx: &Tx) -> ValidationResult {
    match (&tx_body.auxiliary_data_hash, aux_data_from_alonzo_tx(mtx)) {
        (Some(metadata_hash), Some(metadata)) => {
            if metadata_hash.as_slice()
                == pallas_crypto::hash::Hasher::<256>::hash(metadata).as_ref()
            {
                Ok(())
            } else {
                Err(ShelleyMA(MetadataHash))
            }
        }
        (None, None) => Ok(()),
        _ => Err(ShelleyMA(MetadataHash)),
    }
}

fn check_witnesses(
    tx_body: &KeepRaw<'_, TransactionBody>,
    tx_wits: &WitnessSet,
    utxos: &UTxOs,
) -> ValidationResult {
    let vk_wits: &mut Vec<(bool, VKeyWitness)> = &mut crate::utils::mk_alonzo_vk_wits_check_list(
        &tx_wits.vkeywitness,
        ShelleyMA(MissingVKWitness),
    )?;
    let bootstrap_wits: &mut Vec<(bool, BootstrapWitness)> =
        &mut mk_wits_check_list(tx_wits.bootstrap_witness.iter().flatten());
    let tx_hash: &Vec<u8> = &Vec::from(tx_body.original_hash().as_ref());
    let native_scripts: Vec<NativeScript> = match &tx_wits.native_script {
        Some(scripts) => scripts.iter().map(|x| x.clone().unwrap()).collect(),
        None => Vec::new(),
    };
    let native_script_wits: Option<Vec<NativeScript>> = tx_wits
        .native_script
        .as_ref()
        .map(|x| x.iter().map(|y| y.deref().clone()).collect());
    for input in tx_body.inputs.iter() {
        match utxos.get(&MultiEraInput::from_alonzo_compatible(input)) {
            Some(multi_era_output) => {
                match spending_credential(multi_era_output).ok_or(ShelleyMA(AddressDecoding))? {
                    SpendingCredential::Key(payment_key_hash) => {
                        check_vk_wit(&payment_key_hash, tx_hash, vk_wits)?
                    }
                    SpendingCredential::Script(script_hash) => {
                        check_native_script_witness(&script_hash, &native_script_wits)?
                    }
                    SpendingCredential::Bootstrap(root) => check_bootstrap_wit(
                        &root,
                        bootstrap_wits,
                        tx_hash,
                        &ShelleyMA(MissingVKWitness),
                        &ShelleyMA(WrongSignature),
                    )?,
                }
            }
            None => return Err(ShelleyMA(InputNotInUTxO)),
        }
    }
    for cert in certificates(&tx_body.certificates) {
        for key_hash in certificate_key_hashes(cert) {
            check_vk_wit(key_hash, tx_hash, vk_wits)?;
        }
    }
    for (_, script_hash) in certificate_scripts(certificates(&tx_body.certificates)) {
        check_native_script_witness(script_hash, &native_script_wits)?;
    }
    let vkey_wits = &vk_wits.iter().map(|bv| bv.clone().1).collect();
    check_native_scripts(
        vkey_wits,
        &native_scripts,
        &tx_body.validity_interval_start,
        &tx_body.ttl,
    )?;
    check_remaining_vk_wits(vk_wits, tx_hash, ShelleyMA(WrongSignature))?;
    check_remaining_bootstrap_wits(bootstrap_wits, tx_hash, &ShelleyMA(WrongSignature))
}

fn check_vk_wit(
    payment_key_hash: &PaymentKeyHash,
    data_to_verify: &[u8],
    wits: &mut [(bool, VKeyWitness)],
) -> ValidationResult {
    for (found, vkey_wit) in wits {
        if pallas_crypto::hash::Hasher::<224>::hash(&vkey_wit.vkey.clone()) == *payment_key_hash {
            if verify_signature(vkey_wit, data_to_verify) {
                *found = true;
                return Ok(());
            } else {
                return Err(ShelleyMA(WrongSignature));
            }
        }
    }
    Err(ShelleyMA(MissingVKWitness))
}

fn check_native_script_witness(
    script_hash: &ScriptHash,
    wits: &Option<Vec<NativeScript>>,
) -> ValidationResult {
    match wits {
        Some(scripts) => {
            for script in scripts.iter() {
                if compute_script_hash(script) == *script_hash {
                    return Ok(());
                }
            }
            Err(ShelleyMA(MissingScriptWitness))
        }
        None => Err(ShelleyMA(MissingScriptWitness)),
    }
}

fn check_minting(tx_body: &TransactionBody, mtx: &Tx) -> ValidationResult {
    match &tx_body.mint {
        Some(minted_value) => {
            let native_script_wits: Vec<NativeScript> =
                match &mtx.transaction_witness_set.native_script {
                    None => Vec::new(),
                    Some(keep_raw_native_script_wits) => keep_raw_native_script_wits
                        .iter()
                        .map(|x| x.clone().unwrap())
                        .collect(),
                };
            for policy in minted_value.keys() {
                if native_script_wits
                    .iter()
                    .all(|script| compute_script_hash(script) != *policy)
                {
                    return Err(ShelleyMA(MintingLacksPolicy));
                }
            }
            Ok(())
        }
        None => Ok(()),
    }
}

fn compute_script_hash(script: &NativeScript) -> PolicyId {
    let mut payload = Vec::new();
    let _ = encode(script, &mut payload);
    payload.insert(0, 0);
    pallas_crypto::hash::Hasher::<224>::hash(&payload)
}

// Checks all certificates in order, and counts the relevant ones for computing
// deposits.
#[allow(clippy::too_many_arguments)]
fn check_certificates(
    cert_opt: &Option<Vec<Certificate>>,
    tx_ix: TransactionIndex,
    cert_state: &mut CertState,
    stk_dep_count: &mut u64,
    stk_refund_count: &mut u64,
    pool_count: &mut u64,
    acnt: &AccountState,
    slot: &Slot,
    stab_win: &Slot,
    prot_pps: &ShelleyProtParams,
) -> ValidationResult {
    if let Some(certs) = cert_opt {
        let genesis = &GenesisValues::mainnet();
        let cepoch: Epoch = to_epoch(genesis, slot);
        let mpc: Coin = prot_pps.min_pool_cost;
        let mut ptr = CertPointer {
            slot: *slot,
            tx_ix,
            cert_ix: 0,
        };
        for (ix, cert) in certs.iter().enumerate() {
            match cert {
                StakeRegistration(stc) => {
                    *stk_dep_count += 1;
                    check_stake_registration(stc, &ptr, &mut cert_state.dstate)?;
                }
                StakeDeregistration(stc) => {
                    check_stake_deregistration(stc, &mut cert_state.dstate)?;
                    *stk_refund_count += 1;
                }
                StakeDelegation(stc, pk) => {
                    check_stake_delegation(stc, pk, &mut cert_state.dstate, &cert_state.pstate)?;
                }
                PoolRegistration {
                    operator,
                    vrf_keyhash,
                    pledge,
                    cost,
                    margin,
                    reward_account,
                    pool_owners,
                    relays,
                    pool_metadata,
                } => {
                    if !cert_state.pstate.pool_params.contains_key(operator) {
                        *pool_count += 1;
                    }
                    let pool_param = PoolParam {
                        vrf_keyhash: *vrf_keyhash,
                        pledge: *pledge,
                        cost: *cost,
                        margin: margin.clone(),
                        reward_account: reward_account.clone(),
                        pool_owners: pool_owners.clone(),
                        relays: relays.clone(),
                        pool_metadata: pool_metadata.clone(),
                    };
                    check_pool_reg_or_update(operator, &pool_param, &mpc, &mut cert_state.pstate)?;
                }
                PoolRetirement(pk, repoch) => {
                    check_pool_retirement(
                        pk,
                        repoch,
                        &cepoch,
                        &prot_pps.maximum_epoch,
                        &mut cert_state.pstate,
                    )?;
                }
                GenesisKeyDelegation(gkh, dkh, vrf) => {
                    check_genesis_key_delegation(
                        gkh,
                        dkh,
                        vrf,
                        slot,
                        stab_win,
                        &mut cert_state.dstate,
                    )?;
                }
                MoveInstantaneousRewardsCert(mir) => {
                    check_mir(mir, slot, stab_win, &mut cert_state.dstate, acnt)?;
                }
            }
            ptr.cert_ix = ix as u32; // FIXME: Careful here, `ix` is `usize`
        }
        Ok(())
    } else {
        Ok(())
    }
}

fn check_stake_registration(
    stc: &StakeCredential,
    ptr: &CertPointer,
    ds: &mut DState,
) -> ValidationResult {
    insert_or_err(
        &mut ds.rewards,
        stc,
        &0_u64,
        ShelleyMA(KeyAlreadyRegistered),
    )?;
    insert_or_err(&mut ds.ptrs, ptr, stc, ShelleyMA(PointerInUse))
}

fn check_stake_deregistration(stc: &StakeCredential, ds: &mut DState) -> ValidationResult {
    match ds.rewards.get(stc) {
        None => Err(ShelleyMA(KeyNotRegistered)),
        Some(0) => {
            ds.ptrs.retain(|_, v| v != stc);
            ds.delegations.remove(stc);
            ds.rewards.remove(stc);
            Ok(())
        }
        Some(_) => Err(ShelleyMA(RewardsNotNull)),
    }
}

fn check_stake_delegation(
    stc: &StakeCredential,
    pk: &PoolKeyhash,
    ds: &mut DState,
    ps: &PState,
) -> ValidationResult {
    if !ps.pool_params.contains_key(pk) {
        Err(ShelleyMA(PoolNotRegistered))
    } else if ds.rewards.contains_key(stc) {
        ds.delegations.insert(stc.clone(), *pk);
        Ok(())
    } else {
        Err(ShelleyMA(KeyNotRegistered))
    }
}

// Inserts a key-value pair if the key is not already in use, otherwise return
// the provided error.
fn insert_or_err<K, V, E>(map: &mut HashMap<K, V>, key: &K, value: &V, error: E) -> Result<(), E>
where
    K: Eq,
    K: std::hash::Hash,
    K: Clone,
    V: Clone,
{
    if map.contains_key(key) {
        Err(error)
    } else {
        map.insert(key.clone(), value.clone());
        Ok(())
    }
}

fn check_pool_reg_or_update(
    pool_hash: &PoolKeyhash,
    pool_param: &PoolParam,
    min_pool_cost: &Coin,
    ps: &mut PState,
) -> ValidationResult {
    if pool_param.cost < *min_pool_cost {
        Err(ShelleyMA(PoolCostBelowMin))
    } else if ps.pool_params.contains_key(pool_hash) {
        // Updating
        ps.fut_pool_params.insert(*pool_hash, (*pool_param).clone());
        ps.retiring.remove(pool_hash);
        Ok(())
    } else {
        // Registering
        ps.pool_params.insert(*pool_hash, (*pool_param).clone());
        Ok(())
    }
}

fn check_pool_retirement(
    pool_hash: &PoolKeyhash,
    repoch: &Epoch,
    cepoch: &Epoch,
    emax: &Epoch,
    ps: &mut PState,
) -> ValidationResult {
    if !ps.pool_params.contains_key(pool_hash) {
        return Err(ShelleyMA(PoolNotRegistered));
    }
    if (*cepoch < *repoch) & (*repoch <= *cepoch + *emax) {
        ps.retiring.insert(*pool_hash, *repoch);
        Ok(())
    } else {
        Err(ShelleyMA(PoolNotRegistered))
    }
}

fn check_genesis_key_delegation(
    gkh: &Genesishash,
    dkh: &GenesisDelegateHash, // called `vkh` in specs
    vrf: &VrfKeyhash,
    slot: &Slot,
    stab_win: &Slot,
    ds: &mut DState,
) -> ValidationResult {
    let cod = ds
        .gen_delegs
        .iter()
        .filter(|kv| kv.0 != gkh)
        .map(|kv| kv.1)
        .collect::<Vec<_>>();
    let fod = ds
        .fut_gen_delegs
        .iter()
        .filter(|kv| kv.0.1 != *gkh)
        .map(|kv| kv.1)
        .collect::<Vec<_>>();
    let curr_keyhashes = cod.iter().map(|v| v.0.clone()).collect::<Vec<_>>();
    let curr_vrfs = cod.iter().map(|v| v.1).collect::<Vec<_>>();
    let fut_keyhashes = fod.iter().map(|v| v.0.clone()).collect::<Vec<_>>();
    let fut_vrfs = fod.iter().map(|v| v.1).collect::<Vec<_>>();
    if curr_keyhashes.contains(dkh)
        | fut_keyhashes.contains(dkh)
        | curr_vrfs.contains(vrf)
        | fut_vrfs.contains(vrf)
    {
        Err(ShelleyMA(DuplicateGenesisDelegate))
    } else if !ds.gen_delegs.contains_key(gkh) {
        Err(ShelleyMA(GenesisKeyNotInMapping))
    } else {
        let gen_slot: Slot = *slot + *stab_win;
        ds.fut_gen_delegs
            .insert((gen_slot, gkh.clone()), (dkh.clone(), *vrf));
        Ok(())
    }
}

fn check_mir(
    mir: &MoveInstantaneousReward,
    slot: &Slot,
    stab_win: &Slot,
    ds: &mut DState,
    acnt: &AccountState,
) -> ValidationResult {
    let genesis = &GenesisValues::mainnet();
    if *slot >= first_slot(genesis, &(to_epoch(genesis, slot) + 1)) - *stab_win {
        Err(ShelleyMA(MIRCertificateTooLateinEpoch))
    } else {
        let (ir_reserves, ir_treasury) = ds.inst_rewards.clone();
        let (pot, ir_pot) = match mir.source {
            Reserves => (acnt.reserves, ir_reserves.clone()),
            Treasury => (acnt.treasury, ir_treasury.clone()),
        };
        let mut combined: HashMap<StakeCredential, Coin> = HashMap::new();
        if let StakeCredentials(kvp) = &mir.target {
            let mut kvv: Vec<(StakeCredential, u64)> = // TODO: Err if the value is negative
                kvp.iter().map(|kv| (kv.0.clone(), *kv.1 as u64)).collect();
            kvv.extend(ir_pot);
            for (key, value) in kvv {
                combined.insert(key, value);
            }
        }
        if combined.iter().map(|kv| kv.1).sum::<u64>() > pot {
            return Err(ShelleyMA(InsufficientForInstantaneousRewards));
        } else {
            ds.inst_rewards = match mir.source {
                Reserves => (combined, ir_reserves),
                Treasury => (ir_treasury, combined),
            }
        };
        Ok(())
    }
}

#[inline]
// Called just `epoch` in specs
fn to_epoch(genesis: &GenesisValues, slot: &Slot) -> Epoch {
    genesis.absolute_slot_to_relative(*slot).0
}

#[inline]
// CamelCase in specs
fn first_slot(genesis: &GenesisValues, epoch: &Epoch) -> Slot {
    genesis.relative_slot_to_absolute(*epoch, 0)
}

fn check_native_scripts(
    vkey_wits: &Vec<VKeyWitness>, // changed from alonzo
    native_scripts: &Vec<NativeScript>,
    low_bnd: &Option<u64>,
    upp_bnd: &Option<u64>,
) -> ValidationResult {
    for native_script in native_scripts {
        if !eval_native_script(vkey_wits, native_script, low_bnd, upp_bnd) {
            return Err(ShelleyMA(ScriptDenial));
        }
    }
    Ok(())
}

// Evaluates each witness or reference native script that a certificate needs.
pub(super) fn check_certificate_native_scripts(
    tx: &MultiEraTx,
    utxos: &UTxOs,
    denial: ValidationError,
) -> ValidationResult {
    let needed = needed_certificate_scripts(tx);
    let references = reference_scripts(tx, utxos);
    let witnesses = tx
        .native_scripts()
        .iter()
        .map(|script| (compute_native_script_hash(script), script.deref()));
    let referenced = references.iter().filter_map(|script| match script {
        ScriptRef::NativeScript(script) => Some((script.original_hash(), script.deref())),
        _ => None,
    });
    let vkey_wits = tx.vkey_witnesses().to_vec();
    for (hash, script) in witnesses.chain(referenced) {
        if needed.contains(&hash)
            && !eval_native_script(&vkey_wits, script, &tx.validity_start(), &tx.ttl())
        {
            return Err(denial);
        }
    }
    Ok(())
}

fn eval_native_script(
    vkey_wits: &Vec<VKeyWitness>, // changed from alonzo
    native_script: &NativeScript,
    low_bnd: &Option<u64>,
    upp_bnd: &Option<u64>,
) -> bool {
    match native_script {
        NativeScript::ScriptAll(scripts) => scripts
            .iter()
            .all(|scr| eval_native_script(vkey_wits, scr, low_bnd, upp_bnd)),
        NativeScript::ScriptAny(scripts) => scripts
            .iter()
            .any(|scr| eval_native_script(vkey_wits, scr, low_bnd, upp_bnd)),
        NativeScript::ScriptPubkey(hash) => vkey_wits
            .iter()
            .any(|vkey_wit| PallasHasher::<224>::hash(&vkey_wit.vkey.clone()) == *hash),
        NativeScript::ScriptNOfK(val, scripts) => {
            let count = scripts
                .iter()
                .map(|scr| eval_native_script(vkey_wits, scr, low_bnd, upp_bnd))
                .fold(0i64, |x, y| x + y as i64);
            count >= *val
        }
        NativeScript::InvalidBefore(val) => {
            match low_bnd {
                // The script is satisfied when the transaction cannot be applied
                // before `val`, i.e. its validity interval starts at or after it.
                Some(time) => val <= time,
                None => false, // as per mary-ledger.pdf, p.20
            }
        }
        NativeScript::InvalidHereafter(val) => {
            match upp_bnd {
                // The script is satisfied when the transaction cannot be applied
                // at or after `val`, i.e. its validity interval ends at or before it.
                Some(time) => val >= time,
                None => false, // as per mary-ledger.pdf, p.20
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Certificate, Era, MultiEraInput, MultiEraOutput, NativeScript, OriginalHash, ScriptHash,
        StakeCredential, TransactionOutput, Tx, UTxOs, VKeyWitness, Value, WitnessSet,
        check_witnesses, compute_script_hash, eval_native_script,
    };
    use crate::phase1::fixtures::{
        byron_address, byron_era_output, byron_output, key_address, legacy_output, reward_address,
        tx_spending_input, undecodable_byron_address, utxos,
    };
    use pallas_addresses::{
        ByronAddress,
        byron::{AddrType, AddressPayload, SpendingData},
    };
    use pallas_codec::minicbor;
    use pallas_crypto::{hash::Hasher, key::ed25519::SecretKey};
    use pallas_primitives::alonzo::{AddrKeyhash, TransactionInput};
    use pallas_traverse::MultiEraBlock;
    use std::ops::Deref;

    // Evaluates `native_script` with no vkey witnesses and the given validity
    // interval bounds (`low_bnd` is `validity_interval_start`, `upp_bnd` is `ttl`).
    fn eval(native_script: &NativeScript, low_bnd: Option<u64>, upp_bnd: Option<u64>) -> bool {
        eval_native_script(&Vec::new(), native_script, &low_bnd, &upp_bnd)
    }

    #[test]
    // `InvalidBefore(n)` holds only when the transaction's validity interval
    // starts at or after slot `n` (mary-ledger.pdf, p.20).
    fn invalid_before() {
        let script = NativeScript::InvalidBefore(100);
        assert!(eval(&script, Some(150), None), "starts after the lock");
        assert!(eval(&script, Some(100), None), "starts exactly at the lock");
        assert!(!eval(&script, Some(50), None), "starts before the lock");
        assert!(!eval(&script, None, None), "no lower bound");
    }

    #[test]
    // `InvalidHereafter(n)` holds only when the transaction's validity interval
    // ends at or before slot `n` (mary-ledger.pdf, p.20).
    fn invalid_hereafter() {
        let script = NativeScript::InvalidHereafter(200);
        assert!(eval(&script, None, Some(150)), "ends before the lock");
        assert!(eval(&script, None, Some(200)), "ends exactly at the lock");
        assert!(!eval(&script, None, Some(250)), "ends after the lock");
        assert!(!eval(&script, None, None), "no upper bound");
    }

    #[test]
    // A threshold <= 0 is satisfied by any witness count, including none: the
    // ledger CDDL types it int64, not uint (see `ScriptNOfK`'s doc comment).
    fn n_of_k_threshold() {
        assert!(eval(&NativeScript::ScriptNOfK(-1, vec![]), None, None));
        assert!(eval(&NativeScript::ScriptNOfK(0, vec![]), None, None));
        assert!(!eval(
            &NativeScript::ScriptNOfK(1, vec![NativeScript::InvalidBefore(100)]),
            None,
            None
        ));
    }

    #[test]
    // Recursion combinators must thread the bounds down to nested timelocks.
    fn nested_timelocks() {
        let all = NativeScript::ScriptAll(vec![
            NativeScript::InvalidBefore(100),
            NativeScript::InvalidHereafter(200),
        ]);
        assert!(
            eval(&all, Some(120), Some(180)),
            "interval within the window"
        );
        assert!(!eval(&all, Some(80), Some(180)), "starts too early");
        assert!(!eval(&all, Some(120), Some(220)), "ends too late");

        let any = NativeScript::ScriptAny(vec![
            NativeScript::InvalidBefore(100),
            NativeScript::InvalidHereafter(200),
        ]);
        assert!(
            eval(&any, Some(80), Some(180)),
            "satisfies the hereafter arm"
        );
        assert!(!eval(&any, Some(80), Some(220)), "satisfies neither arm");
    }

    // Re-encodes and decodes the edited transaction, so the body hash the keys
    // sign is the hash of the edited body.
    fn witnesses_outcome(
        certificates: Vec<Certificate>,
        native_scripts: Vec<NativeScript>,
        keys: &[u8],
        corrupted: Option<usize>,
    ) -> String {
        spending_outcome(&[], certificates, native_scripts, keys, corrupted)
    }

    // Spends one output locked by each of `spent` scripts.
    fn spending_outcome(
        spent: &[ScriptHash],
        certificates: Vec<Certificate>,
        native_scripts: Vec<NativeScript>,
        keys: &[u8],
        corrupted: Option<usize>,
    ) -> String {
        let inputs: Vec<TransactionInput> = (0..spent.len())
            .map(|index| TransactionInput {
                transaction_id: [5; 32].into(),
                index: index as u64,
            })
            .collect();
        let outputs: Vec<TransactionOutput> = spent
            .iter()
            .map(|hash| TransactionOutput {
                address: [&[0x71][..], hash.as_ref()].concat().into(),
                amount: Value::Coin(10),
                datum_hash: None,
            })
            .collect();
        let utxos: UTxOs = inputs
            .iter()
            .zip(&outputs)
            .map(|(input, output)| {
                (
                    MultiEraInput::from_alonzo_compatible(input),
                    MultiEraOutput::from_alonzo_compatible(output, Era::Shelley),
                )
            })
            .collect();
        let raw = hex::decode(include_str!("../../../test_data/shelley1.tx").trim()).unwrap();
        let mut tx: Tx = minicbor::decode(&raw).unwrap();
        tx.transaction_body.inputs = inputs.clone();
        tx.transaction_body.certificates = Some(certificates);
        tx.transaction_witness_set = minicbor::decode::<WitnessSet>(&[0xa0]).unwrap().into();
        tx.transaction_witness_set.native_script =
            Some(native_scripts.into_iter().map(Into::into).collect());
        let encoded = minicbor::to_vec(tx).unwrap();
        let mut tx: Tx = minicbor::decode(&encoded).unwrap();
        let hash = tx.transaction_body.original_hash();
        let mut witnesses: Vec<VKeyWitness> = keys
            .iter()
            .map(|id| {
                let key = SecretKey::from([*id; 32]);
                VKeyWitness {
                    vkey: key.public_key().as_ref().to_vec().into(),
                    signature: key.sign(hash).as_ref().to_vec().into(),
                }
            })
            .collect();
        if let Some(position) = corrupted {
            witnesses[position].signature = vec![0; 64].into();
        }
        tx.transaction_witness_set.vkeywitness = Some(witnesses);
        let actual = check_witnesses(&tx.transaction_body, &tx.transaction_witness_set, &utxos);
        format!("{actual:?}")
    }

    fn key_hash(id: u8) -> AddrKeyhash {
        Hasher::<224>::hash(SecretKey::from([id; 32]).public_key().as_ref())
    }

    fn credential_certificates(cred: StakeCredential) -> Vec<Certificate> {
        vec![
            Certificate::StakeDeregistration(cred.clone()),
            Certificate::StakeDelegation(cred, [9; 28].into()),
        ]
    }

    #[test]
    fn every_vkey_witness_must_verify() {
        for (corrupted, expected) in [
            (None, "Ok(())"),
            (Some(0), "Err(ShelleyMA(WrongSignature))"),
            (Some(1), "Err(ShelleyMA(WrongSignature))"),
        ] {
            assert_eq!(
                witnesses_outcome(vec![], vec![], &[1, 2], corrupted),
                expected
            );
        }
    }

    #[test]
    fn key_certificates_require_their_key_and_valid_signature() {
        let cred = StakeCredential::AddrKeyhash(key_hash(1));
        assert_eq!(
            witnesses_outcome(
                vec![Certificate::StakeRegistration(cred.clone())],
                vec![],
                &[],
                None
            ),
            "Ok(())"
        );
        for cert in credential_certificates(cred) {
            for (keys, corrupted, expected) in [
                (&[1][..], None, "Ok(())"),
                (&[][..], None, "Err(ShelleyMA(MissingVKWitness))"),
                (&[2][..], None, "Err(ShelleyMA(MissingVKWitness))"),
                (&[1][..], Some(0), "Err(ShelleyMA(WrongSignature))"),
            ] {
                assert_eq!(
                    witnesses_outcome(vec![cert.clone()], vec![], keys, corrupted),
                    expected
                );
            }
        }
    }

    #[test]
    fn pool_certificates_require_operator_and_registration_owners() {
        let registration = Certificate::PoolRegistration {
            operator: key_hash(1),
            vrf_keyhash: [0; 32].into(),
            pledge: 0,
            cost: 0,
            margin: pallas_primitives::UnitInterval {
                numerator: 0,
                denominator: 1,
            },
            reward_account: vec![0xe1; 29].into(),
            pool_owners: vec![key_hash(2)],
            relays: vec![],
            pool_metadata: None,
        };
        for (cert, keys) in [
            (registration, vec![1, 2]),
            (Certificate::PoolRetirement(key_hash(1), 1), vec![1]),
        ] {
            assert_eq!(
                witnesses_outcome(vec![cert.clone()], vec![], &keys, None),
                "Ok(())"
            );
            for missing in 0..keys.len() {
                let mut incomplete = keys.clone();
                incomplete.remove(missing);
                assert_eq!(
                    witnesses_outcome(vec![cert.clone()], vec![], &incomplete, None),
                    "Err(ShelleyMA(MissingVKWitness))"
                );
            }
        }
    }

    #[test]
    fn script_certificates_require_their_script() {
        let script = NativeScript::ScriptPubkey(key_hash(1));
        let other = NativeScript::ScriptAll(vec![]);
        let cred = StakeCredential::ScriptHash(compute_script_hash(&script));
        assert_eq!(
            witnesses_outcome(
                vec![Certificate::StakeRegistration(cred.clone())],
                vec![],
                &[],
                None
            ),
            "Ok(())"
        );
        for cert in credential_certificates(cred) {
            for (scripts, keys, expected) in [
                (vec![script.clone()], &[1][..], "Ok(())"),
                (vec![], &[1][..], "Err(ShelleyMA(MissingScriptWitness))"),
                (
                    vec![other.clone()],
                    &[1][..],
                    "Err(ShelleyMA(MissingScriptWitness))",
                ),
                (
                    vec![script.clone()],
                    &[2][..],
                    "Err(ShelleyMA(ScriptDenial))",
                ),
            ] {
                assert_eq!(
                    witnesses_outcome(vec![cert.clone()], scripts, keys, None),
                    expected
                );
            }
        }
    }

    #[test]
    fn every_needed_script_is_found_in_any_position() {
        let first = NativeScript::ScriptPubkey(key_hash(1));
        let second = NativeScript::ScriptPubkey(key_hash(2));
        let hashes = [compute_script_hash(&first), compute_script_hash(&second)];
        let both = vec![first.clone(), second.clone()];
        let certificate = Certificate::StakeDeregistration(StakeCredential::ScriptHash(hashes[1]));
        for (spent, certificates, scripts, expected) in [
            (&hashes[..], vec![], both.clone(), "Ok(())"),
            (
                &hashes[..1],
                vec![certificate.clone()],
                both.clone(),
                "Ok(())",
            ),
            (
                &hashes[..],
                vec![],
                vec![first.clone()],
                "Err(ShelleyMA(MissingScriptWitness))",
            ),
            (
                &hashes[..1],
                vec![certificate],
                vec![first],
                "Err(ShelleyMA(MissingScriptWitness))",
            ),
        ] {
            assert_eq!(
                spending_outcome(spent, certificates, scripts, &[1, 2], None),
                expected
            );
        }
    }

    #[test]
    fn an_input_at_a_reward_or_undecodable_byron_address_is_refused() {
        let cbor = tx_spending_input();
        let mut tx: Tx = minicbor::decode(&cbor).unwrap();
        tx.transaction_witness_set.vkeywitness = Some(vec![]);
        let outcomes: Vec<_> = [
            (Era::Shelley, legacy_output(key_address())),
            (Era::Shelley, legacy_output(byron_address())),
            (Era::Byron, byron_output()),
            (Era::Shelley, legacy_output(reward_address())),
            (Era::Shelley, legacy_output(undecodable_byron_address())),
        ]
        .map(|(era, raw)| {
            format!(
                "{:?}",
                check_witnesses(
                    &tx.transaction_body,
                    &tx.transaction_witness_set,
                    &utxos(era, &raw)
                )
            )
        })
        .into();
        assert_eq!(
            outcomes,
            [
                "Err(ShelleyMA(MissingVKWitness))",
                "Err(ShelleyMA(MissingVKWitness))",
                "Err(ShelleyMA(MissingVKWitness))",
                "Err(ShelleyMA(AddressDecoding))",
                "Err(ShelleyMA(AddressDecoding))",
            ]
        );
    }

    #[test]
    fn mainnet_bootstrap_witnesses_cover_their_byron_inputs() {
        // The second transaction of the block spends two Byron inputs with two
        // bootstrap witnesses and no key witness.
        let raw = hex::decode(include_str!("../../../test_data/mary1.block").trim()).unwrap();
        let block = MultiEraBlock::decode(&raw).unwrap();
        let txs = block.txs();
        let tx = txs[1].as_alonzo().unwrap();
        let real = tx
            .transaction_witness_set
            .bootstrap_witness
            .clone()
            .unwrap();
        let addresses: Vec<Vec<u8>> = real
            .iter()
            .map(|wit| {
                let xpub = [wit.public_key.as_slice(), wit.chain_code.as_slice()].concat();
                let attributes = minicbor::decode(&wit.attributes).unwrap();
                let spending = SpendingData::PubKey(xpub.into());
                ByronAddress::from(AddressPayload::new(AddrType::PubKey, spending, attributes))
                    .to_vec()
            })
            .collect();
        let hash = tx.transaction_body.original_hash();
        let other_key = SecretKey::from([8; 32]);
        let mut other = real[1].clone();
        other.public_key = other_key.public_key().as_ref().to_vec().into();
        other.signature = other_key.sign(hash).as_ref().to_vec().into();
        let mut tampered = real[0].clone();
        let mut signature = tampered.signature.to_vec();
        signature[0] ^= 1;
        tampered.signature = signature.into();
        let cases = [
            (Some(real.clone()), "Ok(())"),
            (None, "Err(ShelleyMA(MissingVKWitness))"),
            (
                Some(vec![real[0].clone(), other]),
                "Err(ShelleyMA(MissingVKWitness))",
            ),
            (
                Some(vec![tampered, real[1].clone()]),
                "Err(ShelleyMA(WrongSignature))",
            ),
        ];
        let mut expected = vec![];
        let mut actual = vec![];
        for era in [Era::Byron, Era::Shelley] {
            let outputs: Vec<Vec<u8>> = addresses
                .iter()
                .map(|address| match era {
                    Era::Byron => byron_era_output(address),
                    _ => legacy_output(address.clone()),
                })
                .collect();
            let utxos: UTxOs = tx
                .transaction_body
                .inputs
                .iter()
                .zip(&outputs)
                .map(|(input, output)| {
                    (
                        MultiEraInput::from_alonzo_compatible(input),
                        MultiEraOutput::decode(era, output).unwrap(),
                    )
                })
                .collect();
            for (witnesses, outcome) in &cases {
                let mut wits: WitnessSet = tx.transaction_witness_set.deref().clone();
                wits.bootstrap_witness = witnesses.clone();
                let result = check_witnesses(&tx.transaction_body, &wits, &utxos);
                expected.push(format!("{era:?} {outcome}"));
                actual.push(format!("{era:?} {result:?}"));
            }
        }
        assert_eq!(actual, expected);
    }
}
