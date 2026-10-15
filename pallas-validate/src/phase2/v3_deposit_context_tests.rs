use super::{data::Data, script_context::*, to_plutus_data::*};
use pallas_primitives::conway::{
    Certificate, ExUnits, PlutusData, Redeemer, RedeemerTag, StakeCredential,
};

fn fields(data: &PlutusData) -> &[PlutusData] {
    match data {
        PlutusData::Constr(c) => &c.fields,
        _ => panic!("expected constructor: {data:?}"),
    }
}
fn context(cert: Certificate) -> ScriptContext<'static> {
    let r = Redeemer {
        tag: RedeemerTag::Cert,
        index: 2,
        data: Data::list(vec![]),
        ex_units: ExUnits {
            mem: 100,
            steps: 100,
        },
    };
    let purpose = ScriptPurpose::Certifying(2, cert.clone());
    TxInfo::V3(TxInfoV3 {
        inputs: vec![],
        reference_inputs: vec![],
        outputs: vec![],
        fee: 0,
        mint: MintValue {
            mint_value: Default::default(),
        },
        certificates: vec![cert],
        withdrawals: vec![].into(),
        valid_range: TimeRange {
            lower_bound: None,
            upper_bound: None,
        },
        signatories: vec![],
        redeemers: vec![(purpose, r.clone())].into(),
        data: vec![].into(),
        id: [0; 32].into(),
        votes: vec![].into(),
        proposal_procedures: vec![],
        current_treasury_amount: None,
        treasury_donation: None,
    })
    .into_script_context(&r, None)
    .unwrap()
}
#[test]
fn v3_deposit_context_protocol_boundaries_at_every_occurrence() {
    let credential = StakeCredential::ScriptHash([7; 28].into());
    for protocol in [9, 10, 11] {
        for (tag, cert) in [
            (0, Certificate::Reg(credential.clone(), 2000000)),
            (1, Certificate::UnReg(credential.clone(), 2000000)),
        ] {
            let c = context(cert);
            let encoded = c.to_plutus_data_with_protocol(protocol);
            let expected = Data::constr(
                tag,
                vec![
                    credential.to_plutus_data(),
                    if protocol == 9 {
                        None::<u64>.to_plutus_data()
                    } else {
                        Some(2000000u64).to_plutus_data()
                    },
                ],
            );
            let info = fields(&fields(&encoded)[0]);
            assert_eq!(
                info[5],
                Data::list(vec![expected.clone()]),
                "certificate list protocol {protocol}"
            );
            assert_eq!(
                fields(&fields(&encoded)[2])[1],
                expected,
                "active purpose protocol {protocol}"
            );
            let PlutusData::Map(m) = &info[9] else {
                panic!("redeemer map")
            };
            assert_eq!(
                fields(&m[0].0)[1],
                expected,
                "redeemer purpose protocol {protocol}"
            );
        }
    }
}
#[test]
fn public_encoding_is_the_protocol_9_encoding() {
    let credential = StakeCredential::ScriptHash([7; 28].into());
    for cert in [
        Certificate::Reg(credential.clone(), 2000000),
        Certificate::UnReg(credential.clone(), 2000000),
    ] {
        let c = context(cert);
        let ScriptContext::V3 { tx_info, .. } = &c else {
            panic!("V3 context")
        };
        assert_eq!(
            c.to_plutus_data(),
            c.to_plutus_data_with_protocol(9),
            "script context"
        );
        assert_eq!(
            tx_info.to_plutus_data(),
            tx_info.to_plutus_data_with_protocol(9),
            "tx info"
        );
    }
}
