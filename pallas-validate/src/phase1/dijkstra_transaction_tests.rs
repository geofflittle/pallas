use super::*;
#[test]
fn dijkstra_observed_metadata_reaches_input_validation() {
    let raw = hex::decode(
        include_str!("../../../test_data/musashi-dijkstra-validation/order.tx.hex").trim(),
    )
    .unwrap();
    let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &raw).unwrap();
    assert_eq!(
        tx.hash().to_string(),
        "11ef408b2575189a212b9048472fe445e303f5f0d1382769aae086f3728dc656"
    );
    // Deliberately missing state isolates the unsupported-feature gate, not acceptance.
    let result = validate_txs(
        std::slice::from_ref(&tx),
        &dijkstra_tests::env(),
        &UTxOs::new(),
        &mut CertState::default(),
    );
    assert_eq!(format!("{result:?}"), "Err(PostAlonzo(InputNotInUTxO))");
}

#[test]
fn dijkstra_observed_order_valid_context() {
    let raw = hex::decode(
        include_str!("../../../test_data/musashi-dijkstra-validation/order.tx.hex").trim(),
    )
    .unwrap();
    let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &raw).unwrap();
    let producer = hex::decode(
        include_str!("../../../test_data/musashi-dijkstra-validation/order-producer.body.hex")
            .trim(),
    )
    .unwrap();
    let body: pallas_primitives::dijkstra::TransactionBody =
        pallas_codec::minicbor::decode(&producer).unwrap();
    assert_eq!(
        pallas_crypto::hash::Hasher::<256>::hash(&producer),
        tx.inputs()[0].hash().to_owned()
    );
    let input = hex::decode(
        include_str!("../../../test_data/musashi-dijkstra-validation/order.input.hex").trim(),
    )
    .unwrap();
    assert_eq!(
        pallas_codec::minicbor::to_vec(&body.outputs[2]).unwrap(),
        input
    );
    let utxos = [(
        tx.inputs()[0].to_owned(),
        pallas_traverse::MultiEraOutput::decode(Era::Dijkstra, &input).unwrap(),
    )]
    .into_iter()
    .collect();
    // Constructed replay context: historical parameters and a stipulated later slot.
    // No historical/current acceptance is claimed; no time bounds or certificates used.
    // The producer proves the input bytes, not its current unspent status.
    let mut env = dijkstra_tests::env();
    env.block_slot = 2_100_000;
    let result = validate_txs(
        std::slice::from_ref(&tx),
        &env,
        &utxos,
        &mut CertState::default(),
    );
    assert!(result.is_ok(), "{result:?}");
}

use crate::utils::dijkstra_state::{
    DijkstraAccount, DijkstraLedgerOverlay, DijkstraLedgerSnapshot, DijkstraPool, DijkstraProposal,
    DijkstraProposalAction,
};
use pallas_codec::{
    minicbor,
    utils::{CborWrap, KeepRaw, KeyValuePairs, Nullable},
};
use pallas_crypto::{
    hash::{Hash, Hasher},
    key::ed25519::SecretKey,
};
use pallas_primitives::{conway as c, dijkstra as n};
use pallas_traverse::{MultiEraInput, MultiEraOutput};
use std::collections::BTreeMap;

fn utxos<'a>(inputs: &'a [(n::TransactionInput, n::TransactionOutput<'_>)]) -> UTxOs<'a> {
    inputs
        .iter()
        .map(|(i, o)| {
            (
                MultiEraInput::from_alonzo_compatible(i),
                MultiEraOutput::from_dijkstra(o),
            )
        })
        .collect()
}
fn run(
    tx: &n::BlockTransaction<'_>,
    inputs: &[(n::TransactionInput, n::TransactionOutput<'_>)],
    env: &Environment,
    state: &CertState,
) -> crate::utils::ValidationResult {
    validate_txs(
        &[MultiEraTx::from_dijkstra(tx)],
        env,
        &utxos(inputs),
        &mut state.clone(),
    )
}
fn output_mut<'a, 'b>(
    o: &'a mut n::TransactionOutput<'b>,
) -> &'a mut KeepRaw<'b, n::PostAlonzoTransactionOutput<'b>> {
    let n::TransactionOutput::PostAlonzo(o) = o else {
        panic!()
    };
    o
}
fn auxiliary(tx: &mut n::BlockTransaction<'_>, text: &str) {
    set_auxiliary(
        tx,
        n::AuxiliaryData::Shelley(BTreeMap::from([(674, n::Metadatum::Text(text.into()))])),
    );
}
fn set_auxiliary<'b>(tx: &mut n::BlockTransaction<'b>, data: n::AuxiliaryData<'b>) {
    tx.auxiliary_data = Nullable::Some(data.into());
    let Nullable::Some(aux) = &tx.auxiliary_data else {
        panic!()
    };
    tx.transaction_body.auxiliary_data_hash =
        Some(Hasher::<256>::hash(&minicbor::to_vec(aux).unwrap()));
    dijkstra_tests::sign(tx);
}
fn set_raw_auxiliary(tx: &mut n::BlockTransaction<'_>, aux: &str) {
    let raw: &'static [u8] = Box::leak(hex::decode(aux).unwrap().into_boxed_slice());
    tx.auxiliary_data = Nullable::Some(minicbor::decode(raw).unwrap());
    tx.transaction_body.auxiliary_data_hash = Some(Hasher::<256>::hash(raw));
    dijkstra_tests::sign(tx);
}
#[test]
fn dijkstra_metadata_commitment_and_limits() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let check = |tx: &n::BlockTransaction<'_>| {
            run(tx, &inputs, &dijkstra_tests::env(), &CertState::default())
        };
        auxiliary(&mut tx, "metadata test");
        assert!(check(&tx).is_ok());
        let valid = tx.clone();
        tx.transaction_body.auxiliary_data_hash = None;
        dijkstra_tests::sign(&mut tx);
        dijkstra_tests::error(check(&tx), "PostAlonzo(MetadataHash)");
        tx = valid.clone();
        tx.auxiliary_data = Nullable::Null;
        dijkstra_tests::error(check(&tx), "PostAlonzo(MetadataHash)");
        tx = valid.clone();
        tx.transaction_body.auxiliary_data_hash = Some(Hash::from([0; 32]));
        dijkstra_tests::sign(&mut tx);
        dijkstra_tests::error(check(&tx), "PostAlonzo(MetadataHash)");
        tx = valid;
        auxiliary(&mut tx, &"x".repeat(64));
        assert!(check(&tx).is_ok());
        auxiliary(&mut tx, &"x".repeat(65));
        dijkstra_tests::error(check(&tx), "DijkstraMalformed(\"metadata\")");
    });
}
#[test]
fn dijkstra_auxiliary_scripts_are_refused_only_when_malformed() {
    let (well_formed, malformed) = (
        None::<Vec<u8>>,
        "DijkstraUnsupported(\"new Plutus script validation requires phase2 feature\")",
    );
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let check = |tx: &n::BlockTransaction<'_>| {
            run(tx, &inputs, &dijkstra_tests::env(), &CertState::default())
        };
        let native = || Some(vec![n::NativeScript::InvalidBefore(0).into()]);
        set_auxiliary(
            &mut tx,
            n::AuxiliaryData::ShelleyMa(n::ShelleyMaAuxiliaryData {
                transaction_metadata: BTreeMap::new(),
                auxiliary_scripts: native(),
            }),
        );
        let result = check(&tx);
        assert!(result.is_ok(), "{result:?}");
        let n::AuxiliaryData::PostAlonzo(mut data) = dijkstra_tests::plutus_auxiliary(1, &[1])
        else {
            unreachable!()
        };
        data.plutus_v1_scripts = Some(vec![]);
        data.native_scripts = native();
        set_auxiliary(&mut tx, n::AuxiliaryData::PostAlonzo(data));
        let result = check(&tx);
        assert!(result.is_ok(), "{result:?}");
        for version in 1..=4 {
            if let Some(bytes) = &well_formed {
                set_auxiliary(&mut tx, dijkstra_tests::plutus_auxiliary(version, bytes));
                let result = check(&tx);
                assert!(result.is_ok(), "PlutusV{version}: {result:?}");
            }
            set_auxiliary(&mut tx, dijkstra_tests::plutus_auxiliary(version, &[1]));
            dijkstra_tests::error(check(&tx), malformed);
        }
    });
}
#[test]
fn dijkstra_auxiliary_array_holds_exactly_two_elements() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let check = |tx: &n::BlockTransaction<'_>| {
            run(tx, &inputs, &dijkstra_tests::env(), &CertState::default())
        };
        for aux in ["82a080", "9fa080ff", "82a081820400"] {
            set_raw_auxiliary(&mut tx, aux);
            let result = check(&tx);
            assert!(result.is_ok(), "{aux}: {result:?}");
        }
        for aux in ["81a0", "83a08000", "83a08182040000", "9fa0ff", "9fa08000ff"] {
            set_raw_auxiliary(&mut tx, aux);
            assert_eq!(
                check(&tx).err().map(|e| format!("{e:?}")).as_deref(),
                Some("DijkstraMalformed(\"metadata\")"),
                "{aux}"
            );
        }
    });
}
#[test]
fn dijkstra_an_unknown_auxiliary_key_is_malformed_metadata() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        // A tagged auxiliary map with the unknown key 6.
        set_raw_auxiliary(&mut tx, "d90103a106f6");
        dijkstra_tests::error(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
            "DijkstraMalformed(\"metadata\")",
        );
    });
}
#[test]
fn dijkstra_metadata_is_decoded_before_auxiliary_scripts() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let n::AuxiliaryData::PostAlonzo(mut data) = dijkstra_tests::plutus_auxiliary(1, &[1])
        else {
            unreachable!()
        };
        data.metadata = Some(BTreeMap::from([(674, n::Metadatum::Text("x".repeat(65)))]));
        set_auxiliary(&mut tx, n::AuxiliaryData::PostAlonzo(data));
        dijkstra_tests::error(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
            "DijkstraMalformed(\"metadata\")",
        );
    });
}
#[test]
fn dijkstra_metadata_is_decoded_before_its_hash() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        auxiliary(&mut tx, &"x".repeat(65));
        tx.transaction_body.auxiliary_data_hash = Some(Hash::from([0; 32]));
        dijkstra_tests::sign(&mut tx);
        dijkstra_tests::error(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
            "DijkstraMalformed(\"metadata\")",
        );
    });
}
fn value(coin: u64, policy: Hash<28>, quantity: u64) -> c::Value {
    c::Value::Multiasset(
        coin,
        BTreeMap::from([(
            policy,
            BTreeMap::from([(b"token".to_vec().into(), quantity.try_into().unwrap())]),
        )]),
    )
}
#[test]
fn dijkstra_multiasset_conservation_and_script_output() {
    dijkstra_tests::synthetic(|mut tx, mut input| {
        let policy = Hash::from([8; 28]);
        let coin = MultiEraOutput::from_dijkstra(&input).value().coin();
        output_mut(&mut input).value = value(coin, policy, 7);
        let fee = tx.transaction_body.fee;
        let output = output_mut(first_output(&mut tx));
        output.value = value(coin - fee, policy, 7);
        output.address = pallas_addresses::ShelleyAddress::new(
            pallas_addresses::Network::Testnet,
            pallas_addresses::ShelleyPaymentPart::Script(Hash::from([9; 28])),
            pallas_addresses::ShelleyDelegationPart::Null,
        )
        .to_vec()
        .into();
        output.datum_option = Some(
            c::DatumOption::Data(CborWrap(
                n::PlutusData::BigInt(n::BigInt::Int(42.into())).into(),
            ))
            .into(),
        );
        dijkstra_tests::sign(&mut tx);
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        assert!(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()).is_ok(),
            "script-locked output must not execute a script"
        );
        output_mut(first_output(&mut tx)).value = value(coin - fee, policy, 8);
        dijkstra_tests::sign(&mut tx);
        dijkstra_tests::error(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
            "PostAlonzo(PreservationOfValue)",
        );
    });
}

fn first_output<'a, 'b>(tx: &'a mut n::BlockTransaction<'b>) -> &'a mut n::TransactionOutput<'b> {
    match &mut tx.transaction_body.outputs {
        pallas_codec::utils::MaybeIndefArray::Def(xs)
        | pallas_codec::utils::MaybeIndefArray::Indef(xs) => &mut xs[0],
    }
}

#[test]
fn dijkstra_preserves_original_auxiliary_bytes() {
    dijkstra_tests::synthetic(|mut tx, input| {
        // Indefinite metadata map: same value, different committed bytes.
        let bytes: &'static [u8] = Box::leak(
            hex::decode("bf1902a26474657374ff")
                .unwrap()
                .into_boxed_slice(),
        );
        let aux: KeepRaw<n::AuxiliaryData> = minicbor::decode(bytes).unwrap();
        assert_ne!(minicbor::to_vec(&*aux).unwrap(), bytes);
        tx.auxiliary_data = Nullable::Some(aux);
        tx.transaction_body.auxiliary_data_hash = Some(Hasher::<256>::hash(bytes));
        dijkstra_tests::sign(&mut tx);
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        assert!(run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()).is_ok());
        let Nullable::Some(aux) = &tx.auxiliary_data else {
            panic!()
        };
        tx.transaction_body.auxiliary_data_hash =
            Some(Hasher::<256>::hash(&minicbor::to_vec(&**aux).unwrap()));
        dijkstra_tests::sign(&mut tx);
        dijkstra_tests::error(
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
            "PostAlonzo(MetadataHash)",
        );
    });
}
#[test]
fn dijkstra_invalid_multiasset_encodings() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let coin = MultiEraOutput::from_dijkstra(&input).value().coin() - tx.transaction_body.fee;
        let address = MultiEraOutput::from_dijkstra(&input)
            .address()
            .unwrap()
            .to_vec();
        for (name, amount, duplicate, empty) in [
            (vec![1], 0u64, false, false),
            (vec![1; 33], 1, false, false),
            (vec![1], 1, true, false),
            (vec![1], 1, false, true),
        ] {
            let mut e = minicbor::Encoder::new(Vec::new());
            e.array(2)
                .unwrap()
                .bytes(&address)
                .unwrap()
                .array(2)
                .unwrap()
                .u64(coin)
                .unwrap()
                .map(if empty { 0 } else { 1 })
                .unwrap();
            if !empty {
                e.bytes(&[8; 28])
                    .unwrap()
                    .map(if duplicate { 2 } else { 1 })
                    .unwrap()
                    .bytes(&name)
                    .unwrap()
                    .u64(amount)
                    .unwrap();
                if duplicate {
                    e.bytes(&name).unwrap().u64(amount).unwrap();
                }
            }
            let raw = e.into_writer();
            let decoded: n::TransactionOutput = minicbor::decode(&raw).unwrap();
            let n::TransactionOutput::Legacy(decoded) = decoded else {
                panic!()
            };
            tx.transaction_body.outputs =
                pallas_codec::utils::MaybeIndefArray::Def(vec![n::TransactionOutput::Legacy(
                    decoded.to_owned(),
                )]);
            dijkstra_tests::sign(&mut tx);
            let inputs = vec![(tx.transaction_body.inputs[0].clone(), input.clone())];
            dijkstra_tests::error(
                run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default()),
                "DijkstraMalformed(\"multi-asset value\")",
            );
        }
    });
}

fn test_key() -> Hash<28> {
    Hasher::<224>::hash(SecretKey::from([71; 32]).public_key().as_ref())
}
fn test_account() -> n::RewardAccount {
    let mut account = vec![0xe0];
    account.extend(test_key().as_ref());
    account.into()
}
fn free_env() -> Environment {
    let mut env = dijkstra_tests::env();
    let MultiEraProtocolParameters::Dijkstra(ref mut pp) = env.prot_params else {
        unreachable!()
    };
    pp.minfee_a = 0;
    pp.minfee_b = 0;
    pp.governance_action_deposit = 0;
    env
}
fn registered(balance: u64) -> CertState {
    let mut state = CertState::default();
    state.dijkstra.accounts.insert(
        n::StakeCredential::AddrKeyhash(test_key()),
        Some(DijkstraAccount {
            balance,
            deposit: 0,
        }),
    );
    state.dijkstra.pools.insert(
        test_key(),
        DijkstraPool {
            vrf: [9; 32].into(),
            future_vrf: None,
        },
    );
    state
}
fn with_proposal<'a>(
    tx: &n::BlockTransaction<'a>,
    action: n::GovAction,
) -> n::BlockTransaction<'a> {
    let mut tx = tx.clone();
    tx.transaction_body.proposal_procedures =
        n::NonEmptySet::from_vec(vec![n::ProposalProcedure {
            deposit: 0,
            reward_account: test_account(),
            gov_action: action,
            anchor: n::Anchor {
                url: "https://proposal.example".into(),
                content_hash: [0; 32].into(),
            },
        }]);
    dijkstra_tests::sign(&mut tx);
    tx
}

#[test]
fn dijkstra_a_stated_treasury_equals_the_ledger_treasury() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let utxos = utxos(&inputs);
        let state = DijkstraLedgerSnapshot {
            treasury: 7_000_000,
            ..Default::default()
        };
        let MultiEraProtocolParameters::Dijkstra(pp) = free_env().prot_params else {
            unreachable!()
        };
        let stating = |treasury| {
            let mut tx = tx.clone();
            tx.transaction_body.treasury_value = Some(treasury);
            dijkstra_tests::sign(&mut tx);
            dijkstra::validate_dijkstra_tx(
                &tx,
                &utxos,
                &pp,
                &1400007,
                &0,
                &DijkstraLedgerOverlay::new(&state),
            )
            .map(|_| ())
        };
        let result = stating(7_000_000);
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(stating(7_000_001), "DijkstraTreasuryValueMismatch");
    });
}

#[test]
fn dijkstra_stake_pools_vote_only_on_security_relevant_parameter_changes() {
    dijkstra_tests::synthetic(|tx, input| {
        let mut second = tx.transaction_body.inputs[0].clone();
        second.index = 99;
        let inputs = vec![
            (tx.transaction_body.inputs[0].clone(), input.clone()),
            (second.clone(), input),
        ];
        let pool_votes_on = |update: &str| {
            let update = minicbor::decode(&hex::decode(update).unwrap()).unwrap();
            let proposal = with_proposal(
                &tx,
                n::GovAction::ParameterChange(None, Box::new(update), None),
            );
            // Decoding keeps the body bytes that the proposal id hashes.
            let bytes: &'static [u8] =
                Box::leak(minicbor::to_vec(&proposal).unwrap().into_boxed_slice());
            let proposal: n::BlockTransaction = minicbor::decode(bytes).unwrap();
            let id = n::GovActionId {
                transaction_id: Hasher::<256>::hash(proposal.transaction_body.raw_cbor()),
                action_index: 0,
            };
            let mut vote = tx.clone();
            vote.transaction_body.inputs = vec![second.clone()].into();
            vote.transaction_body.voting_procedures = Some(BTreeMap::from([(
                n::Voter::StakePoolKey(test_key()),
                BTreeMap::from([(
                    id,
                    n::VotingProcedure {
                        vote: n::Vote::Yes,
                        anchor: None,
                    },
                )]),
            )]));
            dijkstra_tests::sign(&mut vote);
            validate_txs(
                &[
                    MultiEraTx::from_dijkstra(&proposal),
                    MultiEraTx::from_dijkstra(&vote),
                ],
                &free_env(),
                &utxos(&inputs),
                &mut registered(0),
            )
        };
        // Key 0, the fee per byte, is security relevant and key 5, the key deposit, is not.
        let result = pool_votes_on("a100182c");
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(
            pool_votes_on("a1051a001e8480"),
            "DijkstraInvalidGovernance(\"voter may not vote on the action\")",
        );
    });
}

#[test]
fn dijkstra_a_hard_fork_follows_its_predecessor_up_to_the_highest_major_version() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let pending = n::GovActionId {
            transaction_id: [3; 32].into(),
            action_index: 0,
        };
        let mut state = registered(0);
        state.dijkstra.proposals.push((
            pending.clone(),
            DijkstraProposal {
                action: DijkstraProposalAction::HardForkInitiation { version: (13, 0) },
                expires_after: 100,
            },
        ));
        let proposing = |prev: Option<&n::GovActionId>, version| {
            let tx = with_proposal(
                &tx,
                n::GovAction::HardForkInitiation(prev.cloned(), version),
            );
            run(&tx, &inputs, &free_env(), &state)
        };
        for (prev, version) in [(None, (13, 0)), (Some(&pending), (13, 1))] {
            let result = proposing(prev, version);
            assert!(result.is_ok(), "{version:?} {result:?}");
        }
        dijkstra_tests::error(
            proposing(Some(&pending), (14, 0)),
            "DijkstraInvalidGovernance(\"hard fork version cannot follow\")",
        );
    });
}

#[test]
fn dijkstra_a_pool_retires_at_most_the_maximum_epoch_ahead() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let mut state = registered(0);
        state.dijkstra.epoch = 64;
        let env = free_env();
        let MultiEraProtocolParameters::Dijkstra(pp) = &env.prot_params else {
            unreachable!()
        };
        let latest = 64 + pp.maximum_epoch;
        let retiring = |epoch| {
            let mut tx = tx.clone();
            tx.transaction_body.certificates =
                n::NonEmptySet::from_vec(vec![n::Certificate::PoolRetirement(test_key(), epoch)]);
            dijkstra_tests::sign(&mut tx);
            run(&tx, &inputs, &env, &state)
        };
        let result = retiring(latest);
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(
            retiring(latest + 1),
            "DijkstraInvalidCertificate(\"pool retirement epoch\")",
        );
    });
}

#[test]
fn dijkstra_a_balance_interval_excludes_its_upper_bound() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let bounded_by = |upper| {
            let mut tx = tx.clone();
            tx.transaction_body.account_balance_intervals = Some(BTreeMap::from([(
                test_account(),
                n::AccountBalanceInterval::Bounded(0, upper),
            )]));
            dijkstra_tests::sign(&mut tx);
            run(&tx, &inputs, &free_env(), &registered(10))
        };
        let result = bounded_by(11);
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(
            bounded_by(10),
            "DijkstraInvalidAccount(\"balance outside its interval\")",
        );
    });
}

#[test]
fn dijkstra_collateral_inputs_are_at_most_the_maximum() {
    dijkstra_tests::synthetic(|tx, input| {
        let collateral = |index| n::TransactionInput {
            transaction_id: [12; 32].into(),
            index,
        };
        let inputs = vec![
            (tx.transaction_body.inputs[0].clone(), input.clone()),
            (collateral(0), input.clone()),
            (collateral(1), input),
        ];
        let mut env = free_env();
        let MultiEraProtocolParameters::Dijkstra(ref mut pp) = env.prot_params else {
            unreachable!()
        };
        pp.max_collateral_inputs = 1;
        let with_collateral = |count| {
            let mut tx = tx.clone();
            tx.transaction_body.collateral =
                n::NonEmptySet::from_vec((0..count).map(collateral).collect());
            dijkstra_tests::sign(&mut tx);
            run(&tx, &inputs, &env, &CertState::default())
        };
        let result = with_collateral(1);
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(with_collateral(2), "PostAlonzo(TooManyCollaterals)");
    });
}

#[test]
fn dijkstra_script_locked_collateral_is_allowed_without_redeemers() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let collateral = n::TransactionInput {
            transaction_id: [12; 32].into(),
            index: 0,
        };
        let mut locked = input.clone();
        output_mut(&mut locked).address = pallas_addresses::ShelleyAddress::new(
            pallas_addresses::Network::Testnet,
            pallas_addresses::ShelleyPaymentPart::Script(Hash::from([5; 28])),
            pallas_addresses::ShelleyDelegationPart::Null,
        )
        .to_vec()
        .into();
        let inputs = vec![
            (tx.transaction_body.inputs[0].clone(), input),
            (collateral.clone(), locked),
        ];
        tx.transaction_body.collateral = n::NonEmptySet::from_vec(vec![collateral]);
        dijkstra_tests::sign(&mut tx);
        let result = run(&tx, &inputs, &free_env(), &CertState::default());
        assert!(result.is_ok(), "{result:?}");
    });
}

#[test]
fn dijkstra_min_fee_prices_memory_and_steps_separately() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let MultiEraProtocolParameters::Dijkstra(pp) = free_env().prot_params else {
            unreachable!()
        };
        let fee = |mem, steps| {
            let mut tx = tx.clone();
            tx.transaction_witness_set.redeemer =
                Some(KeepRaw::from(n::Redeemers(BTreeMap::from([(
                    n::RedeemersKey {
                        tag: n::RedeemerTag::Spend,
                        index: 0,
                    },
                    n::RedeemersValue {
                        data: n::PlutusData::BigInt(n::BigInt::Int(0.into())),
                        ex_units: n::ExUnits { mem, steps },
                    },
                )]))));
            dijkstra::dijkstra_min_fee(&tx, &utxos(&inputs), &pp).unwrap()
        };
        // The fixture prices memory at 577/10000 and steps at 721/10000000.
        assert_eq!(
            [fee(0, 0), fee(10_000, 0), fee(0, 10_000_000)],
            [0, 577, 721]
        );
    });
}

#[test]
fn dijkstra_metadata_text_inside_arrays_and_maps_is_at_most_64_bytes() {
    dijkstra_tests::synthetic(|mut tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        for (size, expected) in [(64, None), (65, Some("DijkstraMalformed(\"metadata\")"))] {
            let text = || n::Metadatum::Text("x".repeat(size));
            let nested = [
                n::Metadatum::Array(vec![text()]),
                n::Metadatum::Map(KeyValuePairs::Def(vec![(
                    n::Metadatum::Text("key".into()),
                    text(),
                )])),
            ];
            for item in nested {
                set_auxiliary(
                    &mut tx,
                    n::AuxiliaryData::Shelley(BTreeMap::from([(674, item)])),
                );
                let result = run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default());
                match expected {
                    None => assert!(result.is_ok(), "{result:?}"),
                    Some(expected) => dijkstra_tests::error(result, expected),
                }
            }
        }
    });
}

#[test]
fn dijkstra_output_asset_names_are_at_most_32_bytes() {
    dijkstra_tests::synthetic(|tx, mut input| {
        let named = |coin, name: Vec<u8>| {
            c::Value::Multiasset(
                coin,
                BTreeMap::from([(
                    Hash::from([8; 28]),
                    BTreeMap::from([(name.into(), 1u64.try_into().unwrap())]),
                )]),
            )
        };
        let coin = MultiEraOutput::from_dijkstra(&input).value().coin();
        output_mut(&mut input).value = named(coin, vec![1; 32]);
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let paying = |name| {
            let mut tx = tx.clone();
            let paid = coin - tx.transaction_body.fee;
            output_mut(first_output(&mut tx)).value = named(paid, name);
            dijkstra_tests::sign(&mut tx);
            run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default())
        };
        let result = paying(vec![1; 32]);
        assert!(result.is_ok(), "{result:?}");
        dijkstra_tests::error(
            paying(vec![1; 33]),
            "DijkstraMalformed(\"multi-asset value\")",
        );
    });
}

#[test]
fn dijkstra_an_indefinite_length_body_map_is_accepted() {
    dijkstra_tests::synthetic(|tx, input| {
        let inputs = vec![(tx.transaction_body.inputs[0].clone(), input)];
        let definite = minicbor::to_vec(&tx.transaction_body).unwrap();
        assert!((0xa1..=0xb7).contains(&definite[0]));
        let mut indefinite = vec![0xbf];
        indefinite.extend(&definite[1..]);
        indefinite.push(0xff);
        for raw in [definite, indefinite] {
            let raw: &'static [u8] = Box::leak(raw.into_boxed_slice());
            let mut tx = tx.clone();
            tx.transaction_body = minicbor::decode(raw).unwrap();
            dijkstra_tests::sign(&mut tx);
            let result = run(&tx, &inputs, &dijkstra_tests::env(), &CertState::default());
            assert!(result.is_ok(), "{:02x} {result:?}", raw[0]);
        }
    });
}
