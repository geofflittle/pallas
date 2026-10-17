
    #[cfg(feature = "phase2")]
    mod review_probe_phase2 {
        use super::*;
        use pallas_primitives::{
            BigInt, PlutusData, StakeCredential,
            conway::{Certificate, Redeemer, RedeemerTag, Redeemers, WitnessSet},
        };
        use pallas_traverse::ComputeHash;
        use pallas_validate::{
            phase2::{script_context::SlotConfig, tx::eval_tx},
            utils::UtxoMap,
        };

        #[test]
        fn review_probe_reg_redeemer_with_undecodable_withdrawal() {
            for bad in [false, true] {
                let cbor_bytes: Vec<u8> = cbor_to_bytes(include_str!("../../test_data/conway3.tx"));
                let mut mtx: Tx = conway_minted_tx_from_cbor(&cbor_bytes);
                let script = PlutusScript::<3>(Bytes::from(vec![0x41, 0x01]));
                let body = &mut mtx.transaction_body;
                body.inputs = Set::from(Vec::new());
                body.outputs = Vec::new();
                body.collateral = None;
                body.reference_inputs = None;
                body.certificates = vec![Certificate::Reg(
                    StakeCredential::ScriptHash(script.compute_hash()),
                    2_000_000,
                )]
                .try_into()
                .ok();
                let mut withdrawals = BTreeMap::new();
                let mut good = vec![0xe1u8];
                good.extend([3u8; 28]);
                withdrawals.insert(Bytes::from(good), 1u64);
                let second = if bad { vec![0xe1u8, 0x00] } else { [vec![0xe1u8], vec![4u8; 28]].concat() };
                withdrawals.insert(Bytes::from(second), 1u64);
                body.withdrawals = Some(withdrawals);
                let mut witnesses: WitnessSet = minicbor::decode(&[0xa0]).unwrap();
                witnesses.plutus_v3_script = Some(vec![script].try_into().unwrap());
                witnesses.redeemer = Some(
                    Redeemers::List(vec![Redeemer {
                        tag: RedeemerTag::Cert,
                        index: 0,
                        data: PlutusData::BigInt(BigInt::Int(0.into())),
                        ex_units: ExUnits {
                            mem: 1_000_000,
                            steps: 1_000_000_000,
                        },
                    }])
                    .into(),
                );
                mtx.transaction_witness_set = witnesses.into();
                let cbor = minicbor::to_vec(mtx).unwrap();
                let mtx: Tx = conway_minted_tx_from_cbor(&cbor);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    format!(
                        "{:?}",
                        eval_tx(
                            &MultiEraTx::from_conway(&mtx),
                            &MultiEraProtocolParameters::Conway(mk_mainnet_params_epoch_380()),
                            &UtxoMap::new(),
                            &SlotConfig::default(),
                        )
                        .map(|results| results.len())
                    )
                }));
                println!(
                    "PROBE bad_withdrawal={bad} {}",
                    outcome.unwrap_or_else(|_| "PANIC".to_string())
                );
            }
        }
    }
}
