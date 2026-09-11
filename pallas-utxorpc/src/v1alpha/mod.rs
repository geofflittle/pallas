use std::ops::Deref;

use prost_types::FieldMask;

use pallas_primitives::{babbage, conway};
use pallas_traverse as trv;
use trv::OriginalHash;

use crate::LedgerContext;

pub use utxorpc_spec::utxorpc::v1alpha as spec;

#[derive(Default, Clone)]
pub struct Mapper<C: LedgerContext> {
    pub(crate) ledger: Option<C>,
    pub(crate) _mask: FieldMask,
}

impl<C: LedgerContext> Mapper<C> {
    pub fn new(ledger: C) -> Self {
        Self {
            ledger: Some(ledger),
            _mask: FieldMask { paths: vec![] },
        }
    }

    /// Creates a clone of this mapper using a custom field mask
    pub fn masked(&self, mask: FieldMask) -> Self {
        Self {
            ledger: self.ledger.clone(),
            _mask: mask,
        }
    }
}

crate::shared::impl_cardano_mapper_shared!(utxorpc_spec::utxorpc::v1alpha::cardano);

// ---- v1alpha-specific bodies for methods that diverge from v1beta -----------

impl<C: LedgerContext> Mapper<C> {
    pub fn map_native_script(x: &pallas_primitives::alonzo::NativeScript) -> u5c::NativeScript {
        let inner = match x {
            babbage::NativeScript::ScriptPubkey(x) => {
                u5c::native_script::NativeScript::ScriptPubkey(x.to_vec().into())
            }
            babbage::NativeScript::ScriptAll(x) => {
                u5c::native_script::NativeScript::ScriptAll(u5c::NativeScriptList {
                    items: x.iter().map(|x| Self::map_native_script(x)).collect(),
                })
            }
            babbage::NativeScript::ScriptAny(x) => {
                u5c::native_script::NativeScript::ScriptAny(u5c::NativeScriptList {
                    items: x.iter().map(|x| Self::map_native_script(x)).collect(),
                })
            }
            babbage::NativeScript::ScriptNOfK(n, k) => {
                u5c::native_script::NativeScript::ScriptNOfK(u5c::ScriptNOfK {
                    k: *n,
                    scripts: k.iter().map(|x| Self::map_native_script(x)).collect(),
                })
            }
            babbage::NativeScript::InvalidBefore(s) => {
                u5c::native_script::NativeScript::InvalidBefore(*s)
            }
            babbage::NativeScript::InvalidHereafter(s) => {
                u5c::native_script::NativeScript::InvalidHereafter(*s)
            }
        };

        u5c::NativeScript {
            native_script: inner.into(),
        }
    }

    /// Dijkstra's native script type carries a seventh variant,
    /// `script_require_guard`, which the u5c `native_script` oneof has no
    /// field for. The six shared variants map exactly as they do for every
    /// earlier era, and the seventh leaves the oneof unset rather than being
    /// reported as one of the others.
    #[cfg(feature = "unstable")]
    pub fn map_dijkstra_native_script(
        x: &pallas_primitives::dijkstra::NativeScript,
    ) -> u5c::NativeScript {
        use pallas_primitives::dijkstra;

        let inner = match x {
            dijkstra::NativeScript::ScriptPubkey(x) => Some(
                u5c::native_script::NativeScript::ScriptPubkey(x.to_vec().into()),
            ),
            dijkstra::NativeScript::ScriptAll(x) => Some(
                u5c::native_script::NativeScript::ScriptAll(u5c::NativeScriptList {
                    items: x.iter().map(Self::map_dijkstra_native_script).collect(),
                }),
            ),
            dijkstra::NativeScript::ScriptAny(x) => Some(
                u5c::native_script::NativeScript::ScriptAny(u5c::NativeScriptList {
                    items: x.iter().map(Self::map_dijkstra_native_script).collect(),
                }),
            ),
            dijkstra::NativeScript::ScriptNOfK(n, k) => Some(
                u5c::native_script::NativeScript::ScriptNOfK(u5c::ScriptNOfK {
                    k: *n,
                    scripts: k.iter().map(Self::map_dijkstra_native_script).collect(),
                }),
            ),
            dijkstra::NativeScript::InvalidBefore(s) => {
                Some(u5c::native_script::NativeScript::InvalidBefore(*s))
            }
            dijkstra::NativeScript::InvalidHereafter(s) => {
                Some(u5c::native_script::NativeScript::InvalidHereafter(*s))
            }
            dijkstra::NativeScript::ScriptRequireGuard(_) => None,
        };

        u5c::NativeScript {
            native_script: inner,
        }
    }

    pub fn map_tx_datum(
        &self,
        x: &trv::MultiEraOutput,
        tx: Option<&trv::MultiEraTx>,
    ) -> u5c::Datum {
        u5c::Datum {
            hash: match x.datum() {
                Some(babbage::DatumOption::Data(x)) => x.original_hash().to_vec().into(),
                Some(babbage::DatumOption::Hash(x)) => x.to_vec().into(),
                _ => vec![].into(),
            },
            payload: match x.datum() {
                Some(babbage::DatumOption::Data(x)) => self.map_plutus_datum(&x.0).into(),
                Some(babbage::DatumOption::Hash(x)) => tx
                    .and_then(|tx| tx.find_plutus_data(&x))
                    .map(|d| self.map_plutus_datum(d)),
                _ => None,
            },
            original_cbor: match x.datum() {
                Some(babbage::DatumOption::Data(x)) => x.raw_cbor().to_vec().into(),
                _ => vec![].into(),
            },
        }
    }

    pub fn map_tx_output(
        &self,
        x: &trv::MultiEraOutput,
        tx: Option<&trv::MultiEraTx>,
    ) -> u5c::TxOutput {
        u5c::TxOutput {
            address: x.address().map(|a| a.to_vec()).unwrap_or_default().into(),
            coin: u64_to_bigint(x.value().coin()),
            // TODO: this is wrong, we're crating a new item for each asset even if they share
            // the same policy id. We need to adjust Pallas' interface to make this mapping more
            // ergonomic.
            assets: x
                .value()
                .assets()
                .iter()
                .map(|x| self.map_policy_assets(x))
                .collect(),
            datum: self.map_tx_datum(x, tx).into(),
            script: self.map_output_script(x),
        }
    }

    /// The output's reference script, through whichever accessor can hold the
    /// script types the build has eras for.
    #[cfg(feature = "unstable")]
    fn map_output_script(&self, x: &trv::MultiEraOutput) -> Option<u5c::Script> {
        x.any_script_ref().map(|x| self.map_script_ref(&x))
    }

    #[cfg(not(feature = "unstable"))]
    fn map_output_script(&self, x: &trv::MultiEraOutput) -> Option<u5c::Script> {
        x.script_ref().map(|x| self.map_any_script(&x))
    }

    pub fn map_asset(&self, x: &trv::MultiEraAsset) -> u5c::Asset {
        let quantity = if let Some(v) = x.output_coin() {
            u64_to_bigint(v).map(u5c::asset::Quantity::OutputCoin)
        } else if let Some(v) = x.mint_coin() {
            i64_to_bigint(v).map(u5c::asset::Quantity::MintCoin)
        } else {
            None
        };
        u5c::Asset {
            name: x.name().to_vec().into(),
            quantity,
        }
    }

    pub fn map_policy_assets(&self, x: &trv::MultiEraPolicyAssets) -> u5c::Multiasset {
        u5c::Multiasset {
            policy_id: x.policy().to_vec().into(),
            assets: x.assets().iter().map(|x| self.map_asset(x)).collect(),
            redeemer: None,
        }
    }

    pub fn map_conway_gov_action(&self, x: &conway::GovAction) -> u5c::GovernanceAction {
        let inner = match x {
            conway::GovAction::ParameterChange(gov_id, params, script) => {
                u5c::governance_action::GovernanceAction::ParameterChangeAction(
                    u5c::ParameterChangeAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        protocol_param_update: Some(self.map_conway_pparams_update(params)),
                        policy_hash: match script {
                            Some(x) => x.to_vec().into(),
                            _ => Default::default(),
                        },
                    },
                )
            }
            conway::GovAction::HardForkInitiation(gov_id, version) => {
                u5c::governance_action::GovernanceAction::HardForkInitiationAction(
                    u5c::HardForkInitiationAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        protocol_version: Some(u5c::ProtocolVersion {
                            major: version.0 as u32,
                            minor: version.1 as u32,
                        }),
                    },
                )
            }
            conway::GovAction::TreasuryWithdrawals(withdrawals, script) => {
                u5c::governance_action::GovernanceAction::TreasuryWithdrawalsAction(
                    u5c::TreasuryWithdrawalsAction {
                        withdrawals: withdrawals
                            .iter()
                            .map(|(k, v)| u5c::WithdrawalAmount {
                                reward_account: k.to_vec().into(),
                                coin: u64_to_bigint(*v),
                            })
                            .collect(),
                        policy_hash: match script {
                            Some(x) => x.to_vec().into(),
                            _ => Default::default(),
                        },
                    },
                )
            }
            conway::GovAction::NoConfidence(gov_id) => {
                u5c::governance_action::GovernanceAction::NoConfidenceAction(
                    u5c::NoConfidenceAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                    },
                )
            }
            conway::GovAction::UpdateCommittee(gov_id, remove, add, threshold) => {
                u5c::governance_action::GovernanceAction::UpdateCommitteeAction(
                    u5c::UpdateCommitteeAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        remove_committee_credentials: remove
                            .iter()
                            .map(|x| self.map_stake_credential(x))
                            .collect(),
                        new_committee_credentials: add
                            .iter()
                            .map(|(cred, epoch)| u5c::NewCommitteeCredentials {
                                committee_cold_credential: Some(self.map_stake_credential(cred)),
                                expires_epoch: *epoch as u32,
                            })
                            .collect(),
                        new_committee_threshold: Some(rational_number_to_u5c(threshold.clone())),
                    },
                )
            }
            conway::GovAction::NewConstitution(gov_id, constitution) => {
                u5c::governance_action::GovernanceAction::NewConstitutionAction(
                    u5c::NewConstitutionAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        constitution: Some(u5c::Constitution {
                            anchor: Some(u5c::Anchor {
                                url: constitution.anchor.url.clone(),
                                content_hash: constitution.anchor.content_hash.to_vec().into(),
                            }),
                            hash: match constitution.guardrail_script {
                                Some(x) => x.to_vec().into(),
                                _ => Default::default(),
                            },
                        }),
                    },
                )
            }
            conway::GovAction::Information => {
                // The 6 is just a placeholder; v1alpha encodes Information as a uint32.
                u5c::governance_action::GovernanceAction::InfoAction(6)
            }
        };

        u5c::GovernanceAction {
            governance_action: Some(inner),
        }
    }

    /// Map a Dijkstra governance action.
    ///
    /// Six of the seven arms carry the types Conway carries and map exactly
    /// as Conway's do. The seventh, `parameter_change_action`, carries this
    /// era's protocol parameter update, whose keys past 33 the schema has no
    /// field for.
    #[cfg(feature = "unstable")]
    pub fn map_dijkstra_gov_action(
        &self,
        x: &pallas_primitives::dijkstra::GovAction,
    ) -> u5c::GovernanceAction {
        use pallas_primitives::dijkstra;

        let inner = match x {
            dijkstra::GovAction::ParameterChange(gov_id, params, script) => {
                u5c::governance_action::GovernanceAction::ParameterChangeAction(
                    u5c::ParameterChangeAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        protocol_param_update: Some(self.map_dijkstra_pparams_update(params)),
                        policy_hash: match script {
                            Some(x) => x.to_vec().into(),
                            _ => Default::default(),
                        },
                    },
                )
            }
            dijkstra::GovAction::HardForkInitiation(gov_id, version) => {
                u5c::governance_action::GovernanceAction::HardForkInitiationAction(
                    u5c::HardForkInitiationAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        protocol_version: Some(u5c::ProtocolVersion {
                            major: version.0 as u32,
                            minor: version.1 as u32,
                        }),
                    },
                )
            }
            dijkstra::GovAction::TreasuryWithdrawals(withdrawals, script) => {
                u5c::governance_action::GovernanceAction::TreasuryWithdrawalsAction(
                    u5c::TreasuryWithdrawalsAction {
                        withdrawals: withdrawals
                            .iter()
                            .map(|(k, v)| u5c::WithdrawalAmount {
                                reward_account: k.to_vec().into(),
                                coin: u64_to_bigint(*v),
                            })
                            .collect(),
                        policy_hash: match script {
                            Some(x) => x.to_vec().into(),
                            _ => Default::default(),
                        },
                    },
                )
            }
            dijkstra::GovAction::NoConfidence(gov_id) => {
                u5c::governance_action::GovernanceAction::NoConfidenceAction(
                    u5c::NoConfidenceAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                    },
                )
            }
            dijkstra::GovAction::UpdateCommittee(gov_id, remove, add, threshold) => {
                u5c::governance_action::GovernanceAction::UpdateCommitteeAction(
                    u5c::UpdateCommitteeAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        remove_committee_credentials: remove
                            .iter()
                            .map(|x| self.map_stake_credential(x))
                            .collect(),
                        new_committee_credentials: add
                            .iter()
                            .map(|(cred, epoch)| u5c::NewCommitteeCredentials {
                                committee_cold_credential: Some(self.map_stake_credential(cred)),
                                expires_epoch: *epoch as u32,
                            })
                            .collect(),
                        new_committee_threshold: Some(rational_number_to_u5c(threshold.clone())),
                    },
                )
            }
            dijkstra::GovAction::NewConstitution(gov_id, constitution) => {
                u5c::governance_action::GovernanceAction::NewConstitutionAction(
                    u5c::NewConstitutionAction {
                        gov_action_id: self.map_gov_action_id(gov_id),
                        constitution: Some(u5c::Constitution {
                            anchor: Some(u5c::Anchor {
                                url: constitution.anchor.url.clone(),
                                content_hash: constitution.anchor.content_hash.to_vec().into(),
                            }),
                            hash: match constitution.guardrail_script {
                                Some(x) => x.to_vec().into(),
                                _ => Default::default(),
                            },
                        }),
                    },
                )
            }
            dijkstra::GovAction::Information => {
                // v1alpha encodes an info action as a uint32 and the 6 is a
                // placeholder, the same as on the Conway arm above.
                u5c::governance_action::GovernanceAction::InfoAction(6)
            }
        };

        u5c::GovernanceAction {
            governance_action: Some(inner),
        }
    }

    pub fn map_tx(&self, tx: &trv::MultiEraTx) -> u5c::Tx {
        let resolved = self.ledger.as_ref().and_then(|ctx| {
            let to_resolve = self.find_related_inputs(tx);
            ctx.get_utxos(to_resolve.as_slice())
        });

        u5c::Tx {
            hash: tx.hash().to_vec().into(),
            inputs: tx
                .inputs_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, i)| self.map_tx_input(i, tx, order as u32, &resolved))
                .collect(),
            outputs: tx
                .outputs()
                .iter()
                .map(|x| self.map_tx_output(x, Some(tx)))
                .collect(),
            certificates: tx
                .certs()
                .iter()
                .enumerate()
                .filter_map(|(order, x)| self.map_cert(x, tx, order as u32))
                .collect(),
            proposals: tx
                .gov_proposals()
                .iter()
                .map(|x| self.map_gov_proposal(x))
                .collect(),
            withdrawals: tx
                .withdrawals_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, x)| self.map_withdrawals(x, tx, order as u32))
                .collect(),
            mint: tx
                .mints_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, x)| {
                    let mut ma = self.map_policy_assets(x);

                    ma.redeemer = tx
                        .find_mint_redeemer(order as u32)
                        .map(|r| self.map_redeemer(&r));

                    ma
                })
                .collect(),
            reference_inputs: tx
                .reference_inputs()
                .iter()
                .map(|x| self.map_tx_reference_input(x, &resolved, tx))
                .collect(),
            witnesses: u5c::WitnessSet {
                vkeywitness: tx
                    .vkey_witnesses()
                    .iter()
                    .map(|x| self.map_vkey_witness(x))
                    .collect(),
                script: self.collect_all_scripts(tx),
                plutus_datums: tx
                    .plutus_data()
                    .iter()
                    .map(|x| self.map_plutus_datum(x.deref()))
                    .collect(),
            }
            .into(),
            collateral: u5c::Collateral {
                collateral: tx
                    .collateral()
                    .iter()
                    .map(|x| self.map_tx_collateral(x, &resolved, tx))
                    .collect(),
                collateral_return: tx
                    .collateral_return()
                    .map(|x| self.map_tx_output(&x, Some(tx))),
                total_collateral: u64_to_bigint(tx.total_collateral().unwrap_or_default()),
            }
            .into(),
            fee: u64_to_bigint(tx.fee().unwrap_or_default()),
            validity: u5c::TxValidity {
                start: tx.validity_start().unwrap_or_default(),
                ttl: tx.ttl().unwrap_or_default(),
            }
            .into(),
            successful: tx.is_valid(),
            auxiliary: u5c::AuxData {
                metadata: tx
                    .metadata()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|(l, d)| self.map_metadata(l, d))
                    .collect(),
                scripts: self.collect_all_aux_scripts(tx),
            }
            .into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TxoRef, UtxoMap};
    use pretty_assertions::assert_eq;

    #[derive(Clone)]
    struct NoLedger;

    impl LedgerContext for NoLedger {
        fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
            None
        }

        fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
            None
        }
    }

    #[cfg(feature = "unstable")]
    fn dijkstra_block(block_str: &str) -> pallas_traverse::MultiEraBlock<'_> {
        let cbor: &'static [u8] = Box::leak(hex::decode(block_str).unwrap().into_boxed_slice());
        pallas_traverse::MultiEraBlock::decode(cbor).unwrap()
    }

    #[cfg(feature = "unstable")]
    fn dijkstra_tx(tx_str: &str) -> pallas_traverse::MultiEraTx<'_> {
        let cbor: &'static [u8] = Box::leak(hex::decode(tx_str).unwrap().into_boxed_slice());
        pallas_traverse::MultiEraTx::decode_for_era(pallas_traverse::Era::Dijkstra, cbor).unwrap()
    }

    /// Every certificate a Dijkstra block carries reaches the u5c stream.
    ///
    /// `dijkstra6.block` is the busiest block on the chain, with five
    /// certificates across three of its four transactions: a stake
    /// delegation, two registrations and two pool registrations.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_maps_every_certificate() {
        let block = dijkstra_block(include_str!("../../../test_data/dijkstra6.block"));
        let mapper = Mapper::new(NoLedger);
        let mapped = mapper.map_block(&block);

        let certs: usize = mapped
            .body
            .as_ref()
            .unwrap()
            .tx
            .iter()
            .map(|t| t.certificates.len())
            .sum();

        assert_eq!(certs, 5, "every certificate in the block must be mapped");

        let named = mapped
            .body
            .as_ref()
            .unwrap()
            .tx
            .iter()
            .flat_map(|t| t.certificates.iter())
            .filter(|c| c.certificate.is_some())
            .count();
        assert_eq!(named, 5, "and each one must carry a certificate variant");
    }

    /// A block with no certificates maps none, so the count above is the block
    /// speaking rather than the mapper inventing certificates.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_with_no_certificates_maps_none() {
        let block = dijkstra_block(include_str!("../../../test_data/dijkstra3.block"));
        let mapper = Mapper::new(NoLedger);
        let mapped = mapper.map_block(&block);

        let certs: usize = mapped
            .body
            .as_ref()
            .unwrap()
            .tx
            .iter()
            .map(|t| t.certificates.len())
            .sum();
        assert_eq!(certs, 0);
    }

    /// A Dijkstra governance proposal reaches u5c with its action, not with
    /// the action field left unset beside a real deposit.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_proposal_maps_its_governance_action() {
        let tx = dijkstra_tx(include_str!("../../../test_data/dijkstra-proposal.tx"));
        let mapper = Mapper::new(NoLedger);
        let mapped = mapper.map_tx(&tx);

        assert_eq!(mapped.proposals.len(), 1);
        let proposal = &mapped.proposals[0];
        assert!(
            proposal.gov_action.is_some(),
            "a parameter change action must reach the schema"
        );
    }

    /// A Dijkstra transaction's scripts reach u5c: the native script in the
    /// witness set, the native and PlutusV4 scripts in the auxiliary data, and
    /// the PlutusV4 reference script on the output.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_transaction_maps_every_script_it_carries() {
        let tx = dijkstra_tx(include_str!("../../../test_data/dijkstra-scripts.tx"));
        let mapper = Mapper::new(NoLedger);
        let mapped = mapper.map_tx(&tx);

        assert_eq!(
            mapped.witnesses.as_ref().unwrap().script.len(),
            1,
            "the witness set native script must be mapped"
        );

        assert_eq!(
            mapped.auxiliary.as_ref().unwrap().scripts.len(),
            2,
            "the auxiliary data carries a native script and a V4 script"
        );

        let output = &mapped.outputs[0];
        let script = output
            .script
            .as_ref()
            .expect("the output carries a reference script");
        assert!(
            matches!(script.script, Some(u5c::script::Script::PlutusV4(_))),
            "a V4 reference script must reach the V4 field the schema has"
        );
    }

    #[test]
    fn snapshot() {
        #[allow(unused_mut)]
        let mut cases = vec![(
            include_str!("../../../test_data/u5c1.block"),
            include_str!("../../../test_data/u5c_v1alpha.json"),
            "u5c_v1alpha.json",
        )];

        #[cfg(feature = "unstable")]
        cases.push((
            include_str!("../../../test_data/dijkstra6.block"),
            include_str!("../../../test_data/u5c_v1alpha_dijkstra.json"),
            "u5c_v1alpha_dijkstra.json",
        ));

        // dijkstra6 is entirely legacy array outputs, so nothing above
        // carries a post Alonzo map output through the mapper. dijkstra10
        // carries five map outputs beside two array ones, with certificates
        // and auxiliary data in the same block.
        #[cfg(feature = "unstable")]
        cases.push((
            include_str!("../../../test_data/dijkstra10.block"),
            include_str!("../../../test_data/u5c_v1alpha_dijkstra_map_output.json"),
            "u5c_v1alpha_dijkstra_map_output.json",
        ));

        let mapper = Mapper::new(NoLedger);

        for (block_str, json_str, file) in cases {
            let cbor = hex::decode(block_str).unwrap();
            let block = pallas_traverse::MultiEraBlock::decode(&cbor).unwrap();
            let current = serde_json::json!(mapper.map_block(&block));

            // Set REGENERATE_SNAPSHOTS=1 to overwrite the snapshot file in place.
            if std::env::var("REGENERATE_SNAPSHOTS").is_ok() {
                let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../test_data")
                    .join(file);
                std::fs::write(&path, serde_json::to_string_pretty(&current).unwrap()).unwrap();
                eprintln!("regenerated {}", path.display());
                continue;
            }

            let expected: serde_json::Value = serde_json::from_str(json_str).unwrap();

            assert_eq!(expected, current, "{file}")
        }
    }
}
