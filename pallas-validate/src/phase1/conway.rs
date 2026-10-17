//! Utilities required for Conway-era transaction validation.

use crate::utils::{
    ConwayProtParams,
    PostAlonzoError::*,
    UTxOs,
    ValidationError::{self, *},
    ValidationResult, aux_data_from_conway_tx, compute_native_script_hash,
    compute_plutus_v1_script_hash, compute_plutus_v2_script_hash, compute_plutus_v3_script_hash,
    conway_add_minted_non_zero, conway_add_values, conway_get_val_size_in_words,
    conway_lovelace_diff_or_fail, conway_values_are_equal, get_conway_tx_size,
    get_lovelace_from_conway_val, get_script_hash_from_input,
    get_script_hash_from_reward_account, get_shelley_address, mk_alonzo_vk_wits_check_list,
    output_language_features, verify_signature,
};
use pallas_addresses::{
    Address, ByronAddress, Network, ScriptHash, ShelleyAddress, ShelleyPaymentPart, StakeAddress,
    StakePayload, byron::AddressId,
};
use pallas_codec::utils::{Bytes, KeepRaw, NonEmptySet};
use pallas_crypto::key::ed25519::{PublicKey, Signature};
use pallas_primitives::{
    AddrKeyhash, Hash, PlutusData, PolicyId, PositiveCoin, StakeCredential, TransactionInput,
    babbage,
    conway::{
        BootstrapWitness, Certificate, DatumOption, GovAction, Language, LanguageViews, Mint,
        Redeemers, RedeemersKey, RequiredSigners, ScriptRef, TransactionBody, TransactionOutput,
        Tx, VKeyWitness, Value, Voter, WitnessSet,
    },
};
use pallas_traverse::{ComputeHash, MultiEraInput, MultiEraOutput, MultiEraTx, OriginalHash};
use std::cmp::Ordering;
use std::ops::Deref;

pub fn validate_conway_tx(
    mtx: &Tx,
    utxos: &UTxOs,
    prot_pps: &ConwayProtParams,
    block_slot: &u64,
    network_id: &u8,
) -> ValidationResult {
    let tx_body: &TransactionBody = &mtx.transaction_body.clone();
    let size: u32 = get_conway_tx_size(mtx).ok_or(PostAlonzo(UnknownTxSize))?;
    check_ins_not_empty(tx_body)?;
    check_all_ins_in_utxos(tx_body, utxos)?;
    check_tx_validity_interval(tx_body, block_slot)?;
    check_fee(tx_body, &size, mtx, utxos, prot_pps)?;
    check_preservation_of_value(
        tx_body,
        utxos,
        prot_pps.key_deposit,
        prot_pps.drep_deposit,
        prot_pps.governance_action_deposit,
    )?;
    check_min_lovelace(tx_body, prot_pps)?;
    check_output_val_size(tx_body, prot_pps)?;
    check_network_id(tx_body, network_id)?;
    check_tx_size(&size, prot_pps)?;
    check_tx_ex_units(mtx, prot_pps)?;
    check_minting(tx_body, mtx, utxos)?;
    check_well_formedness(tx_body, mtx)?;
    check_witness_set(mtx, utxos)?;
    check_languages(mtx, utxos, prot_pps)?;
    check_auxiliary_data(tx_body, mtx)?;
    check_script_data_hash(tx_body, mtx, utxos, prot_pps)
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
    prot_pps: &ConwayProtParams,
) -> ValidationResult {
    check_min_fee(tx_body, size, prot_pps)?;
    if !MultiEraTx::from_conway(mtx).redeemers().is_empty() {
        check_collaterals(tx_body, utxos, prot_pps)?
    }
    Ok(())
}

// The fee paid by the transaction should be greater than or equal to the
// minimum fee.
fn check_min_fee(
    tx_body: &TransactionBody,
    size: &u32,
    prot_pps: &ConwayProtParams,
) -> ValidationResult {
    if tx_body.fee < (prot_pps.minfee_b + prot_pps.minfee_a * size) as u64 {
        return Err(PostAlonzo(FeeBelowMin));
    }
    Ok(())
}

fn check_collaterals(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    prot_pps: &ConwayProtParams,
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
    prot_pps: &ConwayProtParams,
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
    prot_pps: &ConwayProtParams,
) -> ValidationResult {
    match &tx_body.collateral {
        Some(collaterals) => {
            let first_collateral = collaterals.first().unwrap();
            let mut coll_input =
                match utxos.get(&MultiEraInput::from_alonzo_compatible(first_collateral)) {
                    Some(multi_era_output) => val_from_multi_era_output(multi_era_output),
                    None => return Err(PostAlonzo(CollateralNotInUTxO)),
                };
            for collateral in collaterals.iter().skip(1) {
                match utxos.get(&MultiEraInput::from_alonzo_compatible(collateral)) {
                    Some(multi_era_output) => {
                        coll_input = conway_add_values(
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
            let coll_return: Value = match &tx_body.collateral_return {
                Some(TransactionOutput::Legacy(output)) => {
                    let amount = output.amount.clone();
                    match amount {
                        babbage::Value::Coin(coin) => Value::Coin(coin),
                        babbage::Value::Multiasset(coin, assets) => {
                            let mut conway_assets = Vec::new();
                            for (key, val) in assets.into_iter() {
                                let mut conway_value = Vec::new();
                                for (inner_key, inner_val) in val.into_iter() {
                                    conway_value.push((
                                        inner_key,
                                        PositiveCoin::try_from(inner_val).unwrap(),
                                    ));
                                }
                                conway_assets.push((key, conway_value.into_iter().collect()));
                            }
                            let conway_assets = conway_assets.into_iter().collect();
                            Value::Multiasset(coin, conway_assets)
                        }
                    }
                }
                Some(TransactionOutput::PostAlonzo(output)) => output.value.clone(),
                None => Value::Coin(0),
            };
            // The balance between collateral inputs and output contains only lovelace.
            let paid_collateral: u64 = conway_lovelace_diff_or_fail(
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
    let value = multi_era_output.value();
    value.into_conway()
}

// The preservation of value property holds.
fn check_preservation_of_value(
    tx_body: &TransactionBody,
    utxos: &UTxOs,
    key_deposit: u64,
    drep_deposit: u64,
    proposal_deposit: u64,
) -> ValidationResult {
    let err = PostAlonzo(NegativeValue);
    let (deposits, refunds): (Vec<_>, Vec<_>) = tx_body
        .certificates
        .iter()
        .flatten()
        .map(|cert| certificate_deposit_and_refund(cert, key_deposit, drep_deposit))
        .unzip();
    let withdrawals = tx_body.withdrawals.iter().flatten().map(|(_, coin)| *coin);
    let proposals = tx_body
        .proposal_procedures
        .iter()
        .flatten()
        .map(|_| proposal_deposit);
    let mut input: Value = get_consumed(tx_body, utxos)?;
    for coin in refunds.into_iter().chain(withdrawals) {
        input = conway_add_values(&input, &Value::Coin(coin), &err)?;
    }
    let mut output: Value = get_produced(tx_body)?;
    let donation = tx_body.donation.map(u64::from);
    for coin in [tx_body.fee]
        .into_iter()
        .chain(deposits)
        .chain(proposals)
        .chain(donation)
    {
        output = conway_add_values(&output, &Value::Coin(coin), &err)?;
    }
    if let Some(m) = &tx_body.mint {
        input = conway_add_minted_non_zero(&input, m, &PostAlonzo(NegativeValue))?;
    }

    if !conway_values_are_equal(&input, &output) {
        return Err(PostAlonzo(PreservationOfValue));
    }
    Ok(())
}

// The deposit a certificate charges and the refund it returns.
fn certificate_deposit_and_refund(
    cert: &Certificate,
    key_deposit: u64,
    drep_deposit: u64,
) -> (u64, u64) {
    match cert {
        Certificate::StakeRegistration(..)
        | Certificate::Reg(..)
        | Certificate::StakeRegDeleg(..)
        | Certificate::VoteRegDeleg(..)
        | Certificate::StakeVoteRegDeleg(..) => (key_deposit, 0),
        Certificate::RegDRepCert(..) => (drep_deposit, 0),
        Certificate::StakeDeregistration(..) => (0, key_deposit),
        Certificate::UnReg(_, refund) | Certificate::UnRegDRepCert(_, refund) => (0, *refund),
        _ => (0, 0),
    }
}

fn get_consumed(tx_body: &TransactionBody, utxos: &UTxOs) -> Result<Value, ValidationError> {
    let mut inputs_iter = tx_body.inputs.iter();
    let Some(first_input) = inputs_iter.next() else {
        return Err(PostAlonzo(TxInsEmpty));
    };
    let multi_era_output: &MultiEraOutput = utxos
        .get(&MultiEraInput::from_alonzo_compatible(first_input))
        .ok_or(PostAlonzo(InputNotInUTxO))?;
    let mut res: Value = val_from_multi_era_output(multi_era_output);
    for input in inputs_iter {
        let multi_era_output: &MultiEraOutput = utxos
            .get(&MultiEraInput::from_alonzo_compatible(input))
            .ok_or(PostAlonzo(InputNotInUTxO))?;
        let val: Value = val_from_multi_era_output(multi_era_output);
        res = conway_add_values(&res, &val, &PostAlonzo(NegativeValue))?;
    }
    Ok(res)
}

fn get_produced(tx_body: &TransactionBody) -> Result<Value, ValidationError> {
    let mut outputs_iter = tx_body.outputs.iter();
    let Some(first_output) = outputs_iter.next() else {
        return Err(PostAlonzo(TxInsEmpty));
    };
    let mut res: Value = match first_output {
        TransactionOutput::Legacy(output) => {
            let amount = output.amount.clone();
            match amount {
                babbage::Value::Coin(coin) => Value::Coin(coin),
                babbage::Value::Multiasset(coin, assets) => {
                    let mut conway_assets = Vec::new();
                    for (key, val) in assets.into_iter() {
                        let mut conway_value = Vec::new();
                        for (inner_key, inner_val) in val.into_iter() {
                            conway_value
                                .push((inner_key, PositiveCoin::try_from(inner_val).unwrap()));
                        }
                        conway_assets.push((key, conway_value.into_iter().collect()));
                    }
                    let conway_assets = conway_assets.into_iter().collect();
                    Value::Multiasset(coin, conway_assets)
                }
            }
        }
        TransactionOutput::PostAlonzo(output) => output.value.clone(),
    };
    for output in outputs_iter {
        match output {
            TransactionOutput::Legacy(output) => {
                let amount = output.amount.clone();
                match amount {
                    babbage::Value::Coin(coin) => {
                        res =
                            conway_add_values(&res, &Value::Coin(coin), &PostAlonzo(NegativeValue))?
                    }
                    babbage::Value::Multiasset(coin, assets) => {
                        let mut conway_assets = Vec::new();
                        for (key, val) in assets.into_iter() {
                            let mut conway_value = Vec::new();
                            for (inner_key, inner_val) in val.into_iter() {
                                conway_value
                                    .push((inner_key, PositiveCoin::try_from(inner_val).unwrap()));
                            }
                            conway_assets.push((key, conway_value.into_iter().collect()));
                        }
                        let conway_assets = conway_assets.into_iter().collect();
                        res = conway_add_values(
                            &res,
                            &Value::Multiasset(coin, conway_assets),
                            &PostAlonzo(NegativeValue),
                        )?
                    }
                }
            }
            TransactionOutput::PostAlonzo(output) => {
                res = conway_add_values(&res, &output.value, &PostAlonzo(NegativeValue))?
            }
        }
    }
    Ok(res)
}

fn check_min_lovelace(tx_body: &TransactionBody, prot_pps: &ConwayProtParams) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let val: &Value = match output {
            TransactionOutput::Legacy(output) => {
                let amount = output.amount.clone();
                match amount {
                    babbage::Value::Coin(coin) => &Value::Coin(coin),
                    babbage::Value::Multiasset(coin, assets) => {
                        let mut conway_assets = Vec::new();
                        for (key, val) in assets.into_iter() {
                            let mut conway_value = Vec::new();
                            for (inner_key, inner_val) in val.into_iter() {
                                conway_value
                                    .push((inner_key, PositiveCoin::try_from(inner_val).unwrap()));
                            }
                            conway_assets.push((key, conway_value.into_iter().collect()));
                        }
                        let conway_assets = conway_assets.into_iter().collect();
                        &Value::Multiasset(coin, conway_assets)
                    }
                }
            }
            TransactionOutput::PostAlonzo(output) => &output.value,
        };
        if get_lovelace_from_conway_val(val) < compute_min_lovelace(val, prot_pps) {
            return Err(PostAlonzo(MinLovelaceUnreached));
        }
    }
    Ok(())
}

fn compute_min_lovelace(val: &Value, prot_pps: &ConwayProtParams) -> u64 {
    prot_pps.ada_per_utxo_byte * (conway_get_val_size_in_words(val) + 160)
}

// The size of the value in each of the outputs should not be greater than the
// maximum allowed.
fn check_output_val_size(
    tx_body: &TransactionBody,
    prot_pps: &ConwayProtParams,
) -> ValidationResult {
    for output in tx_body.outputs.iter() {
        let val: &Value = match output {
            TransactionOutput::Legacy(output) => {
                let amount = output.amount.clone();
                match amount {
                    babbage::Value::Coin(coin) => &Value::Coin(coin),
                    babbage::Value::Multiasset(coin, assets) => {
                        let mut conway_assets = Vec::new();
                        for (key, val) in assets.into_iter() {
                            let mut conway_value = Vec::new();
                            for (inner_key, inner_val) in val.into_iter() {
                                conway_value
                                    .push((inner_key, PositiveCoin::try_from(inner_val).unwrap()));
                            }
                            conway_assets.push((key, conway_value.into_iter().collect()));
                        }
                        let conway_assets = conway_assets.into_iter().collect();
                        &Value::Multiasset(coin, conway_assets)
                    }
                }
            }
            TransactionOutput::PostAlonzo(output) => &output.value,
        };
        if conway_get_val_size_in_words(val) > prot_pps.max_value_size as u64 {
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
        let addr_bytes: &Bytes = match output {
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

fn check_tx_size(size: &u32, prot_pps: &ConwayProtParams) -> ValidationResult {
    if *size > prot_pps.max_transaction_size {
        return Err(PostAlonzo(MaxTxSizeExceeded));
    }
    Ok(())
}

fn check_tx_ex_units(mtx: &Tx, prot_pps: &ConwayProtParams) -> ValidationResult {
    let mut steps: u64 = 0;
    let mut mem: u64 = 0;
    for redeemer in MultiEraTx::from_conway(mtx).redeemers() {
        mem += redeemer.ex_units().mem;
        steps += redeemer.ex_units().steps;
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

            let v3_scripts_wits = mtx
                .transaction_witness_set
                .plutus_v3_script
                .iter()
                .flatten()
                .map(compute_plutus_v3_script_hash);

            let ref_scripts = reference_script_inputs(tx_body)
                .filter_map(|x| get_script_hash_from_reference_input(x, utxos));

            let all_scripts_wits: Vec<_> = native_script_wits
                .chain(v1_scripts_wits)
                .chain(v2_scripts_wits)
                .chain(v3_scripts_wits)
                .chain(ref_scripts)
                .collect();

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
    let tx_body: &TransactionBody = &mtx.transaction_body;
    let tx_hash: &Vec<u8> = &Vec::from(mtx.transaction_body.original_hash().as_ref());
    let tx_wits: &WitnessSet = &mtx.transaction_witness_set;
    let vkey_wits: &Option<Vec<VKeyWitness>> = &Some(
        tx_wits
            .vkeywitness
            .clone()
            .map(|wits| wits.to_vec())
            .unwrap_or_default(),
    );
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
    let plutus_v3_scripts: Vec<PolicyId> = match &tx_wits.plutus_v3_script {
        Some(scripts) => scripts
            .clone()
            .iter()
            .map(compute_plutus_v3_script_hash)
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
        &plutus_v3_scripts,
        &reference_scripts,
    )?;
    let plutus_data = tx_wits.plutus_data.clone();
    check_datums(tx_body, utxos, &plutus_data)?;
    // Native reference scripts satisfy script presence, but never need redeemers.
    let plutus_reference_scripts: Vec<PolicyId> = reference_script_inputs(tx_body)
        .filter_map(|input| reference_script(input, utxos))
        .filter(|script| !matches!(script, ScriptRef::NativeScript(_)))
        .map(|script| script_ref_hash(&script))
        .collect();
    check_redeemers(
        &plutus_v1_scripts,
        &plutus_v2_scripts,
        &plutus_v3_scripts,
        &plutus_reference_scripts,
        tx_body,
        tx_wits,
        utxos,
    )?;
    check_certificate_native_scripts(mtx, utxos)?;
    check_required_signers(&tx_body.required_signers, vkey_wits, tx_hash)?;
    check_vkey_input_wits(
        mtx,
        &Some(
            tx_wits
                .vkeywitness
                .clone()
                .map(|wits| wits.to_vec())
                .unwrap_or_default(),
        ),
        utxos,
    )
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
    plutus_v3_scripts: &[PolicyId],
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
    let mut filtered_plutus_v3_scripts: Vec<(bool, PolicyId)> = plutus_v3_scripts
        .iter()
        .map(|&script_hash| (false, script_hash))
        .collect();
    filtered_plutus_v3_scripts
        .retain(|&(_, plutus_v3_script_hash)| !reference_scripts.contains(&plutus_v3_script_hash));
    check_input_scripts(
        tx_body,
        &mut filtered_native_scripts,
        &mut filtered_plutus_v1_scripts,
        &mut filtered_plutus_v2_scripts,
        &mut filtered_plutus_v3_scripts,
        reference_scripts,
        utxos,
    )?;
    check_minting_policies(
        tx_body,
        &mut filtered_native_scripts,
        &mut filtered_plutus_v1_scripts,
        &mut filtered_plutus_v2_scripts,
        &mut filtered_plutus_v3_scripts,
        reference_scripts,
    )?;
    for cert in tx_body.certificates.iter().flatten() {
        if let Some(StakeCredential::ScriptHash(hash)) = certificate_credential(cert) {
            let mut covered = reference_scripts.contains(hash);
            for scripts in [
                &mut filtered_native_scripts,
                &mut filtered_plutus_v1_scripts,
                &mut filtered_plutus_v2_scripts,
                &mut filtered_plutus_v3_scripts,
            ] {
                for (used, supplied) in scripts.iter_mut() {
                    if supplied == hash {
                        *used = true;
                        covered = true;
                    }
                }
            }
            if !covered {
                return Err(PostAlonzo(ScriptWitnessMissing));
            }
        }
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
    for (covered, _) in filtered_plutus_v3_scripts.iter() {
        if !covered {
            return Err(PostAlonzo(UnneededPlutusV2Script));
        }
    }
    Ok(())
}

fn get_reference_script_hashes(tx_body: &TransactionBody, utxos: &UTxOs) -> Vec<PolicyId> {
    let mut res: Vec<PolicyId> = Vec::new();
    for input in reference_script_inputs(tx_body) {
        if let Some(script_hash) = get_script_hash_from_reference_input(input, utxos) {
            res.push(script_hash);
        }
    }
    res
}

// The credential that must witness a certificate. Legacy stake registration
// needs no witness, and pool certificates are witnessed by pool keys.
fn certificate_credential(cert: &Certificate) -> Option<&StakeCredential> {
    match cert {
        Certificate::StakeRegistration(_)
        | Certificate::PoolRegistration { .. }
        | Certificate::PoolRetirement(..) => None,
        Certificate::StakeDeregistration(cred)
        | Certificate::StakeDelegation(cred, _)
        | Certificate::Reg(cred, _)
        | Certificate::UnReg(cred, _)
        | Certificate::VoteDeleg(cred, _)
        | Certificate::StakeVoteDeleg(cred, _, _)
        | Certificate::StakeRegDeleg(cred, _, _)
        | Certificate::VoteRegDeleg(cred, _, _)
        | Certificate::StakeVoteRegDeleg(cred, _, _, _)
        | Certificate::AuthCommitteeHot(cred, _)
        | Certificate::ResignCommitteeCold(cred, _)
        | Certificate::RegDRepCert(cred, _, _)
        | Certificate::UnRegDRepCert(cred, _)
        | Certificate::UpdateDRepCert(cred, _) => Some(cred),
    }
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
) -> Option<ScriptRef<'a>> {
    utxos
        .get(&MultiEraInput::from_alonzo_compatible(input))
        .and_then(MultiEraOutput::script_ref)
}

fn check_certificate_native_scripts(mtx: &Tx, utxos: &UTxOs) -> ValidationResult {
    let body = &mtx.transaction_body;
    let witnesses = &mtx.transaction_witness_set;
    let vkeys = witnesses
        .vkeywitness
        .as_ref()
        .map(|x| x.clone().to_vec())
        .unwrap_or_default();
    let references: Vec<_> = reference_script_inputs(body)
        .map(|input| reference_script(input, utxos))
        .collect();
    let scripts = witnesses
        .native_script
        .iter()
        .flatten()
        .map(|script| (compute_native_script_hash(script), script.deref()))
        .chain(references.iter().filter_map(|script| match script {
            Some(ScriptRef::NativeScript(script)) => Some((script.original_hash(), script.deref())),
            _ => None,
        }));
    for (hash, script) in scripts {
        let needed = body.certificates.iter().flatten().any(|cert| {
            matches!(certificate_credential(cert), Some(StakeCredential::ScriptHash(cred)) if *cred == hash)
        });
        if needed
            && !super::shelley_ma::eval_native_script(
                &vkeys,
                script,
                &body.validity_interval_start,
                &body.ttl,
            )
        {
            return Err(PostAlonzo(NativeScriptDenial));
        }
    }
    Ok(())
}

fn check_input_scripts(
    tx_body: &TransactionBody,
    native_scripts: &mut [(bool, PolicyId)],
    plutus_v1_scripts: &mut [(bool, PolicyId)],
    plutus_v2_scripts: &mut [(bool, PolicyId)],
    pluts_v3_scripts: &mut [(bool, PolicyId)],
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
        for (plutus_v3_script_covered, plutus_v3_script_hash) in pluts_v3_scripts.iter_mut() {
            if *hash == *plutus_v3_script_hash {
                *covered = true;
                *plutus_v3_script_covered = true;
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
    reference_script(ref_input, utxos)
        .as_ref()
        .map(script_ref_hash)
}

fn script_ref_hash(script: &ScriptRef) -> PolicyId {
    match script {
        ScriptRef::NativeScript(script) => script.original_hash(),
        ScriptRef::PlutusV1Script(script) => script.compute_hash(),
        ScriptRef::PlutusV2Script(script) => script.compute_hash(),
        ScriptRef::PlutusV3Script(script) => script.compute_hash(),
    }
}

fn check_minting_policies(
    tx_body: &TransactionBody,
    native_scripts: &mut [(bool, PolicyId)],
    plutus_v1_scripts: &mut [(bool, PolicyId)],
    plutus_v2_scripts: &mut [(bool, PolicyId)],
    plutus_v3_scripts: &mut [(bool, PolicyId)],
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
                for (plutus_script_covered, plutus_v3_script_hash) in plutus_v3_scripts.iter_mut() {
                    if *policy == *plutus_v3_script_hash {
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
    option_plutus_data: &Option<KeepRaw<NonEmptySet<KeepRaw<PlutusData>>>>,
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

        // we only check for datum in the witness set if it's not an inline datum in the
        // output (aka: DatumOption::Hash).
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
        match babbage_output {
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
    plutus_v3_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
    tx_body: &TransactionBody,
    tx_wits: &WitnessSet,
    utxos: &UTxOs,
) -> ValidationResult {
    let redeemer_key: Vec<RedeemersKey> = match &tx_wits.redeemer {
        Some(redeemers) => match redeemers.clone().unwrap() {
            Redeemers::List(redeemers) => redeemers
                .iter()
                .map(|x| RedeemersKey {
                    tag: x.tag,
                    index: x.index,
                })
                .collect(),
            Redeemers::Map(redeemers) => redeemers
                .iter()
                .map(|x| RedeemersKey {
                    tag: x.0.tag,
                    index: x.0.index,
                })
                .collect(),
        },

        None => Vec::new(),
    };

    let plutus_scripts: Vec<RedeemersKey> = mk_plutus_script_redeemer_pointers(
        plutus_v1_scripts,
        plutus_v2_scripts,
        plutus_v3_scripts,
        reference_scripts,
        tx_body,
        utxos,
    )?;
    redeemer_key_coincide(&redeemer_key, &plutus_scripts)
}

// Lexicographical sorting for inputs.
fn sort_inputs(unsorted_inputs: &[TransactionInput]) -> Vec<TransactionInput> {
    let mut res: Vec<TransactionInput> = unsorted_inputs.to_owned();
    res.sort();
    res
}

// Sorting function for reward accounts (withdrawals).
fn sort_reward_accounts(a: &StakeAddress, b: &StakeAddress) -> Ordering {
    fn network_tag(network: Network) -> u8 {
        match network {
            Network::Testnet => 0,
            Network::Mainnet => 1,
            Network::Other(tag) => tag,
        }
    }

    if a.network() != b.network() {
        return network_tag(a.network()).cmp(&network_tag(b.network()));
    }

    match (a.payload(), b.payload()) {
        (StakePayload::Script(..), StakePayload::Stake(..)) => Ordering::Less,
        (StakePayload::Stake(..), StakePayload::Script(..)) => Ordering::Greater,
        (StakePayload::Script(hash_a), StakePayload::Script(hash_b)) => hash_a.cmp(hash_b),
        (StakePayload::Stake(hash_a), StakePayload::Stake(hash_b)) => hash_a.cmp(hash_b),
    }
}

fn mk_plutus_script_redeemer_pointers(
    plutus_v1_scripts: &[PolicyId],
    plutus_v2_scripts: &[PolicyId],
    plutus_v3_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
    tx_body: &TransactionBody,
    utxos: &UTxOs,
) -> Result<Vec<RedeemersKey>, ValidationError> {
    let mut res: Vec<RedeemersKey> = Vec::new();
    let sorted_inputs: &Vec<TransactionInput> = &sort_inputs(&tx_body.inputs);
    for (index, input) in sorted_inputs.iter().enumerate() {
        if let Some(script_hash) = get_script_hash_from_input(input, utxos) {
            // Only create redeemer pointer for Plutus scripts, not native scripts
            if is_phase_2_script(
                &script_hash,
                plutus_v1_scripts,
                plutus_v2_scripts,
                plutus_v3_scripts,
                reference_scripts,
            ) {
                res.push(RedeemersKey {
                    tag: pallas_primitives::conway::RedeemerTag::Spend,
                    index: index as u32,
                })
            }
        }
    }
    if let Some(mint) = &tx_body.mint {
        for (index, policy) in sort_policies(mint).iter().enumerate() {
            if is_phase_2_script(
                policy,
                plutus_v1_scripts,
                plutus_v2_scripts,
                plutus_v3_scripts,
                reference_scripts,
            ) {
                res.push(RedeemersKey {
                    tag: pallas_primitives::conway::RedeemerTag::Mint,
                    index: index as u32,
                })
            }
        }
    }
    for (index, cert) in tx_body.certificates.iter().flatten().enumerate() {
        if let Some(StakeCredential::ScriptHash(hash)) = certificate_credential(cert)
            && is_phase_2_script(
                hash,
                plutus_v1_scripts,
                plutus_v2_scripts,
                plutus_v3_scripts,
                reference_scripts,
            )
        {
            res.push(RedeemersKey {
                tag: pallas_primitives::conway::RedeemerTag::Cert,
                index: index as u32,
            });
        }
    }
    if let Some(withdrawals) = &tx_body.withdrawals {
        let mut parsed: Vec<StakeAddress> = withdrawals
            .keys()
            .map(|bytes| match Address::from_bytes(bytes) {
                Ok(Address::Stake(addr)) => Ok(addr),
                _ => Err(PostAlonzo(InputDecoding)),
            })
            .collect::<Result<_, _>>()?;
        parsed.sort_by(sort_reward_accounts);
        for (index, stake_addr) in parsed.iter().enumerate() {
            if stake_addr.is_script() {
                let script_hash = stake_addr.payload().as_hash();
                if is_phase_2_script(
                    script_hash,
                    plutus_v1_scripts,
                    plutus_v2_scripts,
                    plutus_v3_scripts,
                    reference_scripts,
                ) {
                    res.push(RedeemersKey {
                        tag: pallas_primitives::conway::RedeemerTag::Reward,
                        index: index as u32,
                    })
                }
            }
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
    plutus_v3_scripts: &[PolicyId],
    reference_scripts: &[PolicyId],
) -> bool {
    plutus_v1_scripts
        .iter()
        .any(|v1_script| policy == v1_script)
        || plutus_v2_scripts
            .iter()
            .any(|v2_script| policy == v2_script)
        || plutus_v3_scripts
            .iter()
            .any(|v3_script| policy == v3_script)
        || reference_scripts
            .iter()
            .any(|ref_script| policy == ref_script)
}

fn redeemer_key_coincide(
    redeemers: &[RedeemersKey],
    plutus_scripts: &[RedeemersKey],
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
    let bootstrap_wits: &mut Vec<(bool, BootstrapWitness)> = &mut mtx
        .transaction_witness_set
        .bootstrap_witness
        .iter()
        .flatten()
        .map(|wit| (false, wit.clone()))
        .collect();
    let mut inputs_and_collaterals: Vec<TransactionInput> = Vec::new();
    inputs_and_collaterals.extend(tx_body.inputs.clone().to_vec());
    if let Some(collaterals) = &tx_body.collateral {
        inputs_and_collaterals.extend(collaterals.clone().to_vec())
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
                Ok(Address::Byron(address)) => {
                    check_bootstrap_wit(&address, bootstrap_wits, tx_hash)?
                }
                _ => return Err(PostAlonzo(InputDecoding)),
            },
            None => return Err(PostAlonzo(InputNotInUTxO)),
        }
    }
    for cert in tx_body.certificates.iter().flatten() {
        if let Some(StakeCredential::AddrKeyhash(hash)) = certificate_credential(cert) {
            check_vk_wit(hash, vk_wits, tx_hash)?;
        }
        match cert {
            Certificate::PoolRegistration {
                operator,
                pool_owners,
                ..
            } => {
                check_vk_wit(operator, vk_wits, tx_hash)?;
                for owner in pool_owners.iter() {
                    check_vk_wit(owner, vk_wits, tx_hash)?;
                }
            }
            Certificate::PoolRetirement(operator, _) => check_vk_wit(operator, vk_wits, tx_hash)?,
            _ => {}
        }
    }
    check_remaining_vk_wits(vk_wits, tx_hash)?; // required for native scripts
    check_remaining_bootstrap_wits(bootstrap_wits, tx_hash)
}

fn check_vk_wit(
    payment_key_hash: &AddrKeyhash,
    wits: &mut [(bool, VKeyWitness)],
    data_to_verify: &[u8],
) -> ValidationResult {
    for (vkey_wit_covered, vkey_wit) in wits.iter_mut() {
        let vkey_hash = pallas_crypto::hash::Hasher::<224>::hash(&vkey_wit.vkey);
        if vkey_hash == *payment_key_hash {
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

fn check_bootstrap_wit(
    address: &ByronAddress,
    wits: &mut [(bool, BootstrapWitness)],
    data_to_verify: &[u8],
) -> ValidationResult {
    let root: AddressId = address
        .decode()
        .map_err(|_| PostAlonzo(InputDecoding))?
        .root;
    for (wit_covered, wit) in wits.iter_mut() {
        if bootstrap_wit_root(wit) == root {
            if !verify_bootstrap_signature(wit, data_to_verify) {
                return Err(PostAlonzo(VKWrongSignature));
            }
            *wit_covered = true;
            return Ok(());
        }
    }
    Err(PostAlonzo(VKWitnessMissing))
}

fn check_remaining_bootstrap_wits(
    wits: &mut [(bool, BootstrapWitness)],
    data_to_verify: &[u8],
) -> ValidationResult {
    for (covered, wit) in wits {
        if !*covered && !verify_bootstrap_signature(wit, data_to_verify) {
            return Err(PostAlonzo(VKWrongSignature));
        }
    }
    Ok(())
}

fn verify_bootstrap_signature(wit: &BootstrapWitness, data_to_verify: &[u8]) -> bool {
    match (
        PublicKey::try_from(wit.public_key.as_slice()),
        Signature::try_from(wit.signature.as_slice()),
    ) {
        (Ok(key), Ok(signature)) => key.verify(data_to_verify, &signature),
        _ => false,
    }
}

fn bootstrap_wit_root(wit: &BootstrapWitness) -> AddressId {
    // CBOR heads of `[0, [0, bytes(64)], attributes]`, the root preimage of a key address.
    let prefix: &[u8] = &[0x83, 0x00, 0x82, 0x00, 0x58, 0x40];
    let payload: Vec<u8> = [
        prefix,
        wit.public_key.as_slice(),
        wit.chain_code.as_slice(),
        wit.attributes.as_slice(),
    ]
    .concat();
    pallas_crypto::hash::Hasher::<224>::hash(&cryptoxide::hashing::sha3_256(&payload))
}

fn check_remaining_vk_wits(
    wits: &mut [(bool, VKeyWitness)],
    data_to_verify: &[u8],
) -> ValidationResult {
    for (covered, vkey_wit) in wits {
        if !*covered && !verify_signature(vkey_wit, data_to_verify) {
            return Err(PostAlonzo(VKWrongSignature));
        }
    }
    Ok(())
}

fn check_languages(mtx: &Tx, utxos: &UTxOs, prot_pps: &ConwayProtParams) -> ValidationResult {
    let used_langs = tx_languages(mtx, utxos);
    let allowed_langs: Vec<Language> = allowed_tx_langs(mtx, utxos);
    let available_langs: Vec<Language> = available_langs(prot_pps);

    for tx_lang in used_langs.iter() {
        if !available_langs.contains(tx_lang) && !allowed_langs.contains(tx_lang) {
            return Err(PostAlonzo(UnsupportedPlutusLanguage));
        }
    }

    Ok(())
}

fn available_langs(prot_pps: &ConwayProtParams) -> Vec<Language> {
    let mut res: Vec<Language> = Vec::new();

    if prot_pps
        .cost_models_for_script_languages
        .plutus_v1
        .is_some()
    {
        res.push(Language::PlutusV1);
    }

    if prot_pps
        .cost_models_for_script_languages
        .plutus_v2
        .is_some()
    {
        res.push(Language::PlutusV2);
    }

    if prot_pps
        .cost_models_for_script_languages
        .plutus_v3
        .is_some()
    {
        res.push(Language::PlutusV3);
    }

    res
}

fn allowed_tx_langs(mtx: &Tx, utxos: &UTxOs) -> Vec<Language> {
    let input_features = reference_script_inputs(&mtx.transaction_body)
        .filter_map(|input| utxos.get(&MultiEraInput::from_alonzo_compatible(input)))
        .map(output_language_features);
    let output_features = mtx
        .transaction_body
        .outputs
        .iter()
        .map(|output| output_language_features(&MultiEraOutput::from_conway(output)));
    let (byron, modern) = input_features
        .chain(output_features)
        .fold((false, false), |(byron, modern), (b, m)| {
            (byron || b, modern || m)
        });
    if byron {
        vec![]
    } else if modern
        || any_reference_inputs(
            &mtx.transaction_body
                .reference_inputs
                .clone()
                .map(|x| x.to_vec()),
        )
    {
        vec![Language::PlutusV2, Language::PlutusV3]
    } else {
        vec![Language::PlutusV1, Language::PlutusV2, Language::PlutusV3]
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
    let mut v3_scripts: bool = false;
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
    if let Some(v3_scripts_vec) = &mtx.transaction_witness_set.plutus_v3_script
        && !v3_scripts_vec.is_empty()
    {
        v3_scripts = true;
    }
    let needed = needed_script_hashes(&mtx.transaction_body, utxos);
    for input in reference_script_inputs(&mtx.transaction_body) {
        let Some(script) = reference_script(input, utxos) else {
            continue;
        };
        if !get_script_hash_from_reference_input(input, utxos)
            .is_some_and(|hash| needed.contains(&hash))
        {
            continue;
        }
        match script {
            ScriptRef::PlutusV1Script(_) => v1_scripts = true,
            ScriptRef::PlutusV2Script(_) => v2_scripts = true,
            ScriptRef::PlutusV3Script(_) => v3_scripts = true,
            ScriptRef::NativeScript(_) => (),
        }
    }
    let mut langs = vec![];
    if v1_scripts {
        langs.push(Language::PlutusV1);
    }
    if v2_scripts {
        langs.push(Language::PlutusV2);
    }
    if v3_scripts {
        langs.push(Language::PlutusV3);
    }
    langs
}

fn needed_script_hashes(tx_body: &TransactionBody, utxos: &UTxOs) -> Vec<ScriptHash> {
    let inputs = tx_body
        .inputs
        .iter()
        .filter_map(|input| get_script_hash_from_input(input, utxos));
    let policies = tx_body.mint.iter().flat_map(|mint| mint.keys().copied());
    let certificates = tx_body.certificates.iter().flatten().filter_map(|cert| {
        match certificate_credential(cert) {
            Some(StakeCredential::ScriptHash(hash)) => Some(*hash),
            _ => None,
        }
    });
    let withdrawals = tx_body
        .withdrawals
        .iter()
        .flat_map(|withdrawals| withdrawals.keys())
        .filter_map(|account| get_script_hash_from_reward_account(account));
    let voters = tx_body
        .voting_procedures
        .iter()
        .flat_map(|votes| votes.keys())
        .filter_map(|voter| match voter {
            Voter::ConstitutionalCommitteeScript(hash) | Voter::DRepScript(hash) => Some(*hash),
            _ => None,
        });
    let proposals = tx_body
        .proposal_procedures
        .iter()
        .flatten()
        .filter_map(|proposal| match &proposal.gov_action {
            GovAction::ParameterChange(_, _, policy)
            | GovAction::TreasuryWithdrawals(_, policy) => *policy,
            _ => None,
        });
    inputs
        .chain(policies)
        .chain(certificates)
        .chain(withdrawals)
        .chain(voters)
        .chain(proposals)
        .collect()
}

// The metadata of the transaction is valid.
fn check_auxiliary_data(tx_body: &TransactionBody, mtx: &Tx) -> ValidationResult {
    match (&tx_body.auxiliary_data_hash, aux_data_from_conway_tx(mtx)) {
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
    prot_pps: &ConwayProtParams,
) -> ValidationResult {
    let tx_languages = tx_languages(mtx, utxos);

    let Some(provided) = tx_body.script_data_hash else {
        if tx_languages.is_empty() {
            return Ok(());
        } else {
            return Err(PostAlonzo(ScriptIntegrityHash));
        }
    };

    let Some(language_views) = cost_model_for_tx(&tx_languages, prot_pps) else {
        return Err(PostAlonzo(ScriptIntegrityHash));
    };

    let expected = pallas_primitives::conway::ScriptData::build_for(
        &mtx.transaction_witness_set,
        &Some(language_views),
    )
    .ok_or(PostAlonzo(ScriptIntegrityHash))?
    .hash();

    if provided != expected {
        return Err(PostAlonzo(ScriptIntegrityHash));
    }

    Ok(())
}

fn cost_model_for_tx(
    tx_languages: &[Language],
    prot_pps: &ConwayProtParams,
) -> Option<LanguageViews> {
    let cost_models = &prot_pps.cost_models_for_script_languages;
    let mut map = std::collections::BTreeMap::new();
    for lang in tx_languages {
        let costs = match lang {
            Language::PlutusV1 => cost_models.plutus_v1.clone()?,
            Language::PlutusV2 => cost_models.plutus_v2.clone()?,
            Language::PlutusV3 => cost_models.plutus_v3.clone()?,
        };
        map.insert(lang.clone() as u8, costs);
    }
    Some(LanguageViews(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase1::fixtures::{
        V2_SCRIPT, byron_address, byron_output, input, key_address, legacy_output,
        output_with_reference_script, reward_address, script_address, tx_referencing_input,
        tx_spending_input, undecodable_byron_address, utxos,
    };
    use pallas_codec::minicbor;
    use pallas_crypto::{hash::Hasher, key::ed25519::SecretKey};
    use pallas_primitives::{
        PlutusScript, StakeCredential,
        alonzo::NativeScript,
        conway::{Certificate, DRep, ExUnits, PostAlonzoTransactionOutput, Redeemer, RedeemerTag},
    };
    use pallas_traverse::Era;

    const KEY_DEPOSIT: u64 = 2_000_000;
    const DREP_DEPOSIT: u64 = 5_000_000;
    const PROPOSAL_DEPOSIT: u64 = 7_000_000;
    const INPUT: u64 = 20_000_000;

    fn output(coin: u64) -> TransactionOutput<'static> {
        let mut address = vec![0x61];
        address.extend([0; 28]);
        TransactionOutput::PostAlonzo(
            PostAlonzoTransactionOutput {
                address: address.into(),
                value: Value::Coin(coin),
                datum_option: None,
                script_ref: None,
            }
            .into(),
        )
    }

    // Balance results for a body whose deposits less refunds come to `net`, with
    // the output short by `net - 1`, `net` and `net + 1`.
    fn balance_around(
        certificates: Vec<Certificate>,
        prepare: impl Fn(&mut TransactionBody),
        net: i64,
    ) -> Vec<String> {
        let raw = hex::decode(include_str!("../../../test_data/conway3.tx").trim()).unwrap();
        let tx: Tx = minicbor::decode(&raw).unwrap();
        let input = output(INPUT);
        let utxos = UTxOs::from([(
            MultiEraInput::from_alonzo_compatible(&tx.transaction_body.inputs[0]),
            MultiEraOutput::from_conway(&input),
        )]);
        [net - 1, net, net + 1]
            .into_iter()
            .map(|reserved| {
                let mut body = (*tx.transaction_body).clone();
                body.fee = 200_000;
                body.mint = None;
                body.outputs = vec![output(((INPUT - body.fee) as i64 - reserved) as u64)];
                body.certificates = certificates.clone().try_into().ok();
                prepare(&mut body);
                let result = check_preservation_of_value(
                    &body,
                    &utxos,
                    KEY_DEPOSIT,
                    DREP_DEPOSIT,
                    PROPOSAL_DEPOSIT,
                );
                format!("{result:?}")
            })
            .collect()
    }

    #[test]
    fn deposits_refunds_withdrawals_and_donations_balance() {
        let credential = |id| StakeCredential::AddrKeyhash([id; 28].into());
        let none = |_: &mut TransactionBody| {};
        let reward_account = [vec![0xe0], vec![3; 28]].concat();
        let proposal = pallas_primitives::conway::ProposalProcedure {
            deposit: PROPOSAL_DEPOSIT,
            reward_account: reward_account.clone().into(),
            gov_action: pallas_primitives::conway::GovAction::Information,
            anchor: pallas_primitives::conway::Anchor {
                url: String::new(),
                content_hash: [0; 32].into(),
            },
        };
        let cases: Vec<(&str, Vec<String>)> = vec![
            ("nothing", balance_around(vec![], none, 0)),
            (
                "deregistration refunds its declared deposit",
                balance_around(
                    vec![Certificate::UnReg(credential(2), 3_000_000)],
                    none,
                    -3_000_000,
                ),
            ),
            (
                "drep registration charges the drep deposit",
                balance_around(
                    vec![Certificate::RegDRepCert(credential(1), DREP_DEPOSIT, None)],
                    none,
                    DREP_DEPOSIT as i64,
                ),
            ),
            (
                "withdrawal",
                balance_around(
                    vec![],
                    |body| {
                        body.withdrawals = Some(
                            [(reward_account.clone().into(), 1_500_000)]
                                .into_iter()
                                .collect(),
                        )
                    },
                    -1_500_000,
                ),
            ),
            (
                "donation",
                balance_around(
                    vec![],
                    |body| body.donation = Some(600_000.try_into().unwrap()),
                    600_000,
                ),
            ),
            (
                "two proposals",
                balance_around(
                    vec![],
                    |body| {
                        body.proposal_procedures =
                            Some(vec![proposal.clone(), proposal.clone()].try_into().unwrap())
                    },
                    2 * PROPOSAL_DEPOSIT as i64,
                ),
            ),
        ];
        let unbalanced = "Err(PostAlonzo(PreservationOfValue))".to_string();
        let expected: Vec<(&str, Vec<String>)> = cases
            .iter()
            .map(|(name, _)| {
                (
                    *name,
                    vec![unbalanced.clone(), "Ok(())".to_string(), unbalanced.clone()],
                )
            })
            .collect();
        assert_eq!(cases, expected);
    }

    #[test]
    fn each_certificate_charges_or_refunds_its_deposit() {
        let credential = StakeCredential::AddrKeyhash([1; 28].into());
        let pool = [2; 28].into();
        let cases = [
            (Certificate::StakeRegistration(credential.clone()), (2, 0)),
            (Certificate::Reg(credential.clone(), 9), (2, 0)),
            (
                Certificate::StakeRegDeleg(credential.clone(), pool, 9),
                (2, 0),
            ),
            (
                Certificate::VoteRegDeleg(credential.clone(), DRep::Abstain, 9),
                (2, 0),
            ),
            (
                Certificate::StakeVoteRegDeleg(credential.clone(), pool, DRep::Abstain, 9),
                (2, 0),
            ),
            (
                Certificate::RegDRepCert(credential.clone(), 9, None),
                (5, 0),
            ),
            (Certificate::StakeDeregistration(credential.clone()), (0, 2)),
            (Certificate::UnReg(credential.clone(), 7), (0, 7)),
            (Certificate::UnRegDRepCert(credential.clone(), 8), (0, 8)),
            (
                Certificate::StakeDelegation(credential.clone(), pool),
                (0, 0),
            ),
            (Certificate::VoteDeleg(credential, DRep::Abstain), (0, 0)),
        ];
        let actual: Vec<_> = cases
            .iter()
            .map(|(cert, _)| certificate_deposit_and_refund(cert, 2, 5))
            .collect();
        let expected: Vec<_> = cases.iter().map(|(_, sums)| *sums).collect();
        assert_eq!(actual, expected);
    }

    fn result(actual: ValidationResult, expected: &str) {
        assert_eq!(format!("{actual:?}"), expected);
    }

    fn credential_certificates(cred: StakeCredential) -> Vec<Certificate> {
        let pool = [9; 28].into();
        vec![
            Certificate::StakeDeregistration(cred.clone()),
            Certificate::StakeDelegation(cred.clone(), pool),
            Certificate::Reg(cred.clone(), 2_000_000),
            Certificate::UnReg(cred.clone(), 2_000_000),
            Certificate::VoteDeleg(cred.clone(), DRep::Abstain),
            Certificate::StakeVoteDeleg(cred.clone(), pool, DRep::Abstain),
            Certificate::StakeRegDeleg(cred.clone(), pool, 2_000_000),
            Certificate::VoteRegDeleg(cred.clone(), DRep::Abstain, 2_000_000),
            Certificate::StakeVoteRegDeleg(cred.clone(), pool, DRep::Abstain, 2_000_000),
            Certificate::AuthCommitteeHot(
                cred.clone(),
                StakeCredential::AddrKeyhash([8; 28].into()),
            ),
            Certificate::ResignCommitteeCold(cred.clone(), None),
            Certificate::RegDRepCert(cred.clone(), 500_000_000, None),
            Certificate::UnRegDRepCert(cred.clone(), 500_000_000),
            Certificate::UpdateDRepCert(cred, None),
        ]
    }

    // Re-encodes and decodes the edited transaction, so the body hash the keys
    // sign is the hash of the edited body.
    fn synthetic(
        certificates: Vec<Certificate>,
        prepare: impl FnOnce(&mut Tx<'_>),
        keys: &[u8],
        test: impl FnOnce(Tx<'_>),
    ) {
        let raw = hex::decode(include_str!("../../../test_data/conway3.tx").trim()).unwrap();
        let mut tx: Tx = minicbor::decode(&raw).unwrap();
        tx.transaction_body.inputs = Vec::new().into();
        tx.transaction_body.collateral = None;
        tx.transaction_body.reference_inputs = None;
        tx.transaction_body.required_signers = None;
        tx.transaction_body.withdrawals = None;
        tx.transaction_body.mint = None;
        tx.transaction_body.certificates = certificates.try_into().ok();
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
        tx.transaction_witness_set.vkeywitness = witnesses.try_into().ok();
        test(tx);
    }

    fn key_hash(id: u8) -> AddrKeyhash {
        Hasher::<224>::hash(SecretKey::from([id; 32]).public_key().as_ref())
    }

    fn plutus() -> PlutusScript<3> {
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

    fn add_plutus(tx: &mut Tx<'_>, pointers: &[(u32, RedeemerTag)], map: bool) {
        tx.transaction_witness_set.plutus_v3_script = Some(vec![plutus()].try_into().unwrap());
        let list: Vec<_> = pointers
            .iter()
            .map(|(index, tag)| redeemer(*index, *tag))
            .collect();
        let redeemers = if map {
            Redeemers::Map(
                list.into_iter()
                    .map(|r| {
                        (
                            RedeemersKey {
                                tag: r.tag,
                                index: r.index,
                            },
                            pallas_primitives::conway::RedeemersValue {
                                data: r.data,
                                ex_units: r.ex_units,
                            },
                        )
                    })
                    .collect(),
            )
        } else {
            Redeemers::List(list)
        };
        tx.transaction_witness_set.redeemer = Some(redeemers.into());
    }

    #[test]
    fn every_credential_certificate_requires_its_key_and_valid_signature() {
        for cert in credential_certificates(StakeCredential::AddrKeyhash(key_hash(1))) {
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
                    let mut vkeys = tx
                        .transaction_witness_set
                        .vkeywitness
                        .clone()
                        .unwrap()
                        .to_vec();
                    vkeys[0].signature = vec![0; 64].into();
                    tx.transaction_witness_set.vkeywitness = Some(vkeys.try_into().unwrap());
                    result(
                        check_witness_set(&tx, &UTxOs::new()),
                        "Err(PostAlonzo(VKWrongSignature))",
                    );
                },
            );
        }
    }

    #[test]
    fn every_credential_certificate_requires_script_and_plutus_redeemer() {
        let cred = StakeCredential::ScriptHash(compute_plutus_v3_script_hash(&plutus()));
        for cert in credential_certificates(cred) {
            for map in [false, true] {
                synthetic(
                    vec![cert.clone()],
                    |tx| add_plutus(tx, &[(0, RedeemerTag::Cert)], map),
                    &[],
                    |tx| result(check_witness_set(&tx, &UTxOs::new()), "Ok(())"),
                );
            }
            synthetic(
                vec![cert.clone()],
                |_| {},
                &[],
                |tx| {
                    result(
                        check_witness_set(&tx, &UTxOs::new()),
                        "Err(PostAlonzo(ScriptWitnessMissing))",
                    )
                },
            );
            synthetic(
                vec![cert.clone()],
                |tx| add_plutus(tx, &[], false),
                &[],
                |tx| {
                    result(
                        check_witness_set(&tx, &UTxOs::new()),
                        "Err(PostAlonzo(RedeemerMissing))",
                    )
                },
            );
            synthetic(
                vec![cert],
                |tx| {
                    add_plutus(tx, &[(0, RedeemerTag::Cert)], true);
                    tx.transaction_witness_set.plutus_v3_script =
                        Some(vec![PlutusScript(vec![99].into())].try_into().unwrap());
                },
                &[],
                |tx| {
                    result(
                        check_witness_set(&tx, &UTxOs::new()),
                        "Err(PostAlonzo(ScriptWitnessMissing))",
                    )
                },
            );
        }
    }

    #[test]
    fn certificate_pointers_use_full_positions_and_distinct_purposes() {
        let hash = compute_plutus_v3_script_hash(&plutus());
        let certs = vec![
            Certificate::StakeRegistration(StakeCredential::ScriptHash([0; 28].into())),
            Certificate::Reg(StakeCredential::AddrKeyhash(key_hash(1)), 2_000_000),
            Certificate::Reg(StakeCredential::ScriptHash(hash), 2_000_000),
            Certificate::VoteDeleg(StakeCredential::ScriptHash(hash), DRep::Abstain),
        ];
        for map in [false, true] {
            for (pointers, expected) in [
                (
                    vec![(2, RedeemerTag::Cert), (3, RedeemerTag::Cert)],
                    "Ok(())",
                ),
                (
                    vec![(2, RedeemerTag::Cert)],
                    "Err(PostAlonzo(RedeemerMissing))",
                ),
                (
                    vec![(0, RedeemerTag::Cert), (1, RedeemerTag::Cert)],
                    "Err(PostAlonzo(UnneededRedeemer))",
                ),
                (
                    vec![(2, RedeemerTag::Spend), (3, RedeemerTag::Cert)],
                    "Err(PostAlonzo(UnneededRedeemer))",
                ),
                (
                    vec![(2, RedeemerTag::Cert), (4, RedeemerTag::Cert)],
                    "Err(PostAlonzo(UnneededRedeemer))",
                ),
            ] {
                synthetic(
                    certs.clone(),
                    |tx| add_plutus(tx, &pointers, map),
                    &[1],
                    |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
                );
            }
        }
    }

    #[test]
    fn native_certificate_checks_authorization_without_redeemers() {
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
                        tx.transaction_witness_set.native_script =
                            Some(vec![script.clone().into()].try_into().unwrap());
                    },
                    keys,
                    |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
                );
            }
        }
        // A bad signature from the script key fails even after a good signature
        // from another key.
        synthetic(
            vec![Certificate::Reg(
                StakeCredential::ScriptHash(compute_native_script_hash(&script)),
                2_000_000,
            )],
            |tx| {
                tx.transaction_witness_set.native_script =
                    Some(vec![script.into()].try_into().unwrap());
            },
            &[2, 1],
            |mut tx| {
                let mut vkeys = tx
                    .transaction_witness_set
                    .vkeywitness
                    .clone()
                    .unwrap()
                    .to_vec();
                vkeys[1].signature = vec![0; 64].into();
                tx.transaction_witness_set.vkeywitness = Some(vkeys.try_into().unwrap());
                result(
                    check_witness_set(&tx, &UTxOs::new()),
                    "Err(PostAlonzo(VKWrongSignature))",
                );
            },
        );
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
            pool_owners: vec![key_hash(2)].into(),
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
    fn native_certificate_timelocks_and_thresholds_are_evaluated() {
        let script = NativeScript::ScriptAll(vec![
            NativeScript::ScriptNOfK(
                1,
                vec![
                    NativeScript::ScriptPubkey(key_hash(1)),
                    NativeScript::ScriptPubkey(key_hash(2)),
                ],
            ),
            NativeScript::InvalidBefore(10),
            NativeScript::InvalidHereafter(20),
        ]);
        for (start, end, expected) in [
            (Some(10), Some(20), "Ok(())"),
            (None, Some(20), "Err(PostAlonzo(NativeScriptDenial))"),
            (Some(9), Some(20), "Err(PostAlonzo(NativeScriptDenial))"),
            (Some(10), Some(21), "Err(PostAlonzo(NativeScriptDenial))"),
        ] {
            synthetic(
                vec![Certificate::Reg(
                    StakeCredential::ScriptHash(compute_native_script_hash(&script)),
                    2_000_000,
                )],
                |tx| {
                    tx.transaction_body.validity_interval_start = start;
                    tx.transaction_body.ttl = end;
                    tx.transaction_witness_set.native_script =
                        Some(vec![script.clone().into()].try_into().unwrap());
                },
                &[2],
                |tx| result(check_witness_set(&tx, &UTxOs::new()), expected),
            );
        }
    }

    #[test]
    fn all_plutus_versions_can_supply_certificate_scripts() {
        let v1 = PlutusScript::<1>(vec![1, 2].into());
        let v2 = PlutusScript::<2>(vec![1, 2].into());
        let v3 = plutus();
        for (version, hash) in [
            (1, compute_plutus_v1_script_hash(&v1)),
            (2, compute_plutus_v2_script_hash(&v2)),
            (3, compute_plutus_v3_script_hash(&v3)),
        ] {
            synthetic(
                vec![Certificate::Reg(
                    StakeCredential::ScriptHash(hash),
                    2_000_000,
                )],
                |tx| {
                    match version {
                        1 => {
                            tx.transaction_witness_set.plutus_v1_script =
                                Some(vec![v1.clone()].try_into().unwrap())
                        }
                        2 => {
                            tx.transaction_witness_set.plutus_v2_script =
                                Some(vec![v2.clone()].try_into().unwrap())
                        }
                        _ => {
                            tx.transaction_witness_set.plutus_v3_script =
                                Some(vec![v3.clone()].try_into().unwrap())
                        }
                    }
                    tx.transaction_witness_set.redeemer =
                        Some(Redeemers::List(vec![redeemer(0, RedeemerTag::Cert)]).into());
                },
                &[],
                |tx| result(check_witness_set(&tx, &UTxOs::new()), "Ok(())"),
            );
        }
    }

    #[test]
    fn native_reference_certificate_needs_no_redeemer() {
        use pallas_codec::utils::CborWrap;
        let script = NativeScript::ScriptPubkey(key_hash(1));
        let hash = compute_native_script_hash(&script);
        let input = TransactionInput {
            transaction_id: [7; 32].into(),
            index: 0,
        };
        for as_spending in [false, true] {
            for keys in [&[1][..], &[][..]] {
                synthetic(
                    vec![Certificate::Reg(
                        StakeCredential::ScriptHash(hash),
                        2_000_000,
                    )],
                    |tx| {
                        if as_spending {
                            tx.transaction_body.inputs = vec![input.clone()].into();
                        } else {
                            tx.transaction_body.reference_inputs =
                                Some(vec![input.clone()].try_into().unwrap());
                        }
                    },
                    keys,
                    |tx| {
                        // Script payment address, so spending it needs no key witness.
                        let mut address = vec![0x71];
                        address.extend(hash.as_ref());
                        let output = TransactionOutput::PostAlonzo(
                            PostAlonzoTransactionOutput {
                                address: address.into(),
                                value: Value::Coin(2_000_000),
                                datum_option: None,
                                script_ref: Some(CborWrap(ScriptRef::NativeScript(
                                    script.clone().into(),
                                ))),
                            }
                            .into(),
                        );
                        let encoded = minicbor::to_vec(output).unwrap();
                        let output: TransactionOutput = minicbor::decode(&encoded).unwrap();
                        let utxos = UTxOs::from([(
                            MultiEraInput::from_alonzo_compatible(&input),
                            MultiEraOutput::from_conway(&output),
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
    }

    // Witness set and minting results for a script by reference that spends,
    // mints, withdraws and certifies, with the reference script on a reference
    // input or a spent input.
    fn reference_script_outcomes(
        script: ScriptRef<'static>,
        hash: PolicyId,
    ) -> Vec<(RedeemerTag, bool, String)> {
        use pallas_codec::utils::{CborWrap, NonZeroInt};
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
                MultiEraOutput::from_conway(&held),
            ),
            (
                MultiEraInput::from_alonzo_compatible(&locked),
                MultiEraOutput::from_conway(&locked_output),
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
                    RedeemerTag::Cert => vec![Certificate::Reg(
                        StakeCredential::ScriptHash(hash),
                        2_000_000,
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
                            body.reference_inputs = Some(vec![holder.clone()].try_into().unwrap());
                        }
                        match purpose {
                            RedeemerTag::Spend => inputs.push(locked.clone()),
                            RedeemerTag::Mint => {
                                let assets =
                                    vec![(Bytes::from(vec![1]), NonZeroInt::try_from(1).unwrap())];
                                body.mint = Some(
                                    [(hash, assets.into_iter().collect())].into_iter().collect(),
                                );
                            }
                            RedeemerTag::Reward => {
                                body.withdrawals = Some(
                                    [(reward_account.clone().into(), 0)].into_iter().collect(),
                                );
                            }
                            _ => {}
                        }
                        body.inputs = inputs.into();
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
        let v2 = PlutusScript::<2>(vec![1, 2].into());
        for (version, script, hash) in [
            (
                1,
                ScriptRef::PlutusV1Script(v1.clone()),
                compute_plutus_v1_script_hash(&v1),
            ),
            (
                2,
                ScriptRef::PlutusV2Script(v2.clone()),
                compute_plutus_v2_script_hash(&v2),
            ),
            (
                3,
                ScriptRef::PlutusV3Script(plutus()),
                compute_plutus_v3_script_hash(&plutus()),
            ),
        ] {
            assert_eq!(
                reference_script_outcomes(script, hash),
                every_purpose("Err(PostAlonzo(RedeemerMissing)) Ok(())"),
                "PlutusV{version}",
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
    fn minted_policy_needs_a_script() {
        let hash = compute_plutus_v3_script_hash(&plutus());
        synthetic(
            vec![],
            |tx| {
                let assets = vec![(
                    Bytes::from(vec![1]),
                    pallas_codec::utils::NonZeroInt::try_from(1).unwrap(),
                )];
                tx.transaction_body.mint =
                    Some([(hash, assets.into_iter().collect())].into_iter().collect());
            },
            &[],
            |tx| {
                assert!(matches!(
                    check_minting(&tx.transaction_body, &tx, &UTxOs::new()),
                    Err(PostAlonzo(MintingLacksPolicy(policy))) if policy == hash
                ))
            },
        );
    }

    #[test]
    fn native_reference_authorization_uses_original_script_hash() {
        // Valid noncanonical CBOR for [0, key_hash]: integer 0 uses two bytes.
        // A reference script is hashed from its original bytes.
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
                vec![Certificate::Reg(
                    StakeCredential::ScriptHash(hash),
                    2_000_000,
                )],
                |tx| {
                    tx.transaction_body.reference_inputs =
                        Some(vec![input.clone()].try_into().unwrap());
                },
                keys,
                |tx| {
                    let output = TransactionOutput::PostAlonzo(
                        pallas_primitives::conway::PostAlonzoTransactionOutput {
                            address: vec![0x61; 29].into(),
                            value: Value::Coin(2_000_000),
                            datum_option: None,
                            script_ref: Some(pallas_codec::utils::CborWrap(
                                ScriptRef::NativeScript(script.clone()),
                            )),
                        }
                        .into(),
                    );
                    let utxos = UTxOs::from([(
                        MultiEraInput::from_alonzo_compatible(&input),
                        MultiEraOutput::from_conway(&output),
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

        let result = check_collaterals_address(&[input()], &utxos(Era::Babbage, &raw));

        assert!(
            matches!(result, Err(PostAlonzo(CollateralNotVKeyLocked))),
            "{result:?}"
        );
    }

    #[test]
    fn key_and_byron_collateral_is_key_locked_from_alonzo_to_conway() {
        for address in [key_address(), byron_address()] {
            let raw = legacy_output(address);
            for era in [Era::Alonzo, Era::Babbage, Era::Conway] {
                let result = check_collaterals_address(&[input()], &utxos(era, &raw));

                assert!(result.is_ok(), "{era:?} {result:?}");
            }
        }
    }

    #[test]
    fn collateral_at_a_reward_or_undecodable_address_is_refused() {
        for address in [reward_address(), vec![0x61]] {
            let raw = legacy_output(address);

            let result = check_collaterals_address(&[input()], &utxos(Era::Babbage, &raw));

            assert!(
                matches!(result, Err(PostAlonzo(InputDecoding))),
                "{result:?}"
            );
        }
    }

    #[test]
    fn a_script_address_from_an_earlier_era_names_its_script() {
        for (address, expected) in [
            (script_address(), Some(Hash::from([2; 28]))),
            (key_address(), None),
        ] {
            let raw = legacy_output(address);
            for era in [Era::Alonzo, Era::Babbage, Era::Conway] {
                let actual = get_script_hash_from_input(&input(), &utxos(era, &raw));

                assert_eq!(actual, expected, "{era:?}");
            }
        }
    }

    #[test]
    fn a_reference_script_from_an_earlier_era_is_found() {
        let cbor = tx_spending_input();
        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).unwrap();
        let body = &tx.as_conway().unwrap().transaction_body;
        let expected = Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);

        let with_script = output_with_reference_script();
        let without_script = legacy_output(key_address());
        for era in [Era::Babbage, Era::Conway] {
            assert_eq!(
                get_reference_script_hashes(body, &utxos(era, &with_script)),
                vec![expected],
                "{era:?}"
            );
            assert!(
                get_reference_script_hashes(body, &utxos(era, &without_script)).is_empty(),
                "{era:?}"
            );
        }
    }

    #[test]
    fn a_datum_hash_on_an_earlier_era_reference_input_is_found() {
        let cbor = tx_referencing_input();
        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).unwrap();
        let body = &tx.as_conway().unwrap().transaction_body;
        let hash = Hash::from([3; 32]);
        let raw = minicbor::to_vec(pallas_primitives::alonzo::TransactionOutput {
            address: key_address().into(),
            amount: babbage::Value::Coin(5_000_000),
            datum_hash: Some(hash),
        })
        .unwrap();

        for era in [Era::Alonzo, Era::Babbage, Era::Conway] {
            let utxos = utxos(era, &raw);

            let found = find_datum(&hash, body, &utxos);
            let other = find_datum(&Hash::from([4; 32]), body, &utxos);

            assert!(found.is_ok(), "{era:?} {found:?}");
            assert!(
                matches!(other, Err(PostAlonzo(UnneededDatum))),
                "{era:?} {other:?}"
            );
        }
    }

    #[test]
    fn a_reference_script_on_an_input_or_an_output_rules_out_plutus_v1() {
        let cbor = tx_spending_input();
        let with_script = output_with_reference_script();
        let without_script = legacy_output(key_address());
        let (mut outcomes, mut expected) = (vec![], vec![]);
        for input_script in [false, true] {
            for output_script in [false, true] {
                let mut tx: Tx = minicbor::decode(&cbor).unwrap();
                let mut body = (*tx.transaction_body).clone();
                if output_script {
                    body.outputs = vec![minicbor::decode(&with_script).unwrap()];
                }
                tx.transaction_body = body.into();
                let spent = if input_script {
                    utxos(Era::Babbage, &with_script)
                } else {
                    utxos(Era::Alonzo, &without_script)
                };
                let languages = allowed_tx_langs(&tx, &spent);
                outcomes.push((input_script, output_script, languages));
                let languages = if input_script || output_script {
                    vec![Language::PlutusV2, Language::PlutusV3]
                } else {
                    vec![Language::PlutusV1, Language::PlutusV2, Language::PlutusV3]
                };
                expected.push((input_script, output_script, languages));
            }
        }
        assert_eq!(outcomes, expected);
    }

    #[test]
    fn a_byron_era_input_allows_no_plutus_language() {
        let cbor = tx_spending_input();
        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).unwrap();
        let tx = tx.as_conway().unwrap();

        let byron = byron_output();
        let key = legacy_output(key_address());

        assert_eq!(allowed_tx_langs(tx, &utxos(Era::Byron, &byron)), vec![]);
        assert_eq!(
            allowed_tx_langs(tx, &utxos(Era::Alonzo, &key)),
            vec![Language::PlutusV1, Language::PlutusV2, Language::PlutusV3]
        );
    }

    #[test]
    fn only_a_needed_reference_script_sets_the_language() {
        use pallas_codec::utils::NonZeroInt;
        let policy = Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
        let with_script = output_with_reference_script();
        let without_script = legacy_output(key_address());
        let (mut outcomes, mut expected) = (vec![], vec![]);
        for (cbor, spent) in [(tx_spending_input(), true), (tx_referencing_input(), false)] {
            let mut tx: Tx = minicbor::decode(&cbor).unwrap();
            for mints in [false, true] {
                let mut body = (*tx.transaction_body).clone();
                body.mint = mints.then(|| {
                    let asset = (Bytes::from(vec![1]), NonZeroInt::try_from(1).unwrap());
                    [(policy, [asset].into_iter().collect())]
                        .into_iter()
                        .collect()
                });
                tx.transaction_body = body.into();
                for (output, has_script) in [(&with_script, true), (&without_script, false)] {
                    for era in [Era::Babbage, Era::Conway] {
                        let languages = tx_languages(&tx, &utxos(era, output));
                        outcomes.push((spent, mints, has_script, era, languages));
                        let needed = mints && has_script;
                        let languages = if needed {
                            vec![Language::PlutusV2]
                        } else {
                            vec![]
                        };
                        expected.push((spent, mints, has_script, era, languages));
                    }
                }
            }
        }
        assert_eq!(outcomes, expected);
    }

    #[test]
    fn each_needed_purpose_sets_the_reference_script_language() {
        use pallas_codec::utils::NonZeroInt;
        use pallas_primitives::conway::{
            Anchor, GovAction, GovActionId, ProposalProcedure, Vote, Voter, VotingProcedure,
        };
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
            ("cert", true),
            ("reward", true),
            ("drep vote", true),
            ("committee vote", true),
            ("treasury proposal", true),
            ("parameter proposal", true),
            ("key reward", false),
        ] {
            for hash in [script, Hash::from([5; 28])] {
                let locked_output = legacy_output([vec![0x71], hash.to_vec()].concat());
                let utxos = UTxOs::from([
                    (
                        MultiEraInput::from_alonzo_compatible(&holder),
                        MultiEraOutput::decode(Era::Conway, &with_script).unwrap(),
                    ),
                    (
                        MultiEraInput::from_alonzo_compatible(&locked),
                        MultiEraOutput::decode(Era::Alonzo, &locked_output).unwrap(),
                    ),
                ]);
                let mut tx: Tx = minicbor::decode(&cbor).unwrap();
                let mut body = (*tx.transaction_body).clone();
                match purpose {
                    "spend" => body.inputs = vec![locked.clone()].into(),
                    "mint" => {
                        let asset = (Bytes::from(vec![1]), NonZeroInt::try_from(1).unwrap());
                        body.mint = Some(
                            [(hash, [asset].into_iter().collect())]
                                .into_iter()
                                .collect(),
                        )
                    }
                    "cert" => {
                        body.certificates = Some(
                            vec![Certificate::Reg(StakeCredential::ScriptHash(hash), 0)]
                                .try_into()
                                .unwrap(),
                        )
                    }
                    "reward" | "key reward" => {
                        let header = if names_script { 0xf0 } else { 0xe0 };
                        let account = [vec![header], hash.to_vec()].concat();
                        body.withdrawals = Some([(account.into(), 0)].into_iter().collect())
                    }
                    "drep vote" | "committee vote" => {
                        let voter = if purpose == "drep vote" {
                            Voter::DRepScript(hash)
                        } else {
                            Voter::ConstitutionalCommitteeScript(hash)
                        };
                        let action = GovActionId {
                            transaction_id: Hash::new([0; 32]),
                            action_index: 0,
                        };
                        let vote = VotingProcedure {
                            vote: Vote::Yes,
                            anchor: None,
                        };
                        body.voting_procedures = Some(
                            [(voter, [(action, vote)].into_iter().collect())]
                                .into_iter()
                                .collect(),
                        )
                    }
                    _ => {
                        let gov_action = if purpose == "treasury proposal" {
                            GovAction::TreasuryWithdrawals(Default::default(), Some(hash))
                        } else {
                            let update = minicbor::decode(&[0xa0]).unwrap();
                            GovAction::ParameterChange(None, Box::new(update), Some(hash))
                        };
                        let proposal = ProposalProcedure {
                            deposit: 0,
                            reward_account: [vec![0xe0], vec![0; 28]].concat().into(),
                            gov_action,
                            anchor: Anchor {
                                url: String::new(),
                                content_hash: Hash::new([0; 32]),
                            },
                        };
                        body.proposal_procedures = Some(vec![proposal].try_into().unwrap())
                    }
                }
                tx.transaction_body = body.into();
                let needed = names_script && hash == script;
                outcomes.push((purpose, needed, tx_languages(&tx, &utxos)));
                let languages = if needed {
                    vec![Language::PlutusV2]
                } else {
                    vec![]
                };
                expected.push((purpose, needed, languages));
            }
        }
        assert_eq!(outcomes.len(), 18);
        assert_eq!(outcomes, expected);
    }

    #[test]
    fn an_input_at_a_reward_or_undecodable_byron_address_is_refused() {
        let cbor = tx_spending_input();
        let tx: Tx = minicbor::decode(&cbor).unwrap();
        let outcomes: Vec<_> = [
            key_address(),
            byron_address(),
            reward_address(),
            undecodable_byron_address(),
        ]
        .map(|address| {
            let raw = legacy_output(address);
            format!(
                "{:?}",
                check_vkey_input_wits(&tx, &Some(vec![]), &utxos(Era::Alonzo, &raw))
            )
        })
        .into();
        assert_eq!(
            outcomes,
            [
                "Err(PostAlonzo(VKWitnessMissing))",
                "Err(PostAlonzo(VKWitnessMissing))",
                "Err(PostAlonzo(InputDecoding))",
                "Err(PostAlonzo(InputDecoding))",
            ]
        );
    }
}
