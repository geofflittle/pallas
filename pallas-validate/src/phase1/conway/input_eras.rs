use super::*;
use pallas_codec::minicbor;
use pallas_traverse::{Era, MultiEraTx};

fn input() -> TransactionInput {
    TransactionInput {
        transaction_id: Hash::new([0; 32]),
        index: 0,
    }
}

fn legacy_output(address: Vec<u8>) -> Vec<u8> {
    minicbor::to_vec(pallas_primitives::alonzo::TransactionOutput {
        address: address.into(),
        amount: babbage::Value::Coin(5_000_000),
        datum_hash: None,
    })
    .unwrap()
}

fn utxos(era: Era, output: &[u8]) -> UTxOs<'_> {
    UTxOs::from([(
        MultiEraInput::AlonzoCompatible(Box::new(std::borrow::Cow::Owned(input()))),
        MultiEraOutput::decode(era, output).unwrap(),
    )])
}

fn key_address() -> Vec<u8> {
    [vec![0x61], vec![1; 28]].concat()
}

fn script_address() -> Vec<u8> {
    [vec![0x71], vec![2; 28]].concat()
}

fn byron_address() -> Vec<u8> {
    pallas_addresses::ByronAddress::from_base58(
        "Ae2tdPwUPEZLs4HtbuNey7tK4hTKrwNwYtGqp7bDfCy2WdR3P6735W5Yfpe",
    )
    .unwrap()
    .to_vec()
}

fn reward_address() -> Vec<u8> {
    [vec![0xe0], vec![3; 28]].concat()
}

fn undecodable_byron_address() -> Vec<u8> {
    pallas_addresses::ByronAddress::new(&[0x00], 0).to_vec()
}

#[test]
fn an_input_at_a_reward_or_undecodable_byron_address_is_refused() {
    let cbor = conway_tx_spending_input();
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

const V2_SCRIPT: [u8; 4] = [1, 2, 3, 4];

fn output_with_reference_script() -> Vec<u8> {
    let mut cbor = vec![0xa3, 0x00, 0x58, 0x1d]; // output, a 29 byte address
    cbor.extend(key_address());
    cbor.extend([0x01, 0x1a, 0x00, 0x4c, 0x4b, 0x40]); // five ada
    cbor.extend([0x03, 0xd8, 0x18, 0x47, 0x82, 0x02, 0x44]); // a PlutusV2 reference script
    cbor.extend(V2_SCRIPT);
    cbor
}

fn byron_output() -> Vec<u8> {
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

fn conway_tx_spending_input() -> Vec<u8> {
    let mut cbor = vec![0x84, 0xa3, 0x00, 0x81]; // transaction, body, one input
    cbor.extend(input_cbor());
    cbor.extend([0x01, 0x80, 0x02, 0x00]); // no outputs, no fee
    cbor.extend([0xa0, 0xf5, 0xf6]); // no witnesses, valid, no auxiliary data
    cbor
}

fn conway_tx_referencing_input() -> Vec<u8> {
    let mut cbor = vec![0x84, 0xa4]; // transaction, body
    cbor.extend([0x00, 0x80, 0x01, 0x80, 0x02, 0x00]); // no inputs, no outputs, no fee
    cbor.extend([0x12, 0x81]); // one reference input
    cbor.extend(input_cbor());
    cbor.extend([0xa0, 0xf5, 0xf6]); // no witnesses, valid, no auxiliary data
    cbor
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
    let cbor = conway_tx_spending_input();
    let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).unwrap();
    let body = &tx.as_conway().unwrap().transaction_body;
    let expected = pallas_crypto::hash::Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);

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
    let cbor = conway_tx_referencing_input();
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
fn a_byron_era_input_allows_no_plutus_language() {
    let cbor = conway_tx_spending_input();
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
    let policy = pallas_crypto::hash::Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
    let with_script = output_with_reference_script();
    let without_script = legacy_output(key_address());
    let (mut outcomes, mut expected) = (vec![], vec![]);
    for (cbor, spent) in [
        (conway_tx_spending_input(), true),
        (conway_tx_referencing_input(), false),
    ] {
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
    let script = pallas_crypto::hash::Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
    let holder = input();
    let locked = TransactionInput {
        transaction_id: Hash::new([9; 32]),
        index: 0,
    };
    let with_script = output_with_reference_script();
    let cbor = conway_tx_referencing_input();
    let mut outcomes = vec![];
    for purpose in ["spend", "mint", "cert", "reward", "vote", "propose"] {
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
                "reward" => {
                    let account = [vec![0xf0], hash.to_vec()].concat();
                    body.withdrawals = Some([(account.into(), 0)].into_iter().collect())
                }
                "vote" => {
                    let action = GovActionId {
                        transaction_id: Hash::new([0; 32]),
                        action_index: 0,
                    };
                    let vote = VotingProcedure {
                        vote: Vote::Yes,
                        anchor: None,
                    };
                    body.voting_procedures = Some(
                        [(
                            Voter::DRepScript(hash),
                            [(action, vote)].into_iter().collect(),
                        )]
                        .into_iter()
                        .collect(),
                    )
                }
                _ => {
                    let proposal = ProposalProcedure {
                        deposit: 0,
                        reward_account: [vec![0xe0], vec![0; 28]].concat().into(),
                        gov_action: GovAction::TreasuryWithdrawals(Default::default(), Some(hash)),
                        anchor: Anchor {
                            url: String::new(),
                            content_hash: Hash::new([0; 32]),
                        },
                    };
                    body.proposal_procedures = Some(vec![proposal].try_into().unwrap())
                }
            }
            tx.transaction_body = body.into();
            outcomes.push((purpose, hash == script, tx_languages(&tx, &utxos)));
        }
    }
    let expected: Vec<_> = outcomes
        .iter()
        .map(|(purpose, needed, _)| {
            let languages = if *needed {
                vec![Language::PlutusV2]
            } else {
                vec![]
            };
            (*purpose, *needed, languages)
        })
        .collect();
    assert_eq!(outcomes.len(), 12);
    assert_eq!(outcomes, expected);
}

#[test]
fn a_reference_script_on_a_spent_input_supplies_a_minting_policy() {
    let policy = pallas_crypto::hash::Hasher::<224>::hash_tagged(&V2_SCRIPT, 2);
    let mut cbor = vec![0x84, 0xa4, 0x00, 0x81]; // transaction, body, one input
    cbor.extend(input_cbor());
    cbor.extend([0x01, 0x80, 0x02, 0x00]); // no outputs, no fee
    cbor.extend([0x09, 0xa1, 0x58, 0x1c]); // mint under one policy
    cbor.extend(policy.as_ref());
    cbor.extend([0xa1, 0x41, 0x01, 0x01]); // one unit of asset 01
    cbor.extend([0xa0, 0xf5, 0xf6]); // no witnesses, valid, no auxiliary data
    let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).unwrap();
    let tx = tx.as_conway().unwrap();

    let with_script = output_with_reference_script();
    let without_script = legacy_output(key_address());

    let supplied = check_minting(&tx.transaction_body, tx, &utxos(Era::Conway, &with_script));
    let lacking = check_minting(
        &tx.transaction_body,
        tx,
        &utxos(Era::Conway, &without_script),
    );

    assert!(supplied.is_ok(), "{supplied:?}");
    assert!(
        matches!(lacking, Err(PostAlonzo(MintingLacksPolicy(missing))) if missing == policy),
        "{lacking:?}"
    );
}
