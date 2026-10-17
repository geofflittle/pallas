
#[cfg(test)]
mod review_probe {
    use super::*;
    use pallas_codec::minicbor;
    use pallas_crypto::key::ed25519::SecretKey;
    use pallas_primitives::alonzo::{TransactionInput, Value as PValue};
    use std::borrow::Cow;

    fn script_hash(script: &NativeScript) -> ScriptHash {
        let mut payload = vec![0u8];
        let _ = encode(script, &mut payload);
        PallasHasher::<224>::hash(&payload)
    }

    fn outcome(scripts: Vec<NativeScript>, target: usize, via_input: bool) -> String {
        outcome_both(scripts, target, via_input, None)
    }

    fn outcome_both(
        scripts: Vec<NativeScript>,
        target: usize,
        via_input: bool,
        cert_target: Option<usize>,
    ) -> String {
        let raw = hex::decode(include_str!("../../../test_data/shelley1.tx").trim()).unwrap();
        let tx: Tx = minicbor::decode(&raw).unwrap();
        let hash = script_hash(&scripts[target]);
        let input = TransactionInput {
            transaction_id: [5u8; 32].into(),
            index: 0,
        };
        let mut body: TransactionBody = tx.transaction_body.unwrap().clone();
        if via_input {
            body.inputs = vec![input.clone()];
            body.certificates = None;
        } else {
            body.inputs = Vec::new();
            body.certificates = Some(vec![Certificate::StakeDeregistration(
                StakeCredential::ScriptHash(hash),
            )]);
        }
        if let Some(cert_target) = cert_target {
            body.certificates = Some(vec![Certificate::StakeDeregistration(
                StakeCredential::ScriptHash(script_hash(&scripts[cert_target])),
            )]);
        }
        let body_bytes = minicbor::to_vec(&body).unwrap();
        let body: KeepRaw<TransactionBody> = minicbor::decode(&body_bytes).unwrap();
        let key = SecretKey::from([1u8; 32]);
        let mut wits: WitnessSet = minicbor::decode(&[0xa0]).unwrap();
        wits.vkeywitness = Some(vec![VKeyWitness {
            vkey: key.public_key().as_ref().to_vec().into(),
            signature: key.sign(body.original_hash()).as_ref().to_vec().into(),
        }]);
        let script_bytes: Vec<Vec<u8>> = scripts
            .iter()
            .map(|script| minicbor::to_vec(script).unwrap())
            .collect();
        wits.native_script = Some(
            script_bytes
                .iter()
                .map(|bytes| minicbor::decode(bytes).unwrap())
                .collect(),
        );
        let mut address = vec![0x71u8];
        address.extend_from_slice(hash.as_ref());
        let output = TransactionOutput {
            address: address.into(),
            amount: PValue::Coin(10),
            datum_hash: None,
        };
        let mut utxos = UTxOs::new();
        utxos.insert(
            MultiEraInput::AlonzoCompatible(Box::new(Cow::Owned(input))),
            MultiEraOutput::AlonzoCompatible(Box::new(Cow::Owned(output)), Era::Shelley),
        );
        format!("{:?}", check_witnesses(&body, &wits, &utxos))
    }

    #[test]
    fn review_probe_native_script_position() {
        let a = NativeScript::ScriptAll(vec![]);
        let b = NativeScript::ScriptAll(vec![NativeScript::ScriptAll(vec![])]);
        for via_input in [false, true] {
            for target in [0, 1] {
                println!(
                    "PROBE via_input={via_input} target={target} {}",
                    outcome(vec![a.clone(), b.clone()], target, via_input)
                );
            }
        }
        println!(
            "PROBE input_needs=0 cert_needs=1 {}",
            outcome_both(vec![a.clone(), b.clone()], 0, true, Some(1))
        );
    }
}
