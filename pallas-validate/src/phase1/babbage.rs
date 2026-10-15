//! Utilities required for Babbage-era transaction validation.

use super::{
    alonzo::{certificate_key_hashes, certificate_scripts},
    shelley_ma::check_needed_native_scripts,
};
use crate::utils::{
    BabbageProtParams,
    PostAlonzoError::*,
    UTxOs,
    ValidationError::{self, *},
    ValidationResult, add_minted_value, add_values, aux_data_from_babbage_tx, check_needed_script,
    check_remaining_vk_wits, compute_native_script_hash, compute_plutus_v1_script_hash,
    compute_plutus_v2_script_hash, empty_value, get_babbage_tx_size, get_lovelace_from_alonzo_val,
    get_script_hash_from_input, get_script_hash_from_reward_account, get_shelley_address,
    get_val_size_in_words, lovelace_diff_or_fail, mk_alonzo_vk_wits_check_list,
    output_language_features, values_are_equal, verify_signature, withdrawal_scripts,
};
use pallas_addresses::{Address, ScriptHash, ShelleyAddress, ShelleyPaymentPart};
use pallas_codec::{
    minicbor::{Encoder, encode},
    utils::{Bytes, KeepRaw},
};
use pallas_primitives::{
    AddrKeyhash, Hash, PlutusData, PolicyId, TransactionInput,
    alonzo::{RedeemerPointer, RedeemerTag},
    babbage::{
        Certificate, DatumOption, Language, Mint, Redeemer, RequiredSigners, ScriptRef,
        TransactionBody, TransactionOutput, Tx, VKeyWitness, Value, WitnessSet,
    },
};
use pallas_traverse::{ComputeHash, MultiEraInput, MultiEraOutput, OriginalHash};
use std::ops::Deref;

pub fn validate_babbage_tx(
    mtx: &Tx,
    utxos: &UTxOs,
    prot_pps: &BabbageProtParams,
    block_slot: &u64,
    network_magic: &u32,
    network_id: &u8,
) -> ValidationResult {
    let tx_body: &TransactionBody = &mtx.transaction_body.clone();
    let size: u32 = get_babbage_tx_size(mtx).ok_or(PostAlonzo(UnknownTxSize))?;
    check_ins_not_empty(tx_body)?;
    check_all_ins_in_utxos(tx_body, utxos)?;
    check_tx_validity_interval(tx_body, block_slot)?;
    check_fee(tx_body, &size, mtx, utxos, prot_pps)?;
    check_preservation_of_value(tx_body, utxos)?;
    check_min_lovelace(tx_body, prot_pps)?;
    check_output_val_size(tx_body, prot_pps)?;
    check_network_id(tx_body, network_id)?;
    check_tx_size(&size, prot_pps)?;
    check_tx_ex_units(mtx, prot_pps)?;
    check_minting(tx_body, mtx, utxos)?;
    check_well_formedness(tx_body, mtx)?;
    check_witness_set(mtx, utxos)?;
    check_languages(mtx, utxos, network_magic, network_id, block_slot)?;
    check_auxiliary_data(tx_body, mtx)?;
    check_script_data_hash(tx_body, mtx, utxos, network_magic, network_id, block_slot)
}

// The set of transaction inputs is not empty.
fn check_ins_not_empty(tx_body: &TransactionBody) -> ValidationResult {
    if tx_body.inputs.is_empty() {
        return Err(PostAlonzo(TxInsEmpty));
    }
    Ok(())
}

// All transaction inputs, collateral inputs and reference inputs are in the
// UTxO set.
fn check_all_ins_in_utxos(tx_body: &TransactionBody, utxos: &UTxOs) -> ValidationResult {
    for input in tx_body.inputs.iter() {
        if !(utxos.contains_key(&MultiEraInput::from_alonzo_compatible(input))) {
            return Err(PostAlonzo(InputNotInUTxO));
        }
    }
    match &tx_body.collateral {
        None => (),
        Some(collaterals) => {
            for collateral in collaterals {
                if !(utxos.contains_key(&MultiEraInput::from_alonzo_compatible(collateral))) {
                    return Err(PostAlonzo(CollateralNotInUTxO));
                }
            }
        }
    }
    match &tx_body.reference_inputs {
        None => (),
        Some(reference_inputs) => {
            for reference_input in reference_inputs {
                if !(utxos.contains_key(&MultiEraInput::from_alonzo_compatible(reference_input))) {
                    return Err(PostAlonzo(ReferenceInputNotInUTxO));
                }
            }
        }
    }
    Ok(())
}

// The block slot is contained in the transaction validity interval, and the
// upper bound is translatable to UTC time.
fn check_tx_validity_interval(tx_body: &TransactionBody, block_slot: &u64) -> ValidationResult {
    check_lower_bound(tx_body, *block_slot)?;
    check_upper_bound(tx_body, *block_slot)
}

// If defined, the lower bound of the validity time interval does not exceed the
// block slot.
fn check_lower_bound(tx_body: &TransactionBody, block_slot: u64) -> ValidationResult {
    match tx_body.validity_interval_start {
        Some(lower_bound) => {
            if block_slot < lower_bound {
                Err(PostAlonzo(BlockPrecedesValInt))
            } else {
                Ok(())
            }
        }
        None => Ok(()),
    }
}

// If defined, the upper bound of the validity time interval is not exceeded by
// the block slot, and it is translatable to UTC time.
fn check_upper_bound(tx_body: &TransactionBody, block_slot: u64) -> ValidationResult {
    match tx_body.ttl {
        Some(upper_bound) => {
            if upper_bound < block_slot {
                Err(PostAlonzo(BlockExceedsValInt))
            } else {
                // TODO: check that `upper_bound` is translatable to UTC time.
                Ok(())
            }
        }
        None => Ok(()),
    }
}

fn check_fee(
    tx_body: &TransactionBody,
    size: &u32,
    mtx: &Tx,
    utxos: &UTxOs,
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    check_min_fee(tx_body, size, prot_pps)?;
    if !option_vec_is_empty(&mtx.transaction_witness_set.redeemer) {
        check_collaterals(tx_body, utxos, prot_pps)?
    }
    Ok(())
}

// The fee paid by the transaction should be greater than or equal to the
// minimum fee.
fn check_min_fee(
    tx_body: &TransactionBody,
    size: &u32,
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    if tx_body.fee < (prot_pps.minfee_b + prot_pps.minfee_a * size) as u64 {
        return Err(PostAlonzo(FeeBelowMin));
    }
    Ok(())
}

fn check_collaterals(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    let collaterals: &[TransactionInput] = &tx_body
        .collateral
        .clone()
        .ok_or(PostAlonzo(CollateralMissing))?;
    check_collaterals_number(collaterals, prot_pps)?;
    check_collaterals_address(collaterals, utxos)?;
    check_collaterals_assets(tx_body, utxos, prot_pps)
}

// The set of collateral inputs is not empty.
// The number of collateral inputs is below maximum allowed by protocol.
fn check_collaterals_number(
    collaterals: &[TransactionInput],
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    if collaterals.is_empty() {
        Err(PostAlonzo(CollateralMissing))
    } else if collaterals.len() as u32 > prot_pps.max_collateral_inputs {
        Err(PostAlonzo(TooManyCollaterals))
    } else {
        Ok(())
    }
}

// Each collateral input refers to a verification-key address.
fn check_collaterals_address(collaterals: &[TransactionInput], utxos: &UTxOs) -> ValidationResult {
    for collateral in collaterals {
        match utxos.get(&MultiEraInput::from_alonzo_compatible(collateral)) {
            Some(multi_era_output) => {
                match multi_era_output
                    .address()
                    .map_err(|_| PostAlonzo(InputDecoding))?
                {
                    Address::Shelley(address)
                        if matches!(address.payment(), ShelleyPaymentPart::Key(_)) => {}
                    Address::Byron(_) => {}
                    Address::Shelley(_) => {
                        return Err(PostAlonzo(CollateralNotVKeyLocked));
                    }
                    _ => return Err(PostAlonzo(InputDecoding)),
                }
            }
            None => {
                return Err(PostAlonzo(CollateralNotInUTxO));
            }
        }
    }
    Ok(())
}

// The balance between collateral inputs and output contains only lovelace.
// The balance is not lower than the minimum allowed.
// The balance matches exactly the collateral annotated in the transaction body.
fn check_collaterals_assets(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    match &tx_body.collateral {
        Some(collaterals) => {
            let mut coll_input: Value = empty_value();
            for collateral in collaterals {
                match utxos.get(&MultiEraInput::from_alonzo_compatible(collateral)) {
                    Some(multi_era_output) => {
                        coll_input = add_values(
                            &coll_input,
                            &val_from_multi_era_output(multi_era_output),
                            &PostAlonzo(NegativeValue),
                        )?
                    }
                    None => {
                        return Err(PostAlonzo(CollateralNotInUTxO));
                    }
                }
            }
            let coll_return: Value = match &tx_body.collateral_return.as_deref() {
                Some(TransactionOutput::Legacy(output)) => output.amount.clone(),
                Some(TransactionOutput::PostAlonzo(output)) => output.value.clone(),
                None => Value::Coin(0),
            };
            // The balance between collateral inputs and output contains only lovelace.
            let paid_collateral: u64 = lovelace_diff_or_fail(
                &coll_input,
                &coll_return,
                &PostAlonzo(NonLovelaceCollateral),
            )?;
            let fee_percentage: u64 = tx_body.fee * prot_pps.collateral_percentage as u64;
            // The balance is not lower than the minimum allowed.
            if paid_collateral * 100 < fee_percentage {
                return Err(PostAlonzo(CollateralMinLovelace));
            }
            // The balance matches exactly the collateral annotated in the transaction body.
            if let Some(annotated_collateral) = &tx_body.total_collateral
                && paid_collateral != *annotated_collateral
            {
                return Err(PostAlonzo(CollateralAnnotation));
            }
        }
        None => return Err(PostAlonzo(CollateralMissing)),
    }
    Ok(())
}

fn val_from_multi_era_output(multi_era_output: &MultiEraOutput) -> Value {
    match multi_era_output {
        MultiEraOutput::Byron(output) => Value::Coin(output.amount),
        MultiEraOutput::AlonzoCompatible(output, _) => output.amount.clone(),
        babbage_output => match babbage_output.as_babbage() {
            Some(TransactionOutput::Legacy(output)) => output.amount.clone(),
            Some(TransactionOutput::PostAlonzo(output)) => output.value.clone(),
            None => unimplemented!(), /* If this is the case, then it must be that non-exhaustive
                                       * type MultiEraOutput was extended with another variant */
        },
    }
}

// The preservation of value property holds.
fn check_preservation_of_value(tx_body: &TransactionBody, utxos: &UTxOs) -> ValidationResult {
    let mut input: Value = get_consumed(tx_body, utxos)?;
    let produced: Value = get_produced(tx_body)?;
    let output: Value = add_values(
        &produced,
        &Value::Coin(tx_body.fee),
        &PostAlonzo(NegativeValue),
    )?;
    if let Some(m) = &tx_body.mint {
        input = add_minted_value(&input, m, &PostAlonzo(NegativeValue))?;
    }
    if !values_are_equal(&input, &output) {
        return Err(PostAlonzo(PreservationOfValue));
    }
    Ok(())
}

fn get_consumed(tx_body: &TransactionBody, utxos: &UTxOs) -> Result<Value, ValidationError> {
    let mut res: Value = empty_value();
    for input in tx_body.inputs.iter() {
        let multi_era_output: &MultiEraOutput = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(InputNotInUTxO))?;
        let val: Value = val_from_multi_era_output(multi_era_output);
        res = add_values(&res, &val, &PostAlonzo(NegativeValue))?;
    }
    Ok(res)
}

fn get_produced(tx_body: &TransactionBody) -> Result<Value, ValidationError> {
    let mut res: Value = empty_value();
    for output in tx_body.outputs.iter() {
        match output.deref() {
            TransactionOutput::Legacy(output) => {
                res = add_values(&res, &output.amount, &PostAlonzo(NegativeValue))?
            }
            TransactionOutput::PostAlonzo(output) => {
                res = add_values(&res, &output.value, &PostAlonzo(NegativeValue))?
            }
        }
    }
    Ok(res)
}

fn check_min_lovelace(tx_body: &TransactionBody, prot_pps: &BabbageProtParams) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let val: &Value = match output.deref() {
            TransactionOutput::Legacy(output) => &output.amount,
            TransactionOutput::PostAlonzo(output) => &output.value,
        };
        if get_lovelace_from_alonzo_val(val) < compute_min_lovelace(val, prot_pps) {
            return Err(PostAlonzo(MinLovelaceUnreached));
        }
    }
    Ok(())
}

fn compute_min_lovelace(val: &Value, prot_pps: &BabbageProtParams) -> u64 {
    prot_pps.ada_per_utxo_byte * (get_val_size_in_words(val) + 160)
}

// The size of the value in each of the outputs should not be greater than the
// maximum allowed.
fn check_output_val_size(
    tx_body: &TransactionBody,
    prot_pps: &BabbageProtParams,
) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let val: &Value = match output.deref() {
            TransactionOutput::Legacy(output) => &output.amount,
            TransactionOutput::PostAlonzo(output) => &output.value,
        };
        if get_val_size_in_words(val) > prot_pps.max_value_size as u64 {
            return Err(PostAlonzo(MaxValSizeExceeded));
        }
    }
    Ok(())
}

fn check_network_id(tx_body: &TransactionBody, network_id: &u8) -> ValidationResult {
    check_tx_outs_network_id(tx_body, network_id)?;
    check_tx_network_id(tx_body, network_id)
}

fn check_tx_outs_network_id(tx_body: &TransactionBody, network_id: &u8) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let addr_bytes: &Bytes = match output.deref() {
            TransactionOutput::Legacy(output) => &output.address,
            TransactionOutput::PostAlonzo(output) => &output.address,
        };
        let addr: ShelleyAddress =
            get_shelley_address(Bytes::deref(addr_bytes)).ok_or(PostAlonzo(AddressDecoding))?;
        if addr.network().value() != *network_id {
            return Err(PostAlonzo(OutputWrongNetworkID));
        }
    }
    Ok(())
}

// The network ID of the transaction body is either undefined or equal to the
// global network ID.
fn check_tx_network_id(tx_body: &TransactionBody, network_id: &u8) -> ValidationResult {
    if let Some(tx_network_id) = tx_body.network_id
        && u8::from(tx_network_id) != *network_id
    {
        return Err(PostAlonzo(TxWrongNetworkID));
    }
    Ok(())
}

fn check_tx_size(size: &u32, prot_pps: &BabbageProtParams) -> ValidationResult {
    if *size > prot_pps.max_transaction_size {
        return Err(PostAlonzo(MaxTxSizeExceeded));
    }
    Ok(())
}

fn check_tx_ex_units(mtx: &Tx, prot_pps: &BabbageProtParams) -> ValidationResult {
    let mut steps: u64 = 0;
    let mut mem: u64 = 0;
    for Redeemer { ex_units, .. } in mtx.transaction_witness_set.redeemer.iter().flatten() {
        mem += ex_units.mem;
        steps += ex_units.steps;
    }
    if mem > prot_pps.max_tx_ex_units.mem || steps > prot_pps.max_tx_ex_units.steps {
        return Err(PostAlonzo(TxExUnitsExceeded));
    }
    Ok(())
}

// Each minted / burned asset is paired with an appropriate native script or
// Plutus script.
fn check_minting(tx_body: &TransactionBody, mtx: &Tx, utxos: &UTxOs) -> ValidationResult {
    match &tx_body.mint {
        Some(minted_value) => {
            let native_script_wits = mtx
                .transaction_witness_set
                .native_script
                .iter()
                .flatten()
                .map(|x| compute_native_script_hash(x));

            let v1_scripts_wits = mtx
                .transaction_witness_set
                .plutus_v1_script
                .iter()
                .flatten()
                .map(compute_plutus_v1_script_hash);

            let v2_scripts_wits = mtx
                .transaction_witness_set
                .plutus_v2_script
                .iter()
                .flatten()
                .map(compute_plutus_v2_script_hash);

            let ref_scripts = reference_script_inputs(tx_body)
                .filter_map(|x| get_script_hash_from_reference_input(x, utxos));

            let all_scripts_wits: Vec<_> = native_script_wits
                .chain(v1_scripts_wits)
                .chain(v2_scripts_wits)
                .chain(ref_scripts)
                .collect();

            dbg!(&all_scripts_wits);

            for policy in minted_value.keys() {
                if !all_scripts_wits.contains(policy) {
                    return Err(PostAlonzo(MintingLacksPolicy(*policy)));
                }
            }
            Ok(())
        }
        None => Ok(()),
    }
}

fn check_well_formedness(_tx_body: &TransactionBody, _mtx: &Tx) -> ValidationResult {
    Ok(())
}

fn check_witness_set(mtx: &Tx, utxos: &UTxOs) -> ValidationResult {
    let tx_hash: &Vec<u8> = &Vec::from(mtx.transaction_body.original_hash().as_ref());
    let tx_body: &TransactionBody = &mtx.transaction_body;
    let tx_wits: &WitnessSet = &mtx.transaction_witness_set;
    let vkey_wits: &Option<Vec<VKeyWitness>> = &tx_wits.vkeywitness;
    let native_scripts: Vec<PolicyId> = match &tx_wits.native_script {
        Some(scripts) => scripts
            .clone()
            .iter()
            .map(|raw_script| compute_native_script_hash(raw_script))
            .collect(),
        None => Vec::new(),
    };
    let plutus_v1_scripts: Vec<PolicyId> = match &tx_wits.plutus_v1_script {
        Some(scripts) => scripts
            .clone()
            .iter()
            .map(compute_plutus_v1_script_hash)
            .collect(),
        None => Vec::new(),
    };
    let plutus_v2_scripts: Vec<PolicyId> = match &tx_wits.plutus_v2_script {
        Some(scripts) => scripts
            .clone()
            .iter()
            .map(compute_plutus_v2_script_hash)
            .collect(),
        None => Vec::new(),
    };
    let reference_scripts: Vec<PolicyId> = get_reference_script_hashes(tx_body, utxos);
    check_needed_scripts(
        tx_body,
        utxos,
        &native_scripts,
        &plutus_v1_scripts,
        &plutus_v2_scripts,
        &reference_scripts,
    )?;
    check_datums(tx_body, utxos, &tx_wits.plutus_data)?;
    // Native reference scripts satisfy script presence, but never need redeemers.
    let plutus_reference_scripts: Vec<PolicyId> = reference_script_inputs(tx_body)
        .filter_map(|input| reference_script(input, utxos))
        .filter(|script| !matches!(script, ScriptRef::NativeScript(_)))
        .map(script_ref_hash)
        .collect();
    check_redeemers(
        &plutus_v1_scripts,
        &plutus_v2_scripts,
        &plutus_reference_scripts,
        tx_body,
        tx_wits,
        utxos,
    )?;
    check_certificate_native_scripts(mtx, utxos)?;
    check_required_signers(&tx_body.required_signers, vkey_wits, tx_hash)?;
    check_vkey_input_wits(mtx, &tx_wits.vkeywitness, utxos)
}

// Each minting policy or script hash in a script input address can be matched
// to a script in the transaction witness set, except when it can be found in a
// reference input
fn check_needed_scripts(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    native_scripts: &[PolicyId],
    plutus_v1_scripts: &[PolicyId],
    plutus_v2_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
) -> ValidationResult {
    let mut filtered_native_scripts: Vec<(bool, PolicyId)> = native_scripts
        .iter()
        .map(|&script_hash| (false, script_hash))
        .collect();
    filtered_native_scripts
        .retain(|&(_, native_script_hash)| !reference_scripts.contains(&native_script_hash));
    let mut filtered_plutus_v1_scripts: Vec<(bool, PolicyId)> = plutus_v1_scripts
        .iter()
        .map(|&script_hash| (false, script_hash))
        .collect();
    filtered_plutus_v1_scripts
        .retain(|&(_, plutus_v1_script_hash)| !reference_scripts.contains(&plutus_v1_script_hash));
    let mut filtered_plutus_v2_scripts: Vec<(bool, PolicyId)> = plutus_v2_scripts
        .iter()
        .map(|&script_hash| (false, script_hash))
        .collect();
    filtered_plutus_v2_scripts
        .retain(|&(_, plutus_v2_script_hash)| !reference_scripts.contains(&plutus_v2_script_hash));
    check_input_scripts(
        tx_body,
        &mut filtered_native_scripts,
        &mut filtered_plutus_v1_scripts,
        &mut filtered_plutus_v2_scripts,
        reference_scripts,
        utxos,
    )?;
    check_minting_policies(
        tx_body,
        &mut filtered_native_scripts,
        &mut filtered_plutus_v1_scripts,
        &mut filtered_plutus_v2_scripts,
        reference_scripts,
    )?;
    for (_, hash) in certificate_scripts(certificates(tx_body)) {
        check_needed_script(
            hash,
            reference_scripts,
            &mut [
                &mut filtered_native_scripts,
                &mut filtered_plutus_v1_scripts,
                &mut filtered_plutus_v2_scripts,
            ],
            PostAlonzo(ScriptWitnessMissing),
        )?;
    }
    for (covered, _) in filtered_native_scripts.iter() {
        if !covered {
            return Err(PostAlonzo(UnneededNativeScript));
        }
    }
    for (covered, _) in filtered_plutus_v1_scripts.iter() {
        if !covered {
            return Err(PostAlonzo(UnneededPlutusV1Script));
        }
    }
    for (covered, _) in filtered_plutus_v2_scripts.iter() {
        if !covered {
            return Err(PostAlonzo(UnneededPlutusV2Script));
        }
    }
    Ok(())
}

fn get_reference_script_hashes(tx_body: &TransactionBody, utxos: &UTxOs) -> Vec<PolicyId> {
    reference_script_inputs(tx_body)
        .filter_map(|input| get_script_hash_from_reference_input(input, utxos))
        .collect()
}

fn certificates<'a>(tx_body: &'a TransactionBody) -> &'a [Certificate] {
    tx_body.certificates.as_deref().unwrap_or_default()
}

fn reference_script_inputs<'a>(
    body: &'a TransactionBody<'_>,
) -> impl Iterator<Item = &'a TransactionInput> {
    body.inputs
        .iter()
        .chain(body.reference_inputs.iter().flatten())
}

fn reference_script<'a>(
    input: &'a TransactionInput,
    utxos: &'a UTxOs<'_>,
) -> Option<&'a ScriptRef<'a>> {
    match utxos
        .get(&MultiEraInput::from_alonzo_compatible(input))?
        .as_babbage()?
    {
        TransactionOutput::PostAlonzo(output) => output.script_ref.as_ref().map(|script| &script.0),
        TransactionOutput::Legacy(_) => None,
    }
}

fn check_certificate_native_scripts(mtx: &Tx, utxos: &UTxOs) -> ValidationResult {
    let body = &mtx.transaction_body;
    let witnesses = &mtx.transaction_witness_set;
    let needed: Vec<ScriptHash> = certificate_scripts(certificates(body))
        .map(|(_, hash)| *hash)
        .collect();
    let scripts = witnesses
        .native_script
        .iter()
        .flatten()
        .map(|script| (compute_native_script_hash(script), script.deref()))
        .chain(reference_script_inputs(body).filter_map(|input| {
            match reference_script(input, utxos) {
                Some(ScriptRef::NativeScript(script)) => {
                    Some((script.original_hash(), script.deref()))
                }
                _ => None,
            }
        }));
    check_needed_native_scripts(
        scripts,
        &needed,
        &witnesses.vkeywitness.clone().unwrap_or_default(),
        &body.validity_interval_start,
        &body.ttl,
        PostAlonzo(NativeScriptDenial),
    )
}

fn check_input_scripts(
    tx_body: &TransactionBody,
    native_scripts: &mut [(bool, PolicyId)],
    plutus_v1_scripts: &mut [(bool, PolicyId)],
    plutus_v2_scripts: &mut [(bool, PolicyId)],
    reference_scripts: &[PolicyId],
    utxos: &UTxOs,
) -> ValidationResult {
    let mut needed_input_scripts: Vec<(bool, ScriptHash)> =
        get_script_hashes_from_inputs(tx_body, utxos);
    for (covered, hash) in &mut needed_input_scripts {
        for (native_script_covered, native_script_hash) in native_scripts.iter_mut() {
            if *hash == *native_script_hash {
                *covered = true;
                *native_script_covered = true;
            }
        }
        for (plutus_v1_script_covered, plutus_v1_script_hash) in plutus_v1_scripts.iter_mut() {
            if *hash == *plutus_v1_script_hash {
                *covered = true;
                *plutus_v1_script_covered = true;
            }
        }
        for (plutus_v2_script_covered, plutus_v2_script_hash) in plutus_v2_scripts.iter_mut() {
            if *hash == *plutus_v2_script_hash {
                *covered = true;
                *plutus_v2_script_covered = true;
            }
        }
    }
    for (covered, hash) in needed_input_scripts {
        if !covered && !reference_scripts.contains(&hash) {
            return Err(PostAlonzo(ScriptWitnessMissing));
        }
    }
    Ok(())
}

fn get_script_hashes_from_inputs(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
) -> Vec<(bool, ScriptHash)> {
    let mut res: Vec<(bool, ScriptHash)> = Vec::new();
    for input in tx_body.inputs.iter() {
        if let Some(script_hash) = get_script_hash_from_input(input, utxos) {
            res.push((false, script_hash))
        }
    }
    res
}

fn get_script_hash_from_reference_input(
    ref_input: &TransactionInput,
    utxos: &UTxOs,
) -> Option<PolicyId> {
    reference_script(ref_input, utxos).map(script_ref_hash)
}

fn script_ref_hash(script: &ScriptRef) -> PolicyId {
    match script {
        ScriptRef::NativeScript(script) => script.original_hash(),
        ScriptRef::PlutusV1Script(script) => script.compute_hash(),
        ScriptRef::PlutusV2Script(script) => script.compute_hash(),
    }
}

fn check_minting_policies(
    tx_body: &TransactionBody,
    native_scripts: &mut [(bool, PolicyId)],
    plutus_v1_scripts: &mut [(bool, PolicyId)],
    plutus_v2_scripts: &mut [(bool, PolicyId)],
    reference_scripts: &[PolicyId],
) -> ValidationResult {
    match &tx_body.mint {
        None => Ok(()),
        Some(minted_value) => {
            let mut minting_policies: Vec<(bool, PolicyId)> =
                minted_value.keys().map(|pol| (false, *pol)).collect();
            for (policy_covered, policy) in &mut minting_policies {
                for (native_script_covered, native_script_hash) in native_scripts.iter_mut() {
                    if *policy == *native_script_hash {
                        *policy_covered = true;
                        *native_script_covered = true;
                    }
                }
                for (plutus_script_covered, plutus_v1_script_hash) in plutus_v1_scripts.iter_mut() {
                    if *policy == *plutus_v1_script_hash {
                        *policy_covered = true;
                        *plutus_script_covered = true;
                    }
                }
                for (plutus_script_covered, plutus_v2_script_hash) in plutus_v2_scripts.iter_mut() {
                    if *policy == *plutus_v2_script_hash {
                        *policy_covered = true;
                        *plutus_script_covered = true;
                    }
                }
                for reference_script_hash in reference_scripts.iter() {
                    if *policy == *reference_script_hash {
                        *policy_covered = true;
                    }
                }
            }
            for (policy_covered, policy) in minting_policies {
                if !policy_covered {
                    return Err(PostAlonzo(MintingLacksPolicy(policy)));
                }
            }
            Ok(())
        }
    }
}

// Each datum hash in a Plutus script input matches the hash of a datum in the
// transaction witness set
fn check_datums(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    option_plutus_data: &Option<Vec<KeepRaw<PlutusData>>>,
) -> ValidationResult {
    let mut plutus_data_hashes: Vec<(bool, Hash<32>)> = match option_plutus_data {
        Some(plutus_data) => plutus_data
            .iter()
            .map(|datum| {
                (
                    false,
                    pallas_crypto::hash::Hasher::<256>::hash(datum.raw_cbor()),
                )
            })
            .collect(),
        None => Vec::new(),
    };
    check_input_datum_hash_in_witness_set(tx_body, utxos, &mut plutus_data_hashes)?;
    check_remaining_datums(&plutus_data_hashes, tx_body, utxos)
}

// Each datum hash in a Plutus script input matches the hash of a datum in the
// transaction witness set.
fn check_input_datum_hash_in_witness_set(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    plutus_data_hash: &mut [(bool, Hash<32>)],
) -> ValidationResult {
    for input in &tx_body.inputs {
        let output = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(InputNotInUTxO))?;
        if let Some(DatumOption::Hash(hash)) = output.datum() {
            find_plutus_datum_in_witness_set(&hash, plutus_data_hash)?
        }
    }
    Ok(())
}

// Extract datum hash if one is contained.
fn get_datum_hash(output: &TransactionOutput) -> Option<Hash<32>> {
    match output {
        TransactionOutput::Legacy(output) => output.datum_hash,
        TransactionOutput::PostAlonzo(output) => match output.datum_option.as_deref() {
            Some(DatumOption::Hash(hash)) => Some(*hash),
            _ => None,
        },
    }
}

fn find_plutus_datum_in_witness_set(
    hash: &Hash<32>,
    plutus_data_hash: &mut [(bool, Hash<32>)],
) -> ValidationResult {
    for (found, plutus_datum_hash) in plutus_data_hash {
        if hash == plutus_datum_hash {
            *found = true;
            return Ok(());
        }
    }
    Err(PostAlonzo(DatumMissing))
}

// Each datum in the transaction witness set can be related to the datum hash in
// a Plutus script input, or in a reference input, or in a regular output, or in
// the collateral return output
fn check_remaining_datums(
    plutus_data_hash: &[(bool, Hash<32>)],
    tx_body: &TransactionBody,
    utxos: &UTxOs,
) -> ValidationResult {
    for (found, plutus_datum_hash) in plutus_data_hash {
        if !found {
            find_datum(plutus_datum_hash, tx_body, utxos)?
        }
    }
    Ok(())
}

fn find_datum(hash: &Hash<32>, tx_body: &TransactionBody, utxos: &UTxOs) -> ValidationResult {
    // Look for hash in transaction (regular) outputs
    for output in tx_body.outputs.iter() {
        if let Some(datum_hash) = get_datum_hash(output)
            && *hash == datum_hash
        {
            return Ok(());
        }
    }
    // Look for hash in collateral return output
    if let Some(babbage_output) = &tx_body.collateral_return {
        match babbage_output.deref() {
            TransactionOutput::Legacy(output) => {
                if let Some(datum_hash) = &output.datum_hash
                    && *hash == *datum_hash
                {
                    return Ok(());
                }
            }
            TransactionOutput::PostAlonzo(output) => {
                if let Some(DatumOption::Hash(datum_hash)) = &output.datum_option.as_deref()
                    && *hash == *datum_hash
                {
                    return Ok(());
                }
            }
        }
    }
    // Look for hash in reference input
    if let Some(reference_inputs) = &tx_body.reference_inputs {
        for reference_input in reference_inputs.iter() {
            if let Some(DatumOption::Hash(datum_hash)) = utxos
                .get(&MultiEraInput::from_alonzo_compatible(reference_input))
                .and_then(MultiEraOutput::datum)
                && *hash == datum_hash
            {
                return Ok(());
            }
        }
    }
    Err(PostAlonzo(UnneededDatum))
}

fn check_redeemers(
    plutus_v1_scripts: &[PolicyId],
    plutus_v2_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
    tx_body: &TransactionBody,
    tx_wits: &WitnessSet,
    utxos: &UTxOs,
) -> ValidationResult {
    let redeemer_pointers: Vec<RedeemerPointer> = match &tx_wits.redeemer {
        Some(redeemers) => redeemers
            .iter()
            .map(|x| RedeemerPointer {
                tag: x.tag,
                index: x.index,
            })
            .collect(),
        None => Vec::new(),
    };
    let plutus_scripts: Vec<RedeemerPointer> = mk_plutus_script_redeemer_pointers(
        plutus_v1_scripts,
        plutus_v2_scripts,
        reference_scripts,
        tx_body,
        utxos,
    )?;
    redeemer_pointers_coincide(&redeemer_pointers, &plutus_scripts)
}

// Lexicographical sorting for inputs.
fn sort_inputs(unsorted_inputs: &[TransactionInput]) -> Vec<TransactionInput> {
    let mut res: Vec<TransactionInput> = unsorted_inputs.to_owned();
    res.sort();
    res
}

fn mk_plutus_script_redeemer_pointers(
    plutus_v1_scripts: &[PolicyId],
    plutus_v2_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
    tx_body: &TransactionBody,
    utxos: &UTxOs,
) -> Result<Vec<RedeemerPointer>, ValidationError> {
    let mut res: Vec<RedeemerPointer> = Vec::new();
    let sorted_inputs: &Vec<TransactionInput> = &sort_inputs(&tx_body.inputs);
    for (index, input) in sorted_inputs.iter().enumerate() {
        if let Some(hash) = get_script_hash_from_input(input, utxos)
            && is_phase_2_script(
                &hash,
                plutus_v1_scripts,
                plutus_v2_scripts,
                reference_scripts,
            )
        {
            res.push(RedeemerPointer {
                tag: RedeemerTag::Spend,
                index: index as u32,
            })
        }
    }
    if let Some(mint) = &tx_body.mint {
        for (index, policy) in sort_policies(mint).iter().enumerate() {
            if is_phase_2_script(
                policy,
                plutus_v1_scripts,
                plutus_v2_scripts,
                reference_scripts,
            ) {
                res.push(RedeemerPointer {
                    tag: RedeemerTag::Mint,
                    index: index as u32,
                })
            }
        }
    }
    for (index, hash) in certificate_scripts(certificates(tx_body)) {
        if is_phase_2_script(
            hash,
            plutus_v1_scripts,
            plutus_v2_scripts,
            reference_scripts,
        ) {
            res.push(RedeemerPointer {
                tag: RedeemerTag::Cert,
                index,
            })
        }
    }
    for (index, hash) in
        withdrawal_scripts(tx_body.withdrawals.as_ref(), PostAlonzo(InputDecoding))?
    {
        if is_phase_2_script(
            &hash,
            plutus_v1_scripts,
            plutus_v2_scripts,
            reference_scripts,
        ) {
            res.push(RedeemerPointer {
                tag: RedeemerTag::Reward,
                index,
            })
        }
    }
    Ok(res)
}

// Lexicographical sorting for PolicyID's.
fn sort_policies(mint: &Mint) -> Vec<PolicyId> {
    let mut res: Vec<PolicyId> = mint.clone().keys().copied().collect();
    res.sort();
    res
}

fn is_phase_2_script(
    policy: &PolicyId,
    plutus_v1_scripts: &[PolicyId],
    plutus_v2_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
) -> bool {
    plutus_v1_scripts
        .iter()
        .any(|v1_script| policy == v1_script)
        || plutus_v2_scripts
            .iter()
            .any(|v2_script| policy == v2_script)
        || reference_scripts
            .iter()
            .any(|ref_script| policy == ref_script)
}

fn redeemer_pointers_coincide(
    redeemers: &[RedeemerPointer],
    plutus_scripts: &[RedeemerPointer],
) -> ValidationResult {
    for redeemer_pointer in redeemers {
        if !plutus_scripts.iter().any(|x| x == redeemer_pointer) {
            return Err(PostAlonzo(UnneededRedeemer));
        }
    }
    for ps_redeemer_pointer in plutus_scripts {
        if !redeemers.iter().any(|x| x == ps_redeemer_pointer) {
            return Err(PostAlonzo(RedeemerMissing));
        }
    }
    Ok(())
}

// All required signers (needed by a Plutus script) have a corresponding match
// in the transaction witness set.
fn check_required_signers(
    required_signers: &Option<RequiredSigners>,
    vkey_wits: &Option<Vec<VKeyWitness>>,
    data_to_verify: &[u8],
) -> ValidationResult {
    if let Some(req_signers) = &required_signers {
        match &vkey_wits {
            Some(vkey_wits) => {
                for req_signer in req_signers {
                    find_and_check_req_signer(req_signer, vkey_wits, data_to_verify)?
                }
            }
            None => return Err(PostAlonzo(ReqSignerMissing)),
        }
    }
    Ok(())
}

// Try to find the verification key in the witnesses, and verify the signature.
fn find_and_check_req_signer(
    vkey_hash: &AddrKeyhash,
    vkey_wits: &[VKeyWitness],
    data_to_verify: &[u8],
) -> ValidationResult {
    for vkey_wit in vkey_wits {
        if pallas_crypto::hash::Hasher::<224>::hash(&vkey_wit.vkey.clone()) == *vkey_hash {
            if !verify_signature(vkey_wit, data_to_verify) {
                return Err(PostAlonzo(ReqSignerWrongSig));
            } else {
                return Ok(());
            }
        }
    }
    Err(PostAlonzo(ReqSignerMissing))
}

fn check_vkey_input_wits(
    mtx: &Tx,
    vkey_wits: &Option<Vec<VKeyWitness>>,
    utxos: &UTxOs,
) -> ValidationResult {
    let tx_body: &TransactionBody = &mtx.transaction_body;
    let vk_wits: &mut Vec<(bool, VKeyWitness)> =
        &mut mk_alonzo_vk_wits_check_list(vkey_wits, PostAlonzo(VKWitnessMissing))?;
    let tx_hash: &Vec<u8> = &Vec::from(mtx.transaction_body.original_hash().as_ref());
    let mut inputs_and_collaterals: Vec<TransactionInput> = Vec::new();
    inputs_and_collaterals.extend(tx_body.inputs.clone());
    if let Some(collaterals) = &tx_body.collateral {
        inputs_and_collaterals.extend(collaterals.clone())
    }
    for input in inputs_and_collaterals.iter() {
        match utxos.get(&MultiEraInput::from_alonzo_compatible(input)) {
            Some(multi_era_output) => match multi_era_output.address() {
                Ok(Address::Shelley(address)) => match address.payment() {
                    ShelleyPaymentPart::Key(payment_key_hash) => {
                        check_vk_wit(payment_key_hash, vk_wits, tx_hash)?
                    }
                    ShelleyPaymentPart::Script(_) => {}
                },
                _ => return Err(PostAlonzo(InputDecoding)),
            },
            None => return Err(PostAlonzo(InputNotInUTxO)),
        }
    }
    for cert in certificates(tx_body) {
        for key_hash in certificate_key_hashes(cert) {
            check_vk_wit(key_hash, vk_wits, tx_hash)?;
        }
    }
    check_remaining_vk_wits(vk_wits, tx_hash, PostAlonzo(VKWrongSignature))
}

fn check_vk_wit(
    payment_key_hash: &AddrKeyhash,
    wits: &mut [(bool, VKeyWitness)],
    data_to_verify: &[u8],
) -> ValidationResult {
    for (vkey_wit_covered, vkey_wit) in wits {
        if pallas_crypto::hash::Hasher::<224>::hash(&vkey_wit.vkey.clone()) == *payment_key_hash {
            if !verify_signature(vkey_wit, data_to_verify) {
                return Err(PostAlonzo(VKWrongSignature));
            } else {
                *vkey_wit_covered = true;
                return Ok(());
            }
        }
    }
    Err(PostAlonzo(VKWitnessMissing))
}

fn check_languages(
    mtx: &Tx,
    utxos: &UTxOs,
    network_magic: &u32,
    network_id: &u8,
    block_slot: &u64,
) -> ValidationResult {
    let available_langs: Vec<Language> =
        available_langs(mtx, utxos, network_magic, network_id, block_slot);
    for tx_lang in tx_languages(mtx, utxos).iter() {
        if !available_langs.contains(tx_lang) {
            return Err(PostAlonzo(UnsupportedPlutusLanguage));
        }
    }
    Ok(())
}

fn available_langs(
    mtx: &Tx,
    utxos: &UTxOs,
    network_magic: &u32,
    network_id: &u8,
    block_slot: &u64,
) -> Vec<Language> {
    let block_langs: Vec<Language> = block_langs(*network_magic, *network_id, *block_slot);
    let allowed_langs: Vec<Language> = allowed_langs(mtx, utxos);
    block_langs
        .iter()
        .filter(|&cost_model_language| allowed_langs.contains(cost_model_language))
        .cloned()
        .collect::<Vec<Language>>()
}

fn block_langs(network_magic: u32, network_id: u8, block_slot: u64) -> Vec<Language> {
    if network_magic == 1 && network_id == 0 {
        //Preprod - 3,974,409 is the slot of the first block in epoch 13
        if block_slot >= 3974409 {
            vec![Language::PlutusV1, Language::PlutusV2]
        } else {
            vec![Language::PlutusV1]
        }
    } else if network_magic == 2 && network_id == 0 {
        // Preview - 777,610 is the slot of the first block in epoch 9
        if block_slot >= 777610 {
            vec![Language::PlutusV1, Language::PlutusV2]
        } else {
            vec![Language::PlutusV1]
        }
    } else {
        // Mainnet - 72,748,820 is the slot of the first block in epoch 366
        if block_slot >= 72748820 {
            vec![Language::PlutusV1, Language::PlutusV2]
        } else {
            vec![Language::PlutusV1]
        }
    }
}

fn allowed_langs(mtx: &Tx, utxos: &UTxOs) -> Vec<Language> {
    let body = &mtx.transaction_body;
    let input_features = reference_script_inputs(body)
        .filter_map(|input| utxos.get(&MultiEraInput::from_alonzo_compatible(input)))
        .map(output_language_features);
    let output_features = body
        .outputs
        .iter()
        .map(|output| output_language_features(&MultiEraOutput::from_babbage(output)));
    let (byron, modern) = input_features
        .chain(output_features)
        .fold((false, false), |(byron, modern), (b, m)| {
            (byron || b, modern || m)
        });
    if byron {
        vec![]
    } else if modern || any_reference_inputs(&body.reference_inputs) {
        vec![Language::PlutusV2]
    } else {
        vec![Language::PlutusV1, Language::PlutusV2]
    }
}

fn any_reference_inputs(reference_inputs: &Option<Vec<TransactionInput>>) -> bool {
    match reference_inputs {
        Some(reference_inputs) => !reference_inputs.is_empty(),
        None => false,
    }
}

fn tx_languages(mtx: &Tx, utxos: &UTxOs) -> Vec<Language> {
    let mut v1_scripts: bool = false;
    let mut v2_scripts: bool = false;
    if let Some(v1_scripts_vec) = &mtx.transaction_witness_set.plutus_v1_script
        && !v1_scripts_vec.is_empty()
    {
        v1_scripts = true
    }
    if let Some(v2_scripts_vec) = &mtx.transaction_witness_set.plutus_v2_script
        && !v2_scripts_vec.is_empty()
    {
        v2_scripts = true;
    }
    let needed = needed_script_hashes(&mtx.transaction_body, utxos);
    for input in reference_script_inputs(&mtx.transaction_body) {
        let Some(TransactionOutput::PostAlonzo(output)) = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .and_then(MultiEraOutput::as_babbage)
        else {
            continue;
        };
        let Some(script) = &output.script_ref else {
            continue;
        };
        if !get_script_hash_from_reference_input(input, utxos)
            .is_some_and(|hash| needed.contains(&hash))
        {
            continue;
        }
        match script.clone().unwrap() {
            ScriptRef::PlutusV1Script(_) => v1_scripts = true,
            ScriptRef::PlutusV2Script(_) => v2_scripts = true,
            ScriptRef::NativeScript(_) => (),
        }
    }
    if !v1_scripts && !v2_scripts {
        vec![]
    } else if v1_scripts && !v2_scripts {
        vec![Language::PlutusV1]
    } else if !v1_scripts && v2_scripts {
        vec![Language::PlutusV2]
    } else {
        vec![Language::PlutusV1, Language::PlutusV2]
    }
}

fn needed_script_hashes(tx_body: &TransactionBody, utxos: &UTxOs) -> Vec<ScriptHash> {
    let inputs = tx_body
        .inputs
        .iter()
        .filter_map(|input| get_script_hash_from_input(input, utxos));
    let policies = tx_body.mint.iter().flat_map(|mint| mint.keys().copied());
    let certificates = certificate_scripts(certificates(tx_body)).map(|(_, hash)| *hash);
    let withdrawals = tx_body
        .withdrawals
        .iter()
        .flat_map(|withdrawals| withdrawals.keys())
        .filter_map(|account| get_script_hash_from_reward_account(account));
    inputs
        .chain(policies)
        .chain(certificates)
        .chain(withdrawals)
        .collect()
}

// The metadata of the transaction is valid.
fn check_auxiliary_data(tx_body: &TransactionBody, mtx: &Tx) -> ValidationResult {
    match (&tx_body.auxiliary_data_hash, aux_data_from_babbage_tx(mtx)) {
        (Some(metadata_hash), Some(metadata)) => {
            if metadata_hash.as_slice()
                == pallas_crypto::hash::Hasher::<256>::hash(metadata).as_ref()
            {
                Ok(())
            } else {
                Err(PostAlonzo(MetadataHash))
            }
        }
        (None, None) => Ok(()),
        _ => Err(PostAlonzo(MetadataHash)),
    }
}

fn check_script_data_hash(
    tx_body: &TransactionBody,
    mtx: &Tx,
    utxos: &UTxOs,
    network_magic: &u32,
    network_id: &u8,
    block_slot: &u64,
) -> ValidationResult {
    match tx_body.script_data_hash {
        Some(script_data_hash) => match (
            &mtx.transaction_witness_set.plutus_data,
            &mtx.transaction_witness_set.redeemer,
        ) {
            (Some(plutus_data), Some(redeemer)) => {
                let plutus_data: Vec<PlutusData> = plutus_data
                    .iter()
                    .map(|x| KeepRaw::unwrap(x.clone()))
                    .collect();
                // The Plutus data part of the script integrity hash may either need to be
                // serialized as a indefinite-length array, or a definite-length one.
                // TODO: compute only the correct hash, not both of them.
                let (indefinite_hash, definite_hash) = compute_script_integrity_hash(
                    &tx_languages(mtx, utxos),
                    &plutus_data,
                    redeemer,
                    network_magic,
                    network_id,
                    block_slot,
                );
                if script_data_hash == indefinite_hash || script_data_hash == definite_hash {
                    Ok(())
                } else {
                    Err(PostAlonzo(ScriptIntegrityHash))
                }
            }
            (_, _) => Err(PostAlonzo(ScriptIntegrityHash)),
        },
        None => {
            if option_vec_is_empty(&mtx.transaction_witness_set.plutus_data)
                && option_vec_is_empty(&mtx.transaction_witness_set.redeemer)
            {
                Ok(())
            } else {
                Err(PostAlonzo(ScriptIntegrityHash))
            }
        }
    }
}

// The Plutus data is encoded both as an indefinite-length and a definite-length
// array. Hence, two hashes are computed.
// TODO: compute only the necessary form, which requires knowing the original
// encoding in the MintedWitnessSet.
fn compute_script_integrity_hash(
    tx_languages: &[Language],
    plutus_data: &[PlutusData],
    redeemer: &[Redeemer],
    network_magic: &u32,
    network_id: &u8,
    block_slot: &u64,
) -> (Hash<32>, Hash<32>) {
    // Indefinite Plutus data serialization
    let mut value_to_hash_with_indef: Vec<u8> = Vec::new();
    let _ = encode(redeemer, &mut value_to_hash_with_indef);
    if !plutus_data.is_empty() {
        let mut plutus_data_encoder_indef: Encoder<Vec<u8>> = Encoder::new(Vec::new());
        let _ = plutus_data_encoder_indef.begin_array();
        for single_plutus_data in plutus_data.iter() {
            let _ = plutus_data_encoder_indef.encode(single_plutus_data);
        }
        let _ = plutus_data_encoder_indef.end();
        value_to_hash_with_indef.extend(plutus_data_encoder_indef.writer().clone());
    }
    let cost_model = cost_model_cbor(tx_languages, *network_magic, *network_id, block_slot);
    value_to_hash_with_indef.extend(&cost_model);
    // Definite Plutus data serialization
    let mut value_to_hash_with_def: Vec<u8> = Vec::new();
    let _ = encode(redeemer, &mut value_to_hash_with_def);
    if !plutus_data.is_empty() {
        let mut plutus_data_encoder_def: Encoder<Vec<u8>> = Encoder::new(Vec::new());
        let _ = plutus_data_encoder_def.array(plutus_data.len() as u64);
        for single_plutus_data in plutus_data.iter() {
            let _ = plutus_data_encoder_def.encode(single_plutus_data);
        }
        value_to_hash_with_def.extend(plutus_data_encoder_def.writer().clone());
    }
    value_to_hash_with_def.extend(&cost_model);
    (
        pallas_crypto::hash::Hasher::<256>::hash(&value_to_hash_with_indef),
        pallas_crypto::hash::Hasher::<256>::hash(&value_to_hash_with_def),
    )
}

// Precondition: !tx_languages.is_empty()
fn cost_model_cbor(
    tx_languages: &[Language],
    network_magic: u32,
    network_id: u8,
    block_slot: &u64,
) -> Vec<u8> {
    if network_magic == 1 && network_id == 0 {
        // Preprod
        if *block_slot < 3974409 {
            // Up to end of epoch 12
            hex::decode(
                "a141005901d59f1a000302590001011a00060bc719026d00011a000249f01903e800011a000249f018201a0025cea81971f70419744d186419744d186419744d186419744d186419744d186419744d18641864186419744d18641a000249f018201a000249f018201a000249f018201a000249f01903e800011a000249f018201a000249f01903e800081a000242201a00067e2318760001011a000249f01903e800081a000249f01a0001b79818f7011a000249f0192710011a0002155e19052e011903e81a000249f01903e8011a000249f018201a000249f018201a000249f0182001011a000249f0011a000249f0041a000194af18f8011a000194af18f8011a0002377c190556011a0002bdea1901f1011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000242201a00067e23187600010119f04c192bd200011a000249f018201a000242201a00067e2318760001011a000242201a00067e2318760001011a0025cea81971f704001a000141bb041a000249f019138800011a000249f018201a000302590001011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a00330da70101ff"
            ).unwrap()
        } else if (3974409..=20390403).contains(block_slot) {
            // From start of epoch 13 up to end of epoch 50
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            }
        } else {
            // From start of epoch 51 onwards
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            }
        }
    } else if network_magic == 2 && network_id == 0 {
        // Preview
        if *block_slot < 777610 {
            // Up to end of epoch 8
            hex::decode(
                "a141005901d59f1a000302590001011a00060bc719026d00011a000249f01903e800011a000249f018201a0025cea81971f70419744d186419744d186419744d186419744d186419744d186419744d18641864186419744d18641a000249f018201a000249f018201a000249f018201a000249f01903e800011a000249f018201a000249f01903e800081a000242201a00067e2318760001011a000249f01903e800081a000249f01a0001b79818f7011a000249f0192710011a0002155e19052e011903e81a000249f01903e8011a000249f018201a000249f018201a000249f0182001011a000249f0011a000249f0041a000194af18f8011a000194af18f8011a0002377c190556011a0002bdea1901f1011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000242201a00067e23187600010119f04c192bd200011a000249f018201a000242201a00067e2318760001011a000242201a00067e2318760001011a0025cea81971f704001a000141bb041a000249f019138800011a000249f018201a000302590001011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a00330da70101ff"
            ).unwrap()
        } else if (777610..1900893).contains(block_slot) {
            // From start of epoch 9 up to end of epoch 21
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            }
        } else if (1900893..=9244810).contains(block_slot) {
            // From start of epoch 22 up to end of epoch 106
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a009063b91903fd0a1a02515e841980b30a"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a009063b91903fd0a1a02515e841980b30a"
                ).unwrap()
            }
        } else {
            // From start of epoch 107 onwards
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            }
        }
    } else {
        // All other combinations are assumed to correspond to a mainnet network
        if *block_slot < 72748820 {
            hex::decode(
                "a141005901d59f1a000302590001011a00060bc719026d00011a000249f01903e800011a000249f018201a0025cea81971f70419744d186419744d186419744d186419744d186419744d186419744d18641864186419744d18641a000249f018201a000249f018201a000249f018201a000249f01903e800011a000249f018201a000249f01903e800081a000242201a00067e2318760001011a000249f01903e800081a000249f01a0001b79818f7011a000249f0192710011a0002155e19052e011903e81a000249f01903e8011a000249f018201a000249f018201a000249f0182001011a000249f0011a000249f0041a000194af18f8011a000194af18f8011a0002377c190556011a0002bdea1901f1011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000242201a00067e23187600010119f04c192bd200011a000249f018201a000242201a00067e2318760001011a000242201a00067e2318760001011a0025cea81971f704001a000141bb041a000249f019138800011a000249f018201a000302590001011a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a000249f018201a00330da70101ff"
            ).unwrap()
        } else if (72748820..84844885).contains(block_slot) {
            // Prior to epoch 394
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            } else {
                // Precondition allows us to conclude both PlutusV1 and PlutusV2 are required by
                // the transaction
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a009063b91903fd0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201b00000004a817c8001b00000004a817c8001a009063b91903fd0a1b00000004a817c800001b00000004a817c800"
                ).unwrap()
            }
        } else {
            // Starting from epoch 394
            if tx_languages.contains(&Language::PlutusV1)
                && !tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a141005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff"
                ).unwrap()
            } else if !tx_languages.contains(&Language::PlutusV1)
                && tx_languages.contains(&Language::PlutusV2)
            {
                hex::decode(
                    "a10198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            } else {
                hex::decode(
                    "a241005901b69f1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0374f693194a1f0aff0198af1a0003236119032c01011903e819023b00011903e8195e7104011903e818201a0001ca761928eb041959d818641959d818641959d818641959d818641959d818641959d81864186418641959d81864194c5118201a0002acfa182019b551041a000363151901ff00011a00015c3518201a000797751936f404021a0002ff941a0006ea7818dc0001011903e8196ff604021a0003bd081a00034ec5183e011a00102e0f19312a011a00032e801901a5011a0002da781903e819cf06011a00013a34182019a8f118201903e818201a00013aac0119e143041903e80a1a00030219189c011a00030219189c011a0003207c1901d9011a000330001901ff0119ccf3182019fd40182019ffd5182019581e18201940b318201a00012adf18201a0002ff941a0006ea7818dc0001011a00010f92192da7000119eabb18201a0002ff941a0006ea7818dc0001011a0002ff941a0006ea7818dc0001011a0011b22c1a0005fdde00021a000c504e197712041a001d6af61a0001425b041a00040c660004001a00014fab18201a0003236119032c010119a0de18201a00033d7618201979f41820197fb8182019a95d1820197df718201995aa18201a0223accc0a1a0374f693194a1f0a1a02515e841980b30a"
                ).unwrap()
            }
        }
    }
}

fn option_vec_is_empty<T>(option_vec: &Option<Vec<T>>) -> bool {
    match option_vec {
        Some(vec) => vec.is_empty(),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase1::fixtures::{
        V2_SCRIPT, byron_address, byron_output, input, key_address, legacy_output,
        output_with_reference_script, reward_address, script_address, tx_referencing_input,
        tx_spending_input, utxos,
    };
    use pallas_codec::{minicbor, utils::CborWrap};
    use pallas_crypto::{hash::Hasher, key::ed25519::SecretKey};
    use pallas_primitives::{
        PlutusScript, StakeCredential,
        alonzo::{ExUnits, NativeScript},
        babbage::{Certificate, PostAlonzoTransactionOutput},
    };
    use pallas_traverse::Era;

    fn result(actual: ValidationResult, expected: &str) {
        assert_eq!(format!("{actual:?}"), expected);
    }

    // Re-encodes and decodes the edited transaction, so the body hash the keys
    // sign is the hash of the edited body.
    fn synthetic(
        certificates: Vec<Certificate>,
        prepare: impl FnOnce(&mut Tx<'_>),
        keys: &[u8],
        test: impl FnOnce(Tx<'_>),
    ) {
        let raw = hex::decode(include_str!("../../../test_data/babbage3.tx").trim()).unwrap();
        let mut tx: Tx = minicbor::decode(&raw).unwrap();
        tx.transaction_body.inputs = Vec::new();
        tx.transaction_body.collateral = None;
        tx.transaction_body.reference_inputs = None;
        tx.transaction_body.required_signers = None;
        tx.transaction_body.withdrawals = None;
        tx.transaction_body.mint = None;
        tx.transaction_body.certificates = Some(certificates);
        tx.transaction_witness_set = minicbor::decode::<WitnessSet>(&[0xa0]).unwrap().into();
        prepare(&mut tx);
        let encoded = minicbor::to_vec(tx).unwrap();
        let mut tx: Tx = minicbor::decode(&encoded).unwrap();
        let hash = tx.transaction_body.original_hash();
        let witnesses: Vec<_> = keys
            .iter()
            .map(|id| {
                let key = SecretKey::from([*id; 32]);
                VKeyWitness {
                    vkey: key.public_key().as_ref().to_vec().into(),
                    signature: key.sign(hash).as_ref().to_vec().into(),
                }
            })
            .collect();
        tx.transaction_witness_set.vkeywitness = Some(witnesses);
        test(tx);
    }

    fn key_hash(id: u8) -> AddrKeyhash {
        Hasher::<224>::hash(SecretKey::from([id; 32]).public_key().as_ref())
    }

    fn corrupt_signature(tx: &mut Tx<'_>, position: usize) {
        tx.transaction_witness_set.vkeywitness.as_mut().unwrap()[position].signature =
            vec![0; 64].into();
    }

    fn plutus() -> PlutusScript<2> {
        PlutusScript(vec![1, 2, 3].into())
    }

    fn redeemer(index: u32, tag: RedeemerTag) -> Redeemer {
        Redeemer {
            tag,
            index,
            data: PlutusData::BigInt(pallas_primitives::BigInt::Int(0.into())),
            ex_units: ExUnits { mem: 1, steps: 1 },
        }
    }

    fn add_plutus(tx: &mut Tx<'_>, cert_indexes: &[u32]) {
        tx.transaction_witness_set.plutus_v2_script = Some(vec![plutus()]);
        tx.transaction_witness_set.redeemer = Some(
            cert_indexes
                .iter()
                .map(|index| redeemer(*index, RedeemerTag::Cert))
                .collect(),
        );
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
            (Some(0), "Err(PostAlonzo(VKWrongSignature))"),
            (Some(1), "Err(PostAlonzo(VKWrongSignature))"),
        ] {
            synthetic(
                vec![],
                |_| {},
                &[1, 2],
                |mut tx| {
                    if let Some(position) = corrupted {
                        corrupt_signature(&mut tx, position);
                    }
                    result(check_witness_set(&tx, &UTxOs::new()), expected);
                },
            );
        }
    }

    #[test]
    fn key_certificates_require_their_key_and_valid_signature() {
        let cred = StakeCredential::AddrKeyhash(key_hash(1));
        synthetic(
            vec![Certificate::StakeRegistration(cred.clone())],
            |_| {},
            &[],
            |tx| result(check_witness_set(&tx, &UTxOs::new()), "Ok(())"),
        );
        for cert in credential_certificates(cred) {
            for (keys, expected) in [
                (&[1][..], "Ok(())"),
                (&[][..], "Err(PostAlonzo(VKWitnessMissing))"),
                (&[2][..], "Err(PostAlonzo(VKWitnessMissing))"),
            ] {
                synthetic(
                    vec![cert.clone()],
                    |_| {},
                    keys,
                    |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
                );
            }
            synthetic(
                vec![cert],
                |_| {},
                &[1],
                |mut tx| {
                    corrupt_signature(&mut tx, 0);
                    result(
                        check_witness_set(&tx, &UTxOs::new()),
                        "Err(PostAlonzo(VKWrongSignature))",
                    );
                },
            );
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
            synthetic(
                vec![cert.clone()],
                |_| {},
                &keys,
                |tx| result(check_witness_set(&tx, &UTxOs::new()), "Ok(())"),
            );
            for missing in 0..keys.len() {
                let mut incomplete = keys.clone();
                incomplete.remove(missing);
                synthetic(
                    vec![cert.clone()],
                    |_| {},
                    &incomplete,
                    |tx| {
                        result(
                            check_witness_set(&tx, &UTxOs::new()),
                            "Err(PostAlonzo(VKWitnessMissing))",
                        )
                    },
                );
            }
        }
    }

    #[test]
    fn script_certificates_require_script_and_plutus_redeemer() {
        let cred = StakeCredential::ScriptHash(compute_plutus_v2_script_hash(&plutus()));
        for cert in credential_certificates(cred) {
            let outcome = |prepare: &dyn Fn(&mut Tx<'_>)| {
                let mut outcome = String::new();
                synthetic(vec![cert.clone()], prepare, &[], |tx| {
                    outcome = format!("{:?}", check_witness_set(&tx, &UTxOs::new()));
                });
                outcome
            };
            assert_eq!(outcome(&|tx| add_plutus(tx, &[0])), "Ok(())");
            assert_eq!(outcome(&|_| {}), "Err(PostAlonzo(ScriptWitnessMissing))");
            assert_eq!(
                outcome(&|tx| add_plutus(tx, &[])),
                "Err(PostAlonzo(RedeemerMissing))"
            );
            assert_eq!(
                outcome(&|tx| {
                    add_plutus(tx, &[0]);
                    tx.transaction_witness_set.plutus_v2_script =
                        Some(vec![PlutusScript(vec![99].into())]);
                }),
                "Err(PostAlonzo(ScriptWitnessMissing))"
            );
        }
        let v1 = PlutusScript::<1>(vec![1, 2].into());
        synthetic(
            vec![Certificate::StakeDeregistration(
                StakeCredential::ScriptHash(compute_plutus_v1_script_hash(&v1)),
            )],
            |tx| {
                tx.transaction_witness_set.plutus_v1_script = Some(vec![v1.clone()]);
                tx.transaction_witness_set.redeemer = Some(vec![redeemer(0, RedeemerTag::Cert)]);
            },
            &[],
            |tx| result(check_witness_set(&tx, &UTxOs::new()), "Ok(())"),
        );
    }

    #[test]
    fn repeated_certificates_point_at_their_first_occurrence() {
        let hash = compute_plutus_v2_script_hash(&plutus());
        let certs = vec![
            Certificate::StakeDeregistration(StakeCredential::ScriptHash(hash)),
            Certificate::StakeRegistration(StakeCredential::AddrKeyhash(key_hash(1))),
            Certificate::StakeDeregistration(StakeCredential::ScriptHash(hash)),
            Certificate::StakeDelegation(StakeCredential::ScriptHash(hash), [9; 28].into()),
        ];
        for (indexes, expected) in [
            (&[0, 3][..], "Ok(())"),
            (&[0, 2, 3][..], "Err(PostAlonzo(UnneededRedeemer))"),
            (&[0][..], "Err(PostAlonzo(RedeemerMissing))"),
        ] {
            synthetic(
                certs.clone(),
                |tx| add_plutus(tx, indexes),
                &[],
                |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
            );
        }
    }

    #[test]
    fn native_certificate_scripts_are_evaluated() {
        let script = NativeScript::ScriptPubkey(key_hash(1));
        for cert in credential_certificates(StakeCredential::ScriptHash(
            compute_native_script_hash(&script),
        )) {
            for (keys, expected) in [
                (&[1][..], "Ok(())"),
                (&[][..], "Err(PostAlonzo(NativeScriptDenial))"),
                (&[2][..], "Err(PostAlonzo(NativeScriptDenial))"),
            ] {
                synthetic(
                    vec![cert.clone()],
                    |tx| {
                        tx.transaction_witness_set.native_script = Some(vec![script.clone().into()])
                    },
                    keys,
                    |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
                );
            }
        }
        let interval = NativeScript::ScriptAll(vec![
            NativeScript::InvalidBefore(10),
            NativeScript::InvalidHereafter(20),
        ]);
        for (start, end, expected) in [
            (Some(10), Some(20), "Ok(())"),
            (Some(9), Some(20), "Err(PostAlonzo(NativeScriptDenial))"),
            (Some(10), Some(21), "Err(PostAlonzo(NativeScriptDenial))"),
        ] {
            synthetic(
                vec![Certificate::StakeDeregistration(
                    StakeCredential::ScriptHash(compute_native_script_hash(&interval)),
                )],
                |tx| {
                    tx.transaction_body.validity_interval_start = start;
                    tx.transaction_body.ttl = end;
                    tx.transaction_witness_set.native_script = Some(vec![interval.clone().into()]);
                },
                &[],
                |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
            );
        }
    }

    // Witness set and minting results for a script by reference that spends,
    // mints, withdraws or certifies, with the reference script on a reference
    // input or on a spent input locked by key 1.
    fn reference_script_outcomes(
        script: ScriptRef<'static>,
        hash: PolicyId,
    ) -> Vec<(RedeemerTag, bool, String)> {
        let holder = TransactionInput {
            transaction_id: [7; 32].into(),
            index: 0,
        };
        let locked = TransactionInput {
            transaction_id: [8; 32].into(),
            index: 0,
        };
        let output = |header: u8, hash: &[u8], script_ref: Option<ScriptRef<'static>>| {
            let mut address = vec![header];
            address.extend(hash);
            let output = TransactionOutput::PostAlonzo(
                PostAlonzoTransactionOutput {
                    address: address.into(),
                    value: Value::Coin(2_000_000),
                    datum_option: None,
                    script_ref: script_ref.map(CborWrap),
                }
                .into(),
            );
            minicbor::to_vec(output).unwrap()
        };
        let held = output(0x61, key_hash(1).as_ref(), Some(script));
        let held: TransactionOutput = minicbor::decode(&held).unwrap();
        let locked_output = output(0x71, hash.as_ref(), None);
        let locked_output: TransactionOutput = minicbor::decode(&locked_output).unwrap();
        let utxos = UTxOs::from([
            (
                MultiEraInput::from_alonzo_compatible(&holder),
                MultiEraOutput::from_babbage(&held),
            ),
            (
                MultiEraInput::from_alonzo_compatible(&locked),
                MultiEraOutput::from_babbage(&locked_output),
            ),
        ]);
        let mut reward_account = vec![0xf0];
        reward_account.extend(hash.as_ref());
        let mut outcomes = Vec::new();
        for purpose in [
            RedeemerTag::Spend,
            RedeemerTag::Mint,
            RedeemerTag::Reward,
            RedeemerTag::Cert,
        ] {
            for holder_spent in [false, true] {
                let certificates = match purpose {
                    RedeemerTag::Cert => vec![Certificate::StakeDeregistration(
                        StakeCredential::ScriptHash(hash),
                    )],
                    _ => vec![],
                };
                synthetic(
                    certificates,
                    |tx| {
                        let body = &mut tx.transaction_body;
                        let mut inputs = Vec::new();
                        if holder_spent {
                            inputs.push(holder.clone());
                        } else {
                            body.reference_inputs = Some(vec![holder.clone()]);
                        }
                        match purpose {
                            RedeemerTag::Spend => inputs.push(locked.clone()),
                            RedeemerTag::Mint => {
                                let assets = [(Bytes::from(vec![1]), 1)].into_iter().collect();
                                body.mint = Some([(hash, assets)].into_iter().collect());
                            }
                            RedeemerTag::Reward => {
                                body.withdrawals = Some(
                                    [(reward_account.clone().into(), 0)].into_iter().collect(),
                                );
                            }
                            _ => {}
                        }
                        body.inputs = inputs;
                    },
                    &[1],
                    |tx| {
                        let outcome = format!(
                            "{:?} {:?}",
                            check_witness_set(&tx, &utxos),
                            check_minting(&tx.transaction_body, &tx, &utxos)
                        );
                        outcomes.push((purpose, holder_spent, outcome));
                    },
                );
            }
        }
        outcomes
    }

    fn every_purpose(expected: &str) -> Vec<(RedeemerTag, bool, String)> {
        [
            RedeemerTag::Spend,
            RedeemerTag::Mint,
            RedeemerTag::Reward,
            RedeemerTag::Cert,
        ]
        .into_iter()
        .flat_map(|purpose| [false, true].map(|spent| (purpose, spent, expected.to_string())))
        .collect()
    }

    #[test]
    fn plutus_reference_scripts_need_a_redeemer_for_every_purpose() {
        let v1 = PlutusScript::<1>(vec![1, 2].into());
        for (script, hash) in [
            (
                ScriptRef::PlutusV1Script(v1.clone()),
                compute_plutus_v1_script_hash(&v1),
            ),
            (
                ScriptRef::PlutusV2Script(plutus()),
                compute_plutus_v2_script_hash(&plutus()),
            ),
        ] {
            assert_eq!(
                reference_script_outcomes(script, hash),
                every_purpose("Err(PostAlonzo(RedeemerMissing)) Ok(())"),
            );
        }
    }

    #[test]
    fn native_reference_scripts_need_no_redeemer_for_any_purpose() {
        let script = NativeScript::ScriptPubkey(key_hash(1));
        let hash = compute_native_script_hash(&script);
        assert_eq!(
            reference_script_outcomes(ScriptRef::NativeScript(script.into()), hash),
            every_purpose("Ok(()) Ok(())"),
        );
    }

    #[test]
    fn native_reference_certificate_scripts_are_evaluated() {
        let script = NativeScript::ScriptPubkey(key_hash(3));
        let hash = compute_native_script_hash(&script);
        let certifying: Vec<_> =
            reference_script_outcomes(ScriptRef::NativeScript(script.into()), hash)
                .into_iter()
                .filter(|(purpose, _, _)| *purpose == RedeemerTag::Cert)
                .map(|(_, _, outcome)| outcome)
                .collect();
        assert_eq!(
            certifying,
            ["Err(PostAlonzo(NativeScriptDenial)) Ok(())"; 2]
        );
    }

    #[test]
    fn native_reference_authorization_uses_original_script_hash() {
        // Valid noncanonical CBOR for [0, key_hash]: integer 0 uses two bytes.
        let mut raw = vec![0x82, 0x18, 0, 0x58, 0x1c];
        raw.extend(key_hash(1).as_ref());
        let script: KeepRaw<'_, NativeScript> = minicbor::decode(&raw).unwrap();
        let hash = script.original_hash();
        assert_ne!(hash, compute_native_script_hash(&script));
        let input = TransactionInput {
            transaction_id: [7; 32].into(),
            index: 0,
        };
        for keys in [&[1][..], &[][..]] {
            synthetic(
                vec![Certificate::StakeDeregistration(
                    StakeCredential::ScriptHash(hash),
                )],
                |tx| tx.transaction_body.reference_inputs = Some(vec![input.clone()]),
                keys,
                |tx| {
                    let output = TransactionOutput::PostAlonzo(
                        PostAlonzoTransactionOutput {
                            address: vec![0x61; 29].into(),
                            value: Value::Coin(2_000_000),
                            datum_option: None,
                            script_ref: Some(CborWrap(ScriptRef::NativeScript(script.clone()))),
                        }
                        .into(),
                    );
                    let utxos = UTxOs::from([(
                        MultiEraInput::from_alonzo_compatible(&input),
                        MultiEraOutput::from_babbage(&output),
                    )]);
                    result(
                        check_witness_set(&tx, &utxos),
                        if keys.is_empty() {
                            "Err(PostAlonzo(NativeScriptDenial))"
                        } else {
                            "Ok(())"
                        },
                    );
                },
            );
        }
    }

    #[test]
    fn script_collateral_from_an_earlier_era_is_not_key_locked() {
        let raw = legacy_output(script_address());
        for era in [Era::Shelley, Era::Alonzo, Era::Babbage] {
            let result = check_collaterals_address(&[input()], &utxos(era, &raw));

            assert!(
                matches!(result, Err(PostAlonzo(CollateralNotVKeyLocked))),
                "{era:?} {result:?}"
            );
        }
    }

    #[test]
    fn key_and_byron_collateral_is_key_locked_from_shelley_to_babbage() {
        for address in [key_address(), byron_address()] {
            let raw = legacy_output(address);
            for era in [Era::Shelley, Era::Alonzo, Era::Babbage] {
                let result = check_collaterals_address(&[input()], &utxos(era, &raw));

                assert!(result.is_ok(), "{era:?} {result:?}");
            }
        }
    }

    #[test]
    fn collateral_at_a_reward_or_undecodable_address_is_refused() {
        for address in [reward_address(), vec![0x61]] {
            let raw = legacy_output(address);
            for era in [Era::Alonzo, Era::Babbage] {
                let result = check_collaterals_address(&[input()], &utxos(era, &raw));

                assert!(
                    matches!(result, Err(PostAlonzo(InputDecoding))),
                    "{era:?} {result:?}"
                );
            }
        }
    }

    #[test]
    fn a_script_address_from_an_earlier_era_names_its_script() {
        for (address, expected) in [
            (script_address(), Some(Hash::from([2; 28]))),
            (key_address(), None),
        ] {
            let raw = legacy_output(address);
            for era in [Era::Shelley, Era::Alonzo, Era::Babbage] {
                let actual = get_script_hash_from_input(&input(), &utxos(era, &raw));

                assert_eq!(actual, expected, "{era:?}");
            }
        }
    }

    #[test]
    fn a_datum_hash_on_an_earlier_era_input_is_found() {
        let (spending, referencing) = (tx_spending_input(), tx_referencing_input());
        let spending: Tx = minicbor::decode(&spending).unwrap();
        let referencing: Tx = minicbor::decode(&referencing).unwrap();
        let hash = Hash::from([3; 32]);
        let raw = minicbor::to_vec(pallas_primitives::alonzo::TransactionOutput {
            address: script_address().into(),
            amount: Value::Coin(5_000_000),
            datum_hash: Some(hash),
        })
        .unwrap();

        for era in [Era::Alonzo, Era::Babbage] {
            let utxos = utxos(era, &raw);

            let witnessed = check_input_datum_hash_in_witness_set(
                &spending.transaction_body,
                &utxos,
                &mut [(false, hash)],
            );
            let unwitnessed =
                check_input_datum_hash_in_witness_set(&spending.transaction_body, &utxos, &mut []);
            let found = find_datum(&hash, &referencing.transaction_body, &utxos);
            let other = find_datum(&Hash::from([4; 32]), &referencing.transaction_body, &utxos);

            assert!(witnessed.is_ok(), "{era:?} {witnessed:?}");
            assert!(
                matches!(unwitnessed, Err(PostAlonzo(DatumMissing))),
                "{era:?} {unwitnessed:?}"
            );
            assert!(found.is_ok(), "{era:?} {found:?}");
            assert!(
                matches!(other, Err(PostAlonzo(UnneededDatum))),
                "{era:?} {other:?}"
            );
        }

        let raw = legacy_output(key_address());
        let result = check_input_datum_hash_in_witness_set(
            &spending.transaction_body,
            &utxos(Era::Shelley, &raw),
            &mut [],
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn a_key_input_from_an_earlier_era_needs_its_key_witness() {
        let cbor = tx_spending_input();
        let tx: Tx = minicbor::decode(&cbor).unwrap();
        let key = SecretKey::from([7; 32]);
        let witnesses = Some(vec![VKeyWitness {
            vkey: key.public_key().as_ref().to_vec().into(),
            signature: key
                .sign(tx.transaction_body.original_hash().as_ref())
                .as_ref()
                .to_vec()
                .into(),
        }]);
        let own = legacy_output(
            [
                vec![0x61],
                Hasher::<224>::hash(key.public_key().as_ref()).to_vec(),
            ]
            .concat(),
        );
        let other = legacy_output(key_address());
        let reward = legacy_output(reward_address());

        for era in [Era::Shelley, Era::Alonzo, Era::Babbage] {
            let own = check_vkey_input_wits(&tx, &witnesses, &utxos(era, &own));
            let other = check_vkey_input_wits(&tx, &witnesses, &utxos(era, &other));
            let reward = check_vkey_input_wits(&tx, &witnesses, &utxos(era, &reward));

            assert!(own.is_ok(), "{era:?} {own:?}");
            assert!(
                matches!(other, Err(PostAlonzo(VKWitnessMissing))),
                "{era:?} {other:?}"
            );
            assert!(
                matches!(reward, Err(PostAlonzo(InputDecoding))),
                "{era:?} {reward:?}"
            );
        }
    }

    #[test]
    fn a_byron_address_on_an_input_of_any_era_allows_no_plutus_language() {
        let cbor = tx_spending_input();
        let tx: Tx = minicbor::decode(&cbor).unwrap();
        let byron = legacy_output(byron_address());
        let key = legacy_output(key_address());

        assert_eq!(
            allowed_langs(&tx, &utxos(Era::Byron, &byron_output())),
            vec![]
        );
        for era in [Era::Shelley, Era::Alonzo, Era::Babbage] {
            assert_eq!(allowed_langs(&tx, &utxos(era, &byron)), vec![], "{era:?}");
            assert_eq!(
                allowed_langs(&tx, &utxos(era, &key)),
                vec![Language::PlutusV1, Language::PlutusV2],
                "{era:?}"
            );
        }
    }

    #[test]
    fn a_reference_script_on_a_spent_input_is_provided() {
        let cbor = tx_spending_input();
        let tx: Tx = minicbor::decode(&cbor).unwrap();
        let with_script = output_with_reference_script();
        let without_script = legacy_output(key_address());

        let provided =
            get_reference_script_hashes(&tx.transaction_body, &utxos(Era::Babbage, &with_script));
        let absent = get_reference_script_hashes(
            &tx.transaction_body,
            &utxos(Era::Babbage, &without_script),
        );

        assert_eq!(provided, vec![Hasher::<224>::hash_tagged(&V2_SCRIPT, 2)]);
        assert!(absent.is_empty(), "{absent:?}");
    }

    #[test]
    fn only_a_needed_reference_script_sets_the_language() {
        let policy = Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
        let with_script = output_with_reference_script();
        let without_script = legacy_output(key_address());
        let (mut outcomes, mut expected) = (vec![], vec![]);
        for (cbor, spent) in [(tx_spending_input(), true), (tx_referencing_input(), false)] {
            let mut tx: Tx = minicbor::decode(&cbor).unwrap();
            for mints in [false, true] {
                let mut body = (*tx.transaction_body).clone();
                body.mint = mints.then(|| [(policy, [(Bytes::from(vec![1]), 1)].into())].into());
                tx.transaction_body = body.into();
                for (output, has_script) in [(&with_script, true), (&without_script, false)] {
                    let languages = tx_languages(&tx, &utxos(Era::Babbage, output));
                    outcomes.push((spent, mints, has_script, languages));
                    let languages = if mints && has_script {
                        vec![Language::PlutusV2]
                    } else {
                        vec![]
                    };
                    expected.push((spent, mints, has_script, languages));
                }
            }
        }
        assert_eq!(outcomes, expected);
    }

    #[test]
    fn each_needed_purpose_sets_the_reference_script_language() {
        let script = Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
        let holder = input();
        let locked = TransactionInput {
            transaction_id: Hash::new([9; 32]),
            index: 0,
        };
        let with_script = output_with_reference_script();
        let cbor = tx_referencing_input();
        let (mut outcomes, mut expected) = (vec![], vec![]);
        for (purpose, names_script) in [
            ("spend", true),
            ("mint", true),
            ("delegation", true),
            ("deregistration", true),
            ("reward", true),
            ("registration", false),
            ("key reward", false),
        ] {
            for hash in [script, Hash::from([5; 28])] {
                let locked_output = legacy_output([vec![0x71], hash.to_vec()].concat());
                let utxos = UTxOs::from([
                    (
                        MultiEraInput::from_alonzo_compatible(&holder),
                        MultiEraOutput::decode(Era::Babbage, &with_script).unwrap(),
                    ),
                    (
                        MultiEraInput::from_alonzo_compatible(&locked),
                        MultiEraOutput::decode(Era::Alonzo, &locked_output).unwrap(),
                    ),
                ]);
                let mut tx: Tx = minicbor::decode(&cbor).unwrap();
                let mut body = (*tx.transaction_body).clone();
                let credential = StakeCredential::ScriptHash(hash);
                match purpose {
                    "spend" => body.inputs = vec![locked.clone()],
                    "mint" => body.mint = Some([(hash, [(Bytes::from(vec![1]), 1)].into())].into()),
                    "delegation" => {
                        let pool = Hash::new([0; 28]);
                        body.certificates =
                            Some(vec![Certificate::StakeDelegation(credential, pool)])
                    }
                    "deregistration" => {
                        body.certificates = Some(vec![Certificate::StakeDeregistration(credential)])
                    }
                    "registration" => {
                        body.certificates = Some(vec![Certificate::StakeRegistration(credential)])
                    }
                    _ => {
                        let header = if names_script { 0xf0 } else { 0xe0 };
                        let account = [vec![header], hash.to_vec()].concat();
                        body.withdrawals = Some([(account.into(), 0)].into())
                    }
                }
                tx.transaction_body = body.into();
                outcomes.push((purpose, hash, tx_languages(&tx, &utxos)));
                let languages = if names_script && hash == script {
                    vec![Language::PlutusV2]
                } else {
                    vec![]
                };
                expected.push((purpose, hash, languages));
            }
        }
        assert_eq!(outcomes, expected);
    }
}
