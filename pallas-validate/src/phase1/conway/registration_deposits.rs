use super::*;
use pallas_codec::minicbor;
use pallas_primitives::{
    StakeCredential,
    conway::{Certificate, DRep, PostAlonzoTransactionOutput},
};

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
    let raw = hex::decode(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test_data/conway3.tx"
        ))
        .trim(),
    )
    .unwrap();
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
