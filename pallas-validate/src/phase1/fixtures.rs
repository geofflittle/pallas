//! Addresses, outputs and UTxO sets for the era unit tests.

use crate::utils::UTxOs;
use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{TransactionInput, alonzo};
use pallas_traverse::{Era, MultiEraInput, MultiEraOutput};

pub(super) fn input() -> TransactionInput {
    TransactionInput {
        transaction_id: Hash::new([0; 32]),
        index: 0,
    }
}

pub(super) fn legacy_output(address: Vec<u8>) -> Vec<u8> {
    minicbor::to_vec(alonzo::TransactionOutput {
        address: address.into(),
        amount: alonzo::Value::Coin(5_000_000),
        datum_hash: None,
    })
    .unwrap()
}

pub(super) fn utxos(era: Era, output: &[u8]) -> UTxOs<'_> {
    UTxOs::from([(
        MultiEraInput::AlonzoCompatible(Box::new(std::borrow::Cow::Owned(input()))),
        MultiEraOutput::decode(era, output).unwrap(),
    )])
}

pub(super) fn key_address() -> Vec<u8> {
    [vec![0x61], vec![1; 28]].concat()
}

pub(super) fn script_address() -> Vec<u8> {
    [vec![0x71], vec![2; 28]].concat()
}

pub(super) fn byron_address() -> Vec<u8> {
    pallas_addresses::ByronAddress::from_base58(
        "Ae2tdPwUPEZLs4HtbuNey7tK4hTKrwNwYtGqp7bDfCy2WdR3P6735W5Yfpe",
    )
    .unwrap()
    .to_vec()
}

pub(super) fn undecodable_byron_address() -> Vec<u8> {
    pallas_addresses::ByronAddress::new(&[0x00], 0).to_vec()
}

pub(super) fn reward_address() -> Vec<u8> {
    [vec![0xe0], vec![3; 28]].concat()
}

pub(super) const V2_SCRIPT: [u8; 4] = [1, 2, 3, 4];

pub(super) fn output_with_reference_script() -> Vec<u8> {
    let mut cbor = vec![0xa3, 0x00, 0x58, 0x1d]; // output, a 29 byte address
    cbor.extend(key_address());
    cbor.extend([0x01, 0x1a, 0x00, 0x4c, 0x4b, 0x40]); // five ada
    cbor.extend([0x03, 0xd8, 0x18, 0x47, 0x82, 0x02, 0x44]); // a PlutusV2 reference script
    cbor.extend(V2_SCRIPT);
    cbor
}

pub(super) fn byron_output() -> Vec<u8> {
    let mut cbor = vec![0x82];
    cbor.extend(byron_address());
    cbor.extend([0x1a, 0x00, 0x4c, 0x4b, 0x40]);
    cbor
}

fn input_cbor() -> Vec<u8> {
    let mut cbor = vec![0x82, 0x58, 0x20];
    cbor.extend([0; 32]);
    cbor.push(0x00);
    cbor
}

pub(super) fn tx_spending_input() -> Vec<u8> {
    let mut cbor = vec![0x84, 0xa3, 0x00, 0x81]; // transaction, body, one input
    cbor.extend(input_cbor());
    cbor.extend([0x01, 0x80, 0x02, 0x00]); // no outputs, no fee
    cbor.extend([0xa0, 0xf5, 0xf6]); // no witnesses, valid, no auxiliary data
    cbor
}

pub(super) fn tx_referencing_input() -> Vec<u8> {
    let mut cbor = vec![0x84, 0xa4]; // transaction, body
    cbor.extend([0x00, 0x80, 0x01, 0x80, 0x02, 0x00]); // no inputs, no outputs, no fee
    cbor.extend([0x12, 0x81]); // one reference input
    cbor.extend(input_cbor());
    cbor.extend([0xa0, 0xf5, 0xf6]); // no witnesses, valid, no auxiliary data
    cbor
}
