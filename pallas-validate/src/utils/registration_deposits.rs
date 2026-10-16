use super::*;
use pallas_primitives::conway::{Certificate, DRep};
use std::borrow::Cow;

fn certificates(raw: &[Certificate]) -> Vec<MultiEraCert<'_>> {
    raw.iter()
        .map(|cert| MultiEraCert::Conway(Box::new(Cow::Borrowed(cert))))
        .collect()
}

#[test]
fn deposit_and_refund_sum_overflow_is_rejected() {
    let credential = |id| StakeCredential::AddrKeyhash([id; 28].into());
    for raw in [
        [1, 2].map(|id| Certificate::Reg(credential(id), 0)),
        [1, 2].map(|id| Certificate::StakeDeregistration(credential(id))),
    ] {
        assert!(matches!(
            certificate_deposits_and_refunds(&certificates(&raw), u64::MAX, 0),
            Err(ValidationError::PostAlonzo(PostAlonzoError::NegativeValue))
        ));
    }
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
        .map(|(cert, _)| {
            certificate_deposits_and_refunds(&certificates(std::slice::from_ref(cert)), 2, 5)
                .unwrap()
        })
        .collect();
    let expected: Vec<_> = cases.iter().map(|(_, sums)| *sums).collect();
    assert_eq!(actual, expected);
    let all: Vec<_> = cases.into_iter().map(|(cert, _)| cert).collect();
    assert_eq!(
        certificate_deposits_and_refunds(&certificates(&all), 2, 5).unwrap(),
        (15, 17)
    );
}
