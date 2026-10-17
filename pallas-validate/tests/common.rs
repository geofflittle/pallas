use pallas_addresses::byron::{AddrType, AddressPayload, ByronAddress, SpendingData};
use pallas_codec::{
    minicbor::{self, bytes::ByteVec},
    utils::TagWrap,
};
use pallas_crypto::key::ed25519::SecretKey;
use pallas_primitives::{
    alonzo::{BootstrapWitness, TransactionBody, TransactionOutput, Tx as AlonzoTx, Value},
    babbage::Tx as BabbageTx,
    byron::{Address, Tx, TxOut, TxPayload},
    conway::Tx as ConwayTx,
};
use pallas_traverse::{Era, MultiEraInput, MultiEraOutput};
use pallas_validate::utils::UTxOs;
use pallas_validate::utils::{EraCbor, TxoRef, UtxoMap, ValidationError, ValidationResult};
use std::{borrow::Cow, iter::zip, vec::Vec};

use pallas_codec::utils::{Bytes, CborWrap};
use pallas_crypto::hash::Hash;

pub const BYRON_CHAIN_CODE: [u8; 32] = [9; 32];

// Type aliases to reduce complexity
pub type BabbageTxOutInfo<'a> = (
    String, // address in string format
    Value,
    Option<pallas_primitives::babbage::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::babbage::ScriptRef<'a>>>,
);

pub type ConwayTxOutInfo<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_primitives::conway::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
);

pub type ConwayTxOutInfoMut<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_codec::utils::KeepRaw<'a, pallas_primitives::conway::DatumOption<'a>>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
    Vec<u8>, // Placeholder for CBOR data.
);

pub type AlonzoCollateralInfo = (
    String, // address in string format
    Value,
    Option<Hash<32>>,
);

pub type BabbageCollateralInfo<'a> = (
    String, // address in string format
    Value,
    Option<pallas_primitives::babbage::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::babbage::ScriptRef<'a>>>,
);

pub type ConwayCollateralInfo<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_primitives::conway::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
);

pub type ConwayCollateralInfoMut<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_codec::utils::KeepRaw<'a, pallas_primitives::conway::DatumOption<'a>>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
    Vec<u8>, // Placeholder for CBOR data.
);

pub type BabbageRefInputInfo<'a> = (
    String, // address in string format
    Value,
    Option<pallas_primitives::babbage::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::babbage::ScriptRef<'a>>>,
);

pub type ConwayRefInputInfo<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_primitives::conway::DatumOption<'a>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
);

pub type ConwayRefInputInfoMut<'a> = (
    String, // address in string format
    pallas_primitives::conway::Value,
    Option<pallas_codec::utils::KeepRaw<'a, pallas_primitives::conway::DatumOption<'a>>>,
    Option<CborWrap<pallas_primitives::conway::ScriptRef<'a>>>,
    Vec<u8>, // Placeholder for CBOR data.
);

pub fn cbor_to_bytes(input: &str) -> Vec<u8> {
    hex::decode(input).unwrap()
}

pub fn minted_tx_from_cbor(tx_cbor: &[u8]) -> AlonzoTx<'_> {
    pallas_codec::minicbor::decode::<AlonzoTx>(tx_cbor).unwrap()
}

pub fn babbage_minted_tx_from_cbor(tx_cbor: &[u8]) -> BabbageTx<'_> {
    pallas_codec::minicbor::decode::<BabbageTx>(tx_cbor).unwrap()
}

pub fn conway_minted_tx_from_cbor(tx_cbor: &[u8]) -> ConwayTx<'_> {
    pallas_codec::minicbor::decode::<ConwayTx>(tx_cbor).unwrap()
}

pub fn minted_tx_payload_from_cbor(tx_cbor: &[u8]) -> TxPayload<'_> {
    pallas_codec::minicbor::decode::<TxPayload>(tx_cbor).unwrap()
}

pub fn mk_utxo_for_byron_tx<'a>(tx: &Tx, tx_outs_info: &[(String, u64)]) -> UTxOs<'a> {
    let mut utxos: UTxOs = UTxOs::new();
    for (tx_in, (address_payload, amount)) in zip(tx.inputs.clone().to_vec(), tx_outs_info) {
        let input_tx_out_addr: Address = match hex::decode(address_payload) {
            Ok(addr_bytes) => Address {
                payload: TagWrap(ByteVec::from(addr_bytes)),
                crc: 3430631884,
            },
            _ => panic!("Unable to decode input address"),
        };
        let tx_out: TxOut = TxOut {
            address: input_tx_out_addr,
            amount: *amount,
        };
        let multi_era_in: MultiEraInput = MultiEraInput::Byron(Box::new(Cow::Owned(tx_in)));
        let multi_era_out: MultiEraOutput = MultiEraOutput::Byron(Box::new(Cow::Owned(tx_out)));
        utxos.insert(multi_era_in, multi_era_out);
    }
    utxos
}

pub fn mk_utxo_for_alonzo_compatible_tx<'a>(
    tx_body: &TransactionBody,
    tx_outs_info: &[(
        String, // address in string format
        Value,
        Option<Hash<32>>,
    )],
) -> UTxOs<'a> {
    let mut utxos: UTxOs = UTxOs::new();
    for (tx_in, (address, amount, datum_hash)) in zip(tx_body.inputs.clone(), tx_outs_info) {
        let multi_era_in: MultiEraInput =
            MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
        let address_bytes: Bytes = match hex::decode(address) {
            Ok(bytes_vec) => Bytes::from(bytes_vec),
            _ => panic!("Unable to decode input address"),
        };
        let tx_out: TransactionOutput = TransactionOutput {
            address: address_bytes,
            amount: amount.clone(),
            datum_hash: *datum_hash,
        };
        let multi_era_out: MultiEraOutput =
            MultiEraOutput::AlonzoCompatible(Box::new(Cow::Owned(tx_out)), Era::Alonzo);
        utxos.insert(multi_era_in, multi_era_out);
    }
    utxos
}

pub fn mk_utxo_for_babbage_tx<'a>(
    tx_body: &pallas_primitives::babbage::TransactionBody,
    tx_outs_info: &'a [BabbageTxOutInfo<'a>],
) -> UTxOs<'a> {
    let mut utxos: UTxOs = UTxOs::new();
    for (tx_in, (addr, val, datum_opt, script_ref)) in zip(tx_body.inputs.clone(), tx_outs_info) {
        let multi_era_in: MultiEraInput =
            MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
        let address_bytes: Bytes = match hex::decode(addr) {
            Ok(bytes_vec) => Bytes::from(bytes_vec),
            _ => panic!("Unable to decode input address"),
        };
        let tx_out: pallas_primitives::babbage::TransactionOutput =
            pallas_primitives::babbage::TransactionOutput::PostAlonzo(
                pallas_primitives::babbage::PostAlonzoTransactionOutput {
                    address: address_bytes,
                    value: val.clone(),
                    datum_option: datum_opt.clone().map(|x| x.into()),
                    script_ref: script_ref.clone(),
                }
                .into(),
            );
        let multi_era_out: MultiEraOutput = MultiEraOutput::Babbage(Box::new(Cow::Owned(tx_out)));
        utxos.insert(multi_era_in, multi_era_out);
    }
    utxos
}

pub fn mk_utxo_for_conway_tx<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    tx_outs_info: &'a [ConwayTxOutInfo<'a>],
) -> UTxOs<'a> {
    let mut utxos: UTxOs = UTxOs::new();

    for (tx_in, (addr, val, datum_opt, script_ref)) in
        zip(tx_body.inputs.clone().to_vec(), tx_outs_info)
    {
        let multi_era_in: MultiEraInput =
            MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
        let address_bytes: Bytes = match hex::decode(addr) {
            Ok(bytes_vec) => Bytes::from(bytes_vec),
            _ => panic!("Unable to decode input address"),
        };
        let tx_out: pallas_primitives::conway::TransactionOutput =
            pallas_primitives::conway::TransactionOutput::PostAlonzo(
                pallas_primitives::conway::PostAlonzoTransactionOutput {
                    address: address_bytes,
                    value: val.clone(),
                    datum_option: datum_opt.clone().map(|x| x.into()),
                    script_ref: script_ref.clone(),
                }
                .into(),
            );
        let multi_era_out: MultiEraOutput = MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
        utxos.insert(multi_era_in, multi_era_out);
    }
    utxos
}

pub fn mk_codec_safe_utxo_for_conway_tx<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    tx_outs_info: &'a mut Vec<ConwayTxOutInfoMut<'a>>,
) -> UTxOs<'a> {
    let mut utxos: UTxOs = UTxOs::new();

    for (tx_in, (addr, val, datum_opt, script_ref, cbor)) in
        zip(tx_body.inputs.clone().to_vec(), tx_outs_info)
    {
        let multi_era_in: MultiEraInput =
            MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
        let address_bytes: Bytes = match hex::decode(addr) {
            Ok(bytes_vec) => Bytes::from(bytes_vec),
            _ => panic!("Unable to decode input address"),
        };
        let post_alonzo = pallas_primitives::conway::PostAlonzoTransactionOutput {
            address: address_bytes,
            value: val.clone(),
            datum_option: datum_opt.clone(),
            script_ref: script_ref.clone(),
        };
        *cbor = minicbor::to_vec(post_alonzo).unwrap();
        let post_alonzo = minicbor::decode::<
            pallas_codec::utils::KeepRaw<
                'a,
                pallas_primitives::conway::PostAlonzoTransactionOutput,
            >,
        >(cbor)
        .unwrap();
        let tx_out = pallas_primitives::conway::TransactionOutput::PostAlonzo(post_alonzo);
        let multi_era_out: MultiEraOutput = MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
        utxos.insert(multi_era_in, multi_era_out);
    }
    utxos
}

pub fn mk_utxo_for_eval(utxos: UTxOs) -> UtxoMap {
    let mut eval_utxos: UtxoMap = UtxoMap::new();

    for (tx_in, tx_out) in utxos {
        eval_utxos.insert(TxoRef::from(&tx_in), EraCbor::from(tx_out));
    }
    eval_utxos
}

pub fn add_collateral_alonzo(
    tx_body: &TransactionBody,
    utxos: &mut UTxOs<'_>,
    collateral_info: &[AlonzoCollateralInfo],
) {
    match &tx_body.collateral {
        Some(collaterals) => {
            for (tx_in, (address, amount, datum_hash)) in zip(collaterals, collateral_info) {
                let address_bytes: Bytes = match hex::decode(address) {
                    Ok(bytes_vec) => Bytes::from(bytes_vec),
                    _ => panic!("Unable to decode input address"),
                };
                let tx_out: TransactionOutput = TransactionOutput {
                    address: address_bytes,
                    amount: amount.clone(),
                    datum_hash: *datum_hash,
                };
                let multi_era_in: MultiEraInput =
                    MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in.clone())));
                let multi_era_out: MultiEraOutput =
                    MultiEraOutput::AlonzoCompatible(Box::new(Cow::Owned(tx_out)), Era::Alonzo);
                utxos.insert(multi_era_in, multi_era_out);
            }
        }
        None => panic!("Adding collateral to UTxO failed due to an empty list of collaterals"),
    }
}

pub fn add_collateral_babbage<'a>(
    tx_body: &pallas_primitives::babbage::TransactionBody,
    utxos: &mut UTxOs<'a>,
    collateral_info: &'a [BabbageCollateralInfo<'a>],
) {
    match &tx_body.collateral {
        Some(collaterals) => {
            if collaterals.is_empty() {
                panic!("UTxO addition error - collateral input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref)) in
                    zip(collaterals.clone(), collateral_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let tx_out: pallas_primitives::babbage::TransactionOutput =
                        pallas_primitives::babbage::TransactionOutput::PostAlonzo(
                            pallas_primitives::babbage::PostAlonzoTransactionOutput {
                                address: address_bytes,
                                value: val.clone(),
                                datum_option: datum_opt.clone().map(|x| x.into()),
                                script_ref: script_ref.clone(),
                            }
                            .into(),
                        );
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Babbage(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - collateral input missing"),
    }
}

pub fn add_collateral_conway<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    utxos: &mut UTxOs<'a>,
    collateral_info: &'a [ConwayCollateralInfo<'a>],
) {
    match &tx_body.collateral {
        Some(collaterals) => {
            if collaterals.is_empty() {
                panic!("UTxO addition error - collateral input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref)) in
                    zip(collaterals.clone().to_vec(), collateral_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let tx_out: pallas_primitives::conway::TransactionOutput =
                        pallas_primitives::conway::TransactionOutput::PostAlonzo(
                            pallas_primitives::conway::PostAlonzoTransactionOutput {
                                address: address_bytes,
                                value: val.clone(),
                                datum_option: datum_opt.clone().map(|x| x.into()),
                                script_ref: script_ref.clone(),
                            }
                            .into(),
                        );
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - collateral input missing"),
    }
}

pub fn add_codec_safe_collateral_conway<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    utxos: &mut UTxOs<'a>,
    collateral_info: &'a mut Vec<ConwayCollateralInfoMut<'a>>,
) {
    match &tx_body.collateral {
        Some(collaterals) => {
            if collaterals.is_empty() {
                panic!("UTxO addition error - collateral input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref, cbor)) in
                    zip(collaterals.clone().to_vec(), collateral_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let post_alonzo = pallas_primitives::conway::PostAlonzoTransactionOutput {
                        address: address_bytes,
                        value: val.clone(),
                        datum_option: datum_opt.clone(),
                        script_ref: script_ref.clone(),
                    };
                    *cbor = minicbor::to_vec(post_alonzo).unwrap();
                    let post_alonzo = minicbor::decode::<
                        pallas_codec::utils::KeepRaw<
                            'a,
                            pallas_primitives::conway::PostAlonzoTransactionOutput,
                        >,
                    >(cbor)
                    .unwrap();
                    let tx_out =
                        pallas_primitives::conway::TransactionOutput::PostAlonzo(post_alonzo);
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - collateral input missing"),
    }
}

pub fn add_ref_input_babbage<'a>(
    tx_body: &pallas_primitives::babbage::TransactionBody,
    utxos: &mut UTxOs<'a>,
    ref_input_info: &'a [BabbageRefInputInfo<'a>],
) {
    match &tx_body.reference_inputs {
        Some(ref_inputs) => {
            if ref_inputs.is_empty() {
                panic!("UTxO addition error - reference input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref)) in
                    zip(ref_inputs.clone(), ref_input_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let tx_out: pallas_primitives::babbage::TransactionOutput =
                        pallas_primitives::babbage::TransactionOutput::PostAlonzo(
                            pallas_primitives::babbage::PostAlonzoTransactionOutput {
                                address: address_bytes,
                                value: val.clone(),
                                datum_option: datum_opt.clone().map(|x| x.into()),
                                script_ref: script_ref.clone(),
                            }
                            .into(),
                        );
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Babbage(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - reference input missing"),
    }
}

pub fn add_ref_input_conway<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    utxos: &mut UTxOs<'a>,
    ref_input_info: &'a [ConwayRefInputInfo<'a>],
) {
    match &tx_body.reference_inputs {
        Some(ref_inputs) => {
            if ref_inputs.is_empty() {
                panic!("UTxO addition error - reference input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref)) in
                    zip(ref_inputs.clone().to_vec(), ref_input_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let tx_out: pallas_primitives::conway::TransactionOutput =
                        pallas_primitives::conway::TransactionOutput::PostAlonzo(
                            pallas_primitives::conway::PostAlonzoTransactionOutput {
                                address: address_bytes,
                                value: val.clone(),
                                datum_option: datum_opt.clone().map(|x| x.into()),
                                script_ref: script_ref.clone(),
                            }
                            .into(),
                        );
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - reference input missing"),
    }
}

pub fn add_codec_safe_ref_input_conway<'a>(
    tx_body: &pallas_primitives::conway::TransactionBody,
    utxos: &mut UTxOs<'a>,
    ref_input_info: &'a mut Vec<ConwayRefInputInfoMut<'a>>,
) {
    match &tx_body.reference_inputs {
        Some(ref_inputs) => {
            if ref_inputs.is_empty() {
                panic!("UTxO addition error - reference input missing")
            } else {
                for (tx_in, (addr, val, datum_opt, script_ref, cbor)) in
                    zip(ref_inputs.clone().to_vec(), ref_input_info)
                {
                    let multi_era_in: MultiEraInput =
                        MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(tx_in)));
                    let address_bytes: Bytes = match hex::decode(addr) {
                        Ok(bytes_vec) => Bytes::from(bytes_vec),
                        _ => panic!("Unable to decode input address"),
                    };
                    let post_alonzo = pallas_primitives::conway::PostAlonzoTransactionOutput {
                        address: address_bytes,
                        value: val.clone(),
                        datum_option: datum_opt.clone(),
                        script_ref: script_ref.clone(),
                    };
                    *cbor = minicbor::to_vec(post_alonzo).unwrap();
                    let post_alonzo = minicbor::decode::<
                        pallas_codec::utils::KeepRaw<
                            'a,
                            pallas_primitives::conway::PostAlonzoTransactionOutput,
                        >,
                    >(cbor)
                    .unwrap();
                    let tx_out =
                        pallas_primitives::conway::TransactionOutput::PostAlonzo(post_alonzo);
                    let multi_era_out: MultiEraOutput =
                        MultiEraOutput::Conway(Box::new(Cow::Owned(tx_out)));
                    utxos.insert(multi_era_in, multi_era_out);
                }
            }
        }
        None => panic!("UTxO addition error - reference input missing"),
    }
}

pub fn byron_key() -> SecretKey {
    SecretKey::from([7; 32])
}

pub fn byron_key_address() -> Vec<u8> {
    let xpub = [byron_key().public_key().as_ref(), &BYRON_CHAIN_CODE[..]].concat();
    ByronAddress::from(AddressPayload::new(
        AddrType::PubKey,
        SpendingData::PubKey(xpub.into()),
        vec![].into(),
    ))
    .to_vec()
}

pub fn byron_era_output(address: &[u8], amount: u64) -> Vec<u8> {
    let mut cbor = vec![0x82];
    cbor.extend(address);
    cbor.extend(minicbor::to_vec(amount).unwrap());
    cbor
}

pub fn bootstrap_witness(key: &SecretKey, attributes: &[u8], signed: &[u8]) -> BootstrapWitness {
    BootstrapWitness {
        public_key: key.public_key().as_ref().to_vec().into(),
        signature: key.sign(signed).as_ref().to_vec().into(),
        chain_code: BYRON_CHAIN_CODE.to_vec().into(),
        attributes: attributes.to_vec().into(),
    }
}

pub fn bootstrap_witness_cases(
    signed: &[u8],
    missing: ValidationError,
    wrong_signature: ValidationError,
) -> Vec<(Vec<BootstrapWitness>, ValidationResult)> {
    let key = byron_key();
    let other_key = SecretKey::from([8; 32]);
    let witness = bootstrap_witness(&key, &[0xa0], signed);
    let other = |signed: &[u8]| bootstrap_witness(&other_key, &[0xa0], signed);
    let mut short_key = other(signed);
    short_key.public_key = vec![1; 31].into();
    let mut short_signature = witness.clone();
    short_signature.signature = vec![1; 63].into();
    vec![
        (vec![witness.clone()], Ok(())),
        (vec![], Err(missing.clone())),
        (
            vec![bootstrap_witness(&key, &[0xa0], &[0; 32])],
            Err(wrong_signature.clone()),
        ),
        (vec![other(signed)], Err(missing.clone())),
        (vec![witness.clone(), other(signed)], Ok(())),
        (
            vec![witness.clone(), other(&[0; 32])],
            Err(wrong_signature.clone()),
        ),
        // An empty attribute map written with indefinite length.
        (
            vec![bootstrap_witness(&key, &[0xbf, 0xff], signed)],
            Err(missing),
        ),
        (vec![witness, short_key], Err(wrong_signature.clone())),
        (vec![short_signature], Err(wrong_signature)),
    ]
}
