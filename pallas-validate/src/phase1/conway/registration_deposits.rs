//! Synthetic Conway cases isolate balance accounting from signatures and DELEG.
use super::*;
use pallas_codec::minicbor;
use pallas_primitives::{
    StakeCredential,
    conway::{Certificate, DRep, LegacyTransactionOutput, PostAlonzoTransactionOutput},
};

const KEY_DEPOSIT: u64 = 2_000_000;
const INPUT: u64 = 20_000_000;

fn registrations(declared: u64) -> Vec<Certificate> {
    let credential = StakeCredential::AddrKeyhash([1; 28].into());
    let pool = [2; 28].into();
    vec![
        Certificate::StakeRegistration(credential.clone()),
        Certificate::Reg(credential.clone(), declared),
        Certificate::StakeRegDeleg(credential.clone(), pool, declared),
        Certificate::VoteRegDeleg(credential.clone(), DRep::Abstain, declared),
        Certificate::StakeVoteRegDeleg(credential, pool, DRep::Abstain, declared),
    ]
}

fn output(coin: u64, legacy: bool) -> TransactionOutput<'static> {
    let mut address = vec![0x61];
    address.extend([0; 28]);
    if legacy {
        TransactionOutput::Legacy(
            LegacyTransactionOutput {
                address: address.into(),
                amount: babbage::Value::Coin(coin),
                datum_hash: None,
            }
            .into(),
        )
    } else {
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
}

fn with_tx(certificates: Vec<Certificate>, reserved: u64, legacy: bool, test: impl FnOnce(Tx<'_>)) {
    // Reuse a native Conway body's structure, with synthetic amounts/certificates.
    let raw = hex::decode(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test_data/conway3.tx"
        ))
        .trim(),
    )
    .unwrap();
    let mut tx: Tx = minicbor::decode(&raw).unwrap();
    let mut body = (*tx.transaction_body).clone();
    assert_eq!(body.inputs.len(), 1);
    body.fee = 200_000;
    body.mint = None;
    body.outputs = vec![output(INPUT - body.fee - reserved, legacy)];
    body.certificates = if certificates.is_empty() {
        None
    } else {
        Some(certificates.try_into().unwrap())
    };
    tx.transaction_body = body.into();
    test(tx);
}

fn balance(tx: &Tx, legacy_input: bool, key_deposit: u64) -> ValidationResult {
    let input = output(INPUT, legacy_input);
    let utxos = UTxOs::from([(
        MultiEraInput::from_alonzo_compatible(&tx.transaction_body.inputs[0]),
        MultiEraOutput::from_conway(&input),
    )]);
    check_preservation_of_value(tx, &utxos, key_deposit, DREP_DEPOSIT, PROPOSAL_DEPOSIT)
}

const DREP_DEPOSIT: u64 = 5_000_000;
const PROPOSAL_DEPOSIT: u64 = 7_000_000;

// Balance results for a body whose deposits less refunds come to `net`, with
// the output short by `net - 1`, `net` and `net + 1`.
fn balance_around(
    certificates: Vec<Certificate>,
    prepare: impl Fn(&mut TransactionBody),
    net: i64,
) -> Vec<String> {
    [net - 1, net, net + 1]
        .into_iter()
        .map(|reserved| {
            let mut outcome = String::new();
            with_tx(certificates.clone(), 0, false, |mut tx| {
                let mut body = (*tx.transaction_body).clone();
                let coin = (INPUT - body.fee) as i64 - reserved;
                body.outputs = vec![output(coin as u64, false)];
                prepare(&mut body);
                tx.transaction_body = body.into();
                outcome = format!("{:?}", balance(&tx, false, KEY_DEPOSIT));
            });
            outcome
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
    let key = KEY_DEPOSIT as i64;
    let cases: Vec<(&str, Vec<String>)> = vec![
        ("nothing", balance_around(vec![], none, 0)),
        (
            "registration and unrelated deregistration",
            balance_around(
                vec![
                    Certificate::Reg(credential(1), KEY_DEPOSIT),
                    Certificate::UnReg(credential(2), KEY_DEPOSIT),
                ],
                none,
                0,
            ),
        ),
        (
            "legacy registration and unrelated deregistration",
            balance_around(
                vec![
                    Certificate::StakeRegistration(credential(1)),
                    Certificate::StakeDeregistration(credential(2)),
                ],
                none,
                0,
            ),
        ),
        (
            "deregistration refunds its declared deposit",
            balance_around(
                vec![Certificate::UnReg(credential(2), 3_000_000)],
                none,
                -3_000_000,
            ),
        ),
        (
            "legacy deregistration refunds the key deposit",
            balance_around(
                vec![Certificate::StakeDeregistration(credential(2))],
                none,
                -key,
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
            "drep deregistration refunds its declared deposit",
            balance_around(
                vec![Certificate::UnRegDRepCert(credential(1), 4_000_000)],
                none,
                -4_000_000,
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
fn all_stake_registration_forms_charge_the_protocol_deposit() {
    for key_deposit in [KEY_DEPOSIT, 3_000_000] {
        for certificate in registrations(key_deposit) {
            for legacy_output in [false, true] {
                with_tx(
                    vec![certificate.clone()],
                    key_deposit,
                    legacy_output,
                    |tx| {
                        for legacy_input in [false, true] {
                            balance(&tx, legacy_input, key_deposit).unwrap();
                        }
                    },
                );
            }
        }
    }
}

#[test]
fn absent_or_delegation_only_certificates_do_not_charge_a_deposit() {
    let credential = StakeCredential::AddrKeyhash([1; 28].into());
    for certificates in [
        vec![],
        vec![Certificate::StakeDelegation(
            credential.clone(),
            [2; 28].into(),
        )],
        vec![Certificate::VoteDeleg(credential, DRep::Abstain)],
    ] {
        with_tx(certificates, 0, false, |tx| {
            balance(&tx, false, KEY_DEPOSIT).unwrap()
        });
    }
}

#[test]
fn balancing_on_a_declared_deposit_unlike_the_protocol_deposit_fails() {
    for delta in [-1, 1] {
        let declared = KEY_DEPOSIT.checked_add_signed(delta).unwrap();
        // The legacy registration declares no amount.
        for certificate in registrations(declared).into_iter().skip(1) {
            with_tx(vec![certificate], declared, false, |tx| {
                assert!(matches!(
                    balance(&tx, false, KEY_DEPOSIT),
                    Err(PostAlonzo(PreservationOfValue))
                ));
            });
        }
    }
}
