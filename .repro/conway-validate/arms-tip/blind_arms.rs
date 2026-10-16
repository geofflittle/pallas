//! Opponent arms against 6b71fdeb. Each prints ARM <name> <result> so a run can be grepped.
use super::*;
use pallas_codec::minicbor;
use pallas_crypto::{hash::Hasher, key::ed25519::SecretKey};
use pallas_primitives::conway::Certificate;
use pallas_traverse::Era;

fn report(name: &str, actual: &ValidationResult) {
    println!("ARM {name} {actual:?}");
}

fn input(id: u8) -> TransactionInput {
    TransactionInput {
        transaction_id: Hash::new([id; 32]),
        index: 0,
    }
}

fn key_hash(id: u8) -> AddrKeyhash {
    Hasher::<224>::hash(SecretKey::from([id; 32]).public_key().as_ref())
}

// ScriptPubkey over key 1, as CBOR.
fn native_cbor() -> Vec<u8> {
    let mut v = vec![0x82, 0x00, 0x58, 0x1c];
    v.extend(key_hash(1).as_ref());
    v
}

fn native_hash() -> Hash<28> {
    Hasher::<224>::hash_tagged(&native_cbor(), 0)
}

// A Conway map output at a key address, five ada, carrying the native script by reference.
fn output_with_native_ref() -> Vec<u8> {
    let mut cbor = vec![0xa3, 0x00, 0x58, 0x1d, 0x61];
    cbor.extend([3u8; 28]);
    cbor.extend([0x01, 0x1a, 0x00, 0x4c, 0x4b, 0x40]);
    let inner = [vec![0x82, 0x00], native_cbor()].concat();
    cbor.extend([0x03, 0xd8, 0x18, 0x58, inner.len() as u8]);
    cbor.extend(inner);
    cbor
}

fn coin_output(coin: u64) -> TransactionOutput<'static> {
    let raw = minicbor::to_vec(pallas_primitives::alonzo::TransactionOutput {
        address: [vec![0x61], vec![4; 28]].concat().into(),
        amount: babbage::Value::Coin(coin),
        datum_hash: None,
    })
    .unwrap();
    let leaked: &'static [u8] = Box::leak(raw.into_boxed_slice());
    minicbor::decode(leaked).unwrap()
}

fn base_tx(prepare: impl FnOnce(&mut Tx<'_>), keys: &[u8], test: impl FnOnce(Tx<'_>)) {
    let raw = hex::decode(include_str!("../../../../test_data/conway3.tx").trim()).unwrap();
    let mut tx: Tx = minicbor::decode(&raw).unwrap();
    tx.transaction_body.inputs = Vec::new().into();
    tx.transaction_body.collateral = None;
    tx.transaction_body.reference_inputs = None;
    tx.transaction_body.required_signers = None;
    tx.transaction_body.withdrawals = None;
    tx.transaction_body.mint = None;
    tx.transaction_body.certificates = None;
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

// A1: a certificate whose script credential is a native script supplied by a
// reference input. The ledger needs no redeemer for a native script.
#[test]
fn arm_a1_native_reference_script_certificate() {
    let raw = output_with_native_ref();
    let utxos = UTxOs::from([(
        MultiEraInput::AlonzoCompatible(Box::new(std::borrow::Cow::Owned(input(5)))),
        MultiEraOutput::decode(Era::Conway, &raw).unwrap(),
    )]);
    let cert = Certificate::StakeDelegation(StakeCredential::ScriptHash(native_hash()), [9; 28].into());
    base_tx(
        |tx| {
            tx.transaction_body.certificates = Some(vec![cert.clone()].try_into().unwrap());
            tx.transaction_body.reference_inputs = Some(vec![input(5)].try_into().unwrap());
        },
        &[1],
        |tx| report("A1_native_ref_cert", &check_witness_set(&tx, &utxos)),
    );
    // Control: the same native script in the witness set.
    let script: KeepRaw<'_, pallas_primitives::alonzo::NativeScript> =
        minicbor::decode(Box::leak(native_cbor().into_boxed_slice())).unwrap();
    base_tx(
        |tx| {
            tx.transaction_body.certificates = Some(vec![cert.clone()].try_into().unwrap());
            tx.transaction_witness_set.native_script = Some(vec![script.clone()].try_into().unwrap());
        },
        &[1],
        |tx| report("A1c_native_witness_cert", &check_witness_set(&tx, &UTxOs::new())),
    );
    // Same native reference script spent as an input rather than referenced.
    base_tx(
        |tx| {
            tx.transaction_body.certificates = Some(vec![cert.clone()].try_into().unwrap());
            tx.transaction_body.inputs = vec![input(5)].into();
        },
        &[1],
        |tx| report("A1s_native_ref_on_spent_input_cert", &check_witness_set(&tx, &utxos)),
    );
}

fn balance_tx(
    input_coin: u64,
    certs: Vec<Certificate>,
    withdrawal: Option<u64>,
    name: &str,
) {
    let utxo_raw = minicbor::to_vec(pallas_primitives::alonzo::TransactionOutput {
        address: [vec![0x61], vec![4; 28]].concat().into(),
        amount: babbage::Value::Coin(input_coin),
        datum_hash: None,
    })
    .unwrap();
    let utxos = UTxOs::from([(
        MultiEraInput::AlonzoCompatible(Box::new(std::borrow::Cow::Owned(input(6)))),
        MultiEraOutput::decode(Era::Conway, &utxo_raw).unwrap(),
    )]);
    base_tx(
        |tx| {
            tx.transaction_body.inputs = vec![input(6)].into();
            tx.transaction_body.outputs = vec![coin_output(10_000_000)];
            tx.transaction_body.fee = 1_000_000;
            tx.transaction_body.certificates = certs.try_into().ok();
            if let Some(w) = withdrawal {
                let account: Bytes = [vec![0xe1], vec![4; 28]].concat().into();
                tx.transaction_body.withdrawals = Some([(account, w)].into_iter().collect());
            }
        },
        &[],
        |tx| {
            report(
                name,
                &check_preservation_of_value(&tx, &utxos, 2_000_000, 500_000_000, 0),
            )
        },
    );
}

// A2: balanced the ledger's way. consumed = inputs, refunds, withdrawals.
// produced = outputs, fee, deposits.
#[test]
fn arm_a2_balance_siblings() {
    let cred = || StakeCredential::AddrKeyhash([7; 28].into());
    balance_tx(13_000_000, vec![Certificate::Reg(cred(), 2_000_000)], None, "A2c_reg_deposit_control");
    balance_tx(9_000_000, vec![Certificate::UnReg(cred(), 2_000_000)], None, "A2u_unreg_refund");
    balance_tx(9_000_000, vec![Certificate::StakeDeregistration(cred())], None, "A2d_legacy_dereg_refund");
    balance_tx(
        511_000_000,
        vec![Certificate::RegDRepCert(cred(), 500_000_000, None)],
        None,
        "A2r_drep_deposit",
    );
    balance_tx(6_000_000, vec![], Some(5_000_000), "A2w_withdrawal");
}
