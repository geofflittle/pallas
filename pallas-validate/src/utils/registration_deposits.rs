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

#[cfg(feature = "unstable")]
mod native {
    use super::*;
    use pallas_traverse::{MultiEraBlock, MultiEraTx};
    use std::{fs, path::PathBuf};

    fn read(path: impl AsRef<std::path::Path>) -> Vec<u8> {
        hex::decode(fs::read_to_string(path).unwrap().trim()).unwrap()
    }

    fn captured_registration(case: &str) {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_data/musashi-registration");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap();
        let key_deposit = manifest["parameters"]["key_deposit"].as_u64().unwrap();
        let case = &manifest["cases"][case];
        let block_file = &manifest["blocks"][case["slot"].to_string()]["file"];
        let bytes = read(root.join(block_file.as_str().unwrap()));
        let block = MultiEraBlock::decode(&bytes).unwrap();
        let txs = block.txs();
        let tx = &txs[case["transaction_index"].as_u64().unwrap() as usize];
        assert_eq!(tx.era(), Era::Dijkstra);
        assert_eq!(
            tx.hash().to_string(),
            case["transaction_id"].as_str().unwrap()
        );
        let native = tx.as_dijkstra().unwrap();
        assert!(native.success);
        let body = &native.transaction_body;
        assert_eq!(
            body.raw_cbor(),
            read(root.join(case["body"].as_str().unwrap()))
        );
        assert!(body.mint.is_none());
        assert!(body.withdrawals.is_none());
        assert!(body.proposal_procedures.is_none());
        assert!(body.donation.is_none());
        assert!(body.sub_transactions.is_none());
        assert!(body.direct_deposits.is_none());
        assert!(body.account_balance_intervals.is_none());
        assert!(body.starting_account_balance_intervals.is_none());
        assert_eq!(body.inputs.len(), 1);
        assert_eq!(body.fee, case["fee"].as_u64().unwrap());
        let input_meta = &case["input"];
        assert_eq!(input_meta["era"], "Dijkstra");
        assert_eq!(
            body.inputs[0].transaction_id.to_string(),
            input_meta["transaction_id"].as_str().unwrap()
        );
        assert_eq!(
            body.inputs[0].index,
            input_meta["output_index"].as_u64().unwrap()
        );
        let input_bytes = read(root.join(input_meta["file"].as_str().unwrap()));
        let input = MultiEraOutput::decode(Era::Dijkstra, &input_bytes).unwrap();
        assert_eq!(input.era(), Era::Dijkstra);
        let certificates = tx.certs();
        assert_eq!(
            certificates.len() as u64,
            case["registration_count"].as_u64().unwrap()
        );
        assert!(
            certificates
                .iter()
                .all(|cert| matches!(cert.kind(), Some(MultiEraCertKind::Reg(..))))
        );
        let outputs = output_value(tx);
        let (deposits, refunds) =
            certificate_deposits_and_refunds(&certificates, key_deposit, 0).unwrap();
        assert_eq!(refunds, 0);
        let produced = conway_add_values(
            &outputs,
            &ConwayValue::Coin(body.fee + deposits),
            &ValidationError::PostAlonzo(PostAlonzoError::NegativeValue),
        )
        .unwrap();
        assert_eq!(
            produced,
            input.value().into_conway(),
            "captured registration must preserve value including its deposits"
        );
        assert_eq!(
            input.encode(),
            input_bytes,
            "original input must remain unchanged"
        );
    }

    fn output_value(tx: &MultiEraTx) -> ConwayValue {
        tx.outputs()
            .iter()
            .try_fold(ConwayValue::Coin(0), |total, output| {
                assert_eq!(output.era(), Era::Dijkstra);
                conway_add_values(
                    &total,
                    &output.value().into_conway(),
                    &ValidationError::PostAlonzo(PostAlonzoError::NegativeValue),
                )
            })
            .unwrap()
    }

    #[test]
    fn settings_native_dijkstra_registration_balance() {
        captured_registration("settings");
    }

    #[test]
    fn protocol_native_dijkstra_registration_balance() {
        captured_registration("protocol");
    }
}
