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

pub(super) fn reward_address() -> Vec<u8> {
    [vec![0xe0], vec![3; 28]].concat()
}
