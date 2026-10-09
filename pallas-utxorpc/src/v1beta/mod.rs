use std::ops::Deref;

use prost_types::FieldMask;

use pallas_primitives::{alonzo, babbage, conway};
use pallas_traverse as trv;
use trv::OriginalHash;

use crate::LedgerContext;

pub use utxorpc_spec::utxorpc::v1beta as spec;

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

crate::shared::impl_cardano_mapper_shared!(utxorpc_spec::utxorpc::v1beta::cardano);

/// Map the interval a reward account's balance must fall within.
#[cfg(feature = "unstable")]
fn map_account_balance_interval(
    (account, interval): (
        &pallas_primitives::RewardAccount,
        &pallas_primitives::dijkstra::AccountBalanceInterval,
    ),
) -> u5c::AccountBalanceInterval {
    use pallas_primitives::dijkstra::AccountBalanceInterval;
    use u5c::account_balance_interval::Interval;

    let range = |lower: Option<u64>, upper: Option<u64>| {
        Interval::Range(u5c::AccountBalanceRange {
            inclusive_lower_bound: lower.and_then(u64_to_bigint),
            exclusive_upper_bound: upper.and_then(u64_to_bigint),
        })
    };

    u5c::AccountBalanceInterval {
        reward_account: account.to_vec().into(),
        interval: Some(match interval {
            AccountBalanceInterval::LowerBound(lower) => range(Some(*lower), None),
            AccountBalanceInterval::Bounded(lower, upper) => range(Some(*lower), Some(*upper)),
            AccountBalanceInterval::UpperBound(upper) => range(None, Some(*upper)),
            AccountBalanceInterval::Exact(x) => {
                Interval::Exact(u64_to_bigint(*x).unwrap_or_default())
            }
        }),
    }
}

// ---- v1beta-specific bodies for methods that diverge from v1alpha -----------

impl<C: LedgerContext> Mapper<C> {
    // v1beta names this variant ScriptPubkeyHash; v1alpha names it
    // ScriptPubkey. The rest of map_native_script is identical between
    // versions and lives in shared.rs.
    fn map_native_script_pubkey(bytes: Vec<u8>) -> u5c::native_script::NativeScript {
        u5c::native_script::NativeScript::ScriptPubkeyHash(bytes.into())
    }

    /// Returns the purpose of a guarding redeemer.
    #[cfg(feature = "unstable")]
    fn map_guarding_purpose() -> u5c::RedeemerPurpose {
        u5c::RedeemerPurpose::Guarding
    }

    /// Returns the clause requiring the guard of a credential.
    #[cfg(feature = "unstable")]
    fn map_native_script_guard(
        credential: &pallas_primitives::StakeCredential,
    ) -> Option<u5c::native_script::NativeScript> {
        Some(u5c::native_script::NativeScript::ScriptRequireGuard(
            map_credential(credential),
        ))
    }

    /// Returns the parameters an update proposes for the keys only the
    /// Dijkstra era has.
    #[cfg(feature = "unstable")]
    fn map_dijkstra_param_update(x: &trv::MultiEraParamUpdate) -> u5c::PParams {
        u5c::PParams {
            max_ref_script_size_per_block: x
                .max_ref_script_size_per_block()
                .proposed()
                .unwrap_or_default(),
            max_ref_script_size_per_tx: x
                .max_ref_script_size_per_tx()
                .proposed()
                .unwrap_or_default(),
            ref_script_cost_stride: x.ref_script_cost_stride().proposed().unwrap_or_default(),
            ref_script_cost_multiplier: x
                .ref_script_cost_multiplier()
                .proposed()
                .map(rational_number_to_u5c),
            max_pledge_leverage: x
                .max_pledge_leverage()
                .proposed()
                .and_then(Option::from)
                .map(rational_number_to_u5c),
            min_pool_margin: x.min_pool_margin().proposed().map(rational_number_to_u5c),
            leios_announcement_period_length: x
                .leios_announcement_period_length()
                .proposed()
                .unwrap_or_default(),
            leios_vote_period_length: x.leios_vote_period_length().proposed().unwrap_or_default(),
            leios_diffusion_period_length: x
                .leios_diffusion_period_length()
                .proposed()
                .unwrap_or_default(),
            leios_committee_size: x.leios_committee_size().proposed().unwrap_or_default() as u32,
            leios_quorum_stake_threshold: x
                .leios_quorum_stake_threshold()
                .proposed()
                .map(rational_number_to_u5c),
            max_endorser_block_references_size: x
                .max_endorser_block_references_size()
                .proposed()
                .unwrap_or_default(),
            max_endorser_block_txs_size: x
                .max_endorser_block_txs_size()
                .proposed()
                .unwrap_or_default(),
            max_endorser_block_execution_units: x
                .max_endorser_block_execution_units()
                .proposed()
                .map(|u| u5c::ExUnits {
                    memory: u.mem,
                    steps: u.steps,
                }),
            max_ref_script_size_per_endorser_block: x
                .max_ref_script_size_per_endorser_block()
                .proposed()
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    /// Returns default parameters, since a build without the unstable feature
    /// reads no Dijkstra era.
    #[cfg(not(feature = "unstable"))]
    fn map_dijkstra_param_update(_x: &trv::MultiEraParamUpdate) -> u5c::PParams {
        u5c::PParams::default()
    }

    fn map_pool_registration(
        &self,
        pool: &trv::MultiEraPoolRegistration,
    ) -> u5c::PoolRegistrationCert {
        u5c::PoolRegistrationCert {
            operator: pool.operator.to_vec().into(),
            vrf_keyhash: pool.vrf_keyhash.to_vec().into(),
            pledge: u64_to_bigint(pool.pledge),
            cost: u64_to_bigint(pool.cost),
            margin: rational_number_to_u5c(pool.margin.clone()).into(),
            reward_account: pool.reward_account.to_vec().into(),
            pool_owners: pool.pool_owners.iter().map(|x| x.to_vec().into()).collect(),
            relays: pool.relays.iter().map(|x| self.map_relay(x)).collect(),
            pool_metadata: pool.pool_metadata.map(map_pool_metadata),
            bls_key: None,
        }
    }

    pub fn map_cert(
        &self,
        x: &trv::MultiEraCert,
        tx: &trv::MultiEraTx,
        order: u32,
    ) -> Option<u5c::Certificate> {
        let inner = match self.map_cert_kind(&x.kind()?) {
            #[cfg(feature = "unstable")]
            Some(u5c::certificate::Certificate::PoolRegistration(pool)) => Some(
                u5c::certificate::Certificate::PoolRegistration(u5c::PoolRegistrationCert {
                    bls_key: x.bls_key().map(|key| u5c::PoolBlsKey {
                        bls_pubkey: key.bls_pubkey.to_vec().into(),
                        bls_possession_proof: key.bls_possession_proof.to_vec().into(),
                    }),
                    ..pool
                }),
            ),
            inner => inner,
        };
        Some(self.certificate(inner, tx, order))
    }

    /// Returns the keys only a Dijkstra transaction body has, with each other
    /// field at its default.
    #[cfg(feature = "unstable")]
    fn map_dijkstra_tx_keys(&self, tx: &trv::MultiEraTx) -> u5c::Tx {
        u5c::Tx {
            sub_transactions: tx
                .sub_transactions()
                .iter()
                .map(|x| self.map_tx(x))
                .collect(),
            required_top_level_guards: tx
                .required_top_level_guards()
                .into_iter()
                .flatten()
                .map(|(credential, datum)| u5c::RequiredTopLevelGuard {
                    credential: Some(map_credential(credential)),
                    datum: match datum {
                        pallas_primitives::Nullable::Some(x) => Some(self.map_plutus_datum(x)),
                        _ => None,
                    },
                })
                .collect(),
            direct_deposits: tx
                .direct_deposits()
                .into_iter()
                .flatten()
                .map(|(account, coin)| u5c::DirectDeposit {
                    reward_account: account.to_vec().into(),
                    coin: u64_to_bigint(*coin),
                })
                .collect(),
            account_balance_intervals: tx
                .account_balance_intervals()
                .into_iter()
                .flatten()
                .map(map_account_balance_interval)
                .collect(),
            starting_account_balance_intervals: tx
                .starting_account_balance_intervals()
                .into_iter()
                .flatten()
                .map(map_account_balance_interval)
                .collect(),
            ..Default::default()
        }
    }

    /// Returns a default transaction, since a build without the unstable
    /// feature reads no Dijkstra era.
    #[cfg(not(feature = "unstable"))]
    fn map_dijkstra_tx_keys(&self, _tx: &trv::MultiEraTx) -> u5c::Tx {
        u5c::Tx::default()
    }

    /// Returns the credentials whose signature or script the transaction
    /// requires.
    fn map_guards(&self, tx: &trv::MultiEraTx) -> Vec<u5c::StakeCredential> {
        tx.required_signers()
            .collect_credentials::<Vec<_>>()
            .iter()
            .map(map_credential)
            .collect()
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
                Some(babbage::DatumOption::Data(x)) => Some(x.raw_cbor().to_vec().into()),
                _ => None,
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
            original_cbor: Some(x.encode().into()),
        }
    }

    fn map_output_script(&self, x: &trv::MultiEraOutput) -> Option<u5c::Script> {
        x.multi_era_script_ref()
            .map(|x| self.map_multi_era_script(&x))
    }

    pub fn map_asset(&self, x: &trv::MultiEraAsset) -> u5c::Asset {
        let quantity = if let Some(v) = x.output_coin() {
            u64_to_bigint(v)
        } else if let Some(v) = x.mint_coin() {
            i64_to_bigint(v)
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
        }
    }

    pub fn map_multi_era_gov_action(&self, x: &trv::MultiEraGovAction) -> u5c::GovernanceAction {
        let inner = match x.kind() {
            trv::MultiEraGovActionKind::ParameterChange(gov_id, params, script) => {
                u5c::governance_action::GovernanceAction::ParameterChangeAction(
                    u5c::ParameterChangeAction {
                        gov_action_id: self.map_gov_action_id(&gov_id.cloned()),
                        protocol_param_update: Some(self.map_multi_era_param_update(&params)),
                        policy_hash: match script {
                            Some(x) => x.to_vec().into(),
                            _ => Default::default(),
                        },
                    },
                )
            }
            trv::MultiEraGovActionKind::HardForkInitiation(gov_id, version) => {
                u5c::governance_action::GovernanceAction::HardForkInitiationAction(
                    u5c::HardForkInitiationAction {
                        gov_action_id: self.map_gov_action_id(&gov_id.cloned()),
                        protocol_version: Some(u5c::ProtocolVersion {
                            major: version.0 as u32,
                            minor: version.1 as u32,
                        }),
                    },
                )
            }
            trv::MultiEraGovActionKind::TreasuryWithdrawals(withdrawals, script) => {
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
            trv::MultiEraGovActionKind::NoConfidence(gov_id) => {
                u5c::governance_action::GovernanceAction::NoConfidenceAction(
                    u5c::NoConfidenceAction {
                        gov_action_id: self.map_gov_action_id(&gov_id.cloned()),
                    },
                )
            }
            trv::MultiEraGovActionKind::UpdateCommittee(gov_id, remove, add, threshold) => {
                u5c::governance_action::GovernanceAction::UpdateCommitteeAction(
                    u5c::UpdateCommitteeAction {
                        gov_action_id: self.map_gov_action_id(&gov_id.cloned()),
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
            trv::MultiEraGovActionKind::NewConstitution(gov_id, constitution) => {
                u5c::governance_action::GovernanceAction::NewConstitutionAction(
                    u5c::NewConstitutionAction {
                        gov_action_id: self.map_gov_action_id(&gov_id.cloned()),
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
            trv::MultiEraGovActionKind::Information => {
                u5c::governance_action::GovernanceAction::InfoAction(u5c::InfoAction {})
            }
            _ => return u5c::GovernanceAction::default(),
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
            votes: self.map_votes(tx),
            withdrawals: tx
                .withdrawals_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, x)| self.map_withdrawals(x, tx, order as u32))
                .collect(),
            mint: tx
                .mints_sorted_set()
                .iter()
                .map(|x| self.map_policy_assets(x))
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
                redeemers: tx
                    .redeemers()
                    .iter()
                    .map(|x| self.map_redeemer(x))
                    .collect(),
                bootstrap_witnesses: tx
                    .bootstrap_witnesses()
                    .iter()
                    .map(|x| self.map_bootstrap_witness(x))
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
            guards: self.map_guards(tx),
            ..self.map_dijkstra_tx_keys(tx)
        }
    }

    // ---- v1beta-only types (no v1alpha counterpart) -------------------------

    pub fn map_bootstrap_witness(&self, x: &alonzo::BootstrapWitness) -> u5c::BootstrapWitness {
        u5c::BootstrapWitness {
            vkey: x.public_key.to_vec().into(),
            signature: x.signature.to_vec().into(),
            chain_code: x.chain_code.to_vec().into(),
            attributes: x.attributes.to_vec().into(),
        }
    }

    pub fn map_vote(&self, x: &conway::Vote) -> u5c::Vote {
        match x {
            conway::Vote::No => u5c::Vote::No,
            conway::Vote::Yes => u5c::Vote::Yes,
            conway::Vote::Abstain => u5c::Vote::Abstain,
        }
    }

    pub fn map_voting_procedure(
        &self,
        gov_action_id: &conway::GovActionId,
        x: &conway::VotingProcedure,
    ) -> u5c::VotingProcedure {
        u5c::VotingProcedure {
            gov_action_id: Some(u5c::GovernanceActionId {
                transaction_id: gov_action_id.transaction_id.to_vec().into(),
                governance_action_index: gov_action_id.action_index,
            }),
            vote: self.map_vote(&x.vote) as i32,
            anchor: x.anchor.as_ref().map(|a| u5c::Anchor {
                url: a.url.clone(),
                content_hash: a.content_hash.to_vec().into(),
            }),
        }
    }

    fn map_voter(&self, voter: &conway::Voter) -> u5c::voter_votes::Voter {
        match voter {
            conway::Voter::ConstitutionalCommitteeKey(hash) => {
                u5c::voter_votes::Voter::ConstitutionalCommittee(u5c::StakeCredential {
                    stake_credential: u5c::stake_credential::StakeCredential::AddrKeyHash(
                        hash.to_vec().into(),
                    )
                    .into(),
                })
            }
            conway::Voter::ConstitutionalCommitteeScript(hash) => {
                u5c::voter_votes::Voter::ConstitutionalCommittee(u5c::StakeCredential {
                    stake_credential: u5c::stake_credential::StakeCredential::ScriptHash(
                        hash.to_vec().into(),
                    )
                    .into(),
                })
            }
            conway::Voter::DRepKey(hash) => u5c::voter_votes::Voter::Drep(u5c::StakeCredential {
                stake_credential: u5c::stake_credential::StakeCredential::AddrKeyHash(
                    hash.to_vec().into(),
                )
                .into(),
            }),
            conway::Voter::DRepScript(hash) => {
                u5c::voter_votes::Voter::Drep(u5c::StakeCredential {
                    stake_credential: u5c::stake_credential::StakeCredential::ScriptHash(
                        hash.to_vec().into(),
                    )
                    .into(),
                })
            }
            conway::Voter::StakePoolKey(hash) => u5c::voter_votes::Voter::Spo(hash.to_vec().into()),
        }
    }

    /// Maps the per-tx voting procedures into the v1beta `votes` field.
    pub fn map_votes(&self, tx: &trv::MultiEraTx) -> Vec<u5c::VoterVotes> {
        let Some(procedures) = tx.voting_procedures() else {
            return Vec::new();
        };

        procedures
            .iter()
            .map(|(voter, ballots)| u5c::VoterVotes {
                voter: Some(self.map_voter(voter)),
                votes: ballots
                    .iter()
                    .map(|(gov_id, procedure)| self.map_voting_procedure(gov_id, procedure))
                    .collect(),
            })
            .collect()
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

    #[test]
    fn snapshot() {
        let test_blocks = [include_str!("../../../test_data/u5c1.block")];
        let test_snapshots = [include_str!("../../../test_data/u5c_v1beta.json")];

        let mapper = Mapper::new(NoLedger);

        for (block_str, json_str) in test_blocks.iter().zip(test_snapshots) {
            let cbor = hex::decode(block_str).unwrap();
            let block = pallas_traverse::MultiEraBlock::decode(&cbor).unwrap();
            let current = serde_json::json!(mapper.map_block(&block));

            // Set REGENERATE_SNAPSHOTS=1 to overwrite the snapshot file in place.
            if std::env::var("REGENERATE_SNAPSHOTS").is_ok() {
                let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../test_data/u5c_v1beta.json");
                std::fs::write(&path, serde_json::to_string_pretty(&current).unwrap()).unwrap();
                eprintln!("regenerated {}", path.display());
                continue;
            }

            let expected: serde_json::Value = serde_json::from_str(json_str).unwrap();

            assert_eq!(expected, current)
        }
    }

    #[test]
    fn negative_n_of_k_threshold_maps_to_zero() {
        let mapped = Mapper::<NoLedger>::map_native_script(
            &pallas_primitives::alonzo::NativeScript::ScriptNOfK(-1, vec![]),
        );
        assert!(matches!(
            mapped.native_script,
            Some(u5c::native_script::NativeScript::ScriptNOfK(
                u5c::ScriptNOfK { k: 0, .. }
            ))
        ));
    }

    #[test]
    #[allow(deprecated)]
    fn the_legacy_purpose_mapper_agrees_with_the_multi_era_one() {
        use pallas_primitives::conway::RedeemerTag;
        use pallas_traverse::MultiEraRedeemerTag;

        let mapper = Mapper::new(NoLedger);
        let tags = [
            RedeemerTag::Spend,
            RedeemerTag::Mint,
            RedeemerTag::Cert,
            RedeemerTag::Reward,
            RedeemerTag::Vote,
            RedeemerTag::Propose,
        ];

        let mut seen = Vec::new();
        for tag in tags {
            let legacy = mapper.map_purpose(&tag);
            assert_eq!(
                legacy,
                mapper.map_multi_era_purpose(&MultiEraRedeemerTag::from(tag))
            );
            seen.push(legacy);
        }

        seen.dedup();
        assert_eq!(seen.len(), 6, "each tag maps to a purpose of its own");
    }

    #[test]
    fn oversized_n_of_k_threshold_maps_to_u32_max() {
        let mapped = Mapper::<NoLedger>::map_native_script(
            &pallas_primitives::alonzo::NativeScript::ScriptNOfK(i64::MAX, vec![]),
        );
        assert!(matches!(
            mapped.native_script,
            Some(u5c::native_script::NativeScript::ScriptNOfK(
                u5c::ScriptNOfK { k: u32::MAX, .. }
            ))
        ));
    }

    #[test]
    fn map_native_script_handles_deeply_nested_scripts_on_a_small_stack() {
        // Depth and stack size are load-bearing, not just generous: both must
        // stay far enough apart that the old recursive mapping (one call
        // frame per level) would abort here, or this test stops proving
        // anything the moment either constant drifts.
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut script =
                    pallas_primitives::alonzo::NativeScript::ScriptPubkey([0; 28].into());
                for _ in 0..20_000 {
                    script = pallas_primitives::alonzo::NativeScript::ScriptAll(vec![script]);
                }

                let mapped = Mapper::<NoLedger>::map_native_script(&script);

                let mut depth = 0;
                let mut cursor = &mapped;
                while let Some(u5c::native_script::NativeScript::ScriptAll(list)) =
                    &cursor.native_script
                {
                    cursor = list.items.first().expect("ScriptAll must carry a child");
                    depth += 1;
                }
                assert_eq!(depth, 20_000);
                assert!(matches!(
                    cursor.native_script,
                    Some(u5c::native_script::NativeScript::ScriptPubkeyHash(_))
                ));

                // u5c's generated type has no custom Drop (unlike the source
                // NativeScript, stack-safe since pallas#802), so dropping a
                // chain this deep would abort the same way the unfixed mapping
                // did. Leak it deliberately: this test is only about the
                // mapping, and the leak is a few MB, thread-local, test-only.
                std::mem::forget(mapped);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn map_native_script_preserves_mixed_shape_trees() {
        use pallas_primitives::alonzo::NativeScript;

        // Width > 1 at more than one level: the iterative rewrite pairs
        // mapped children with source children positionally, so a bug there
        // would only show up once a node has more than one child.
        let script = NativeScript::ScriptNOfK(
            2,
            vec![
                NativeScript::ScriptPubkey([1; 28].into()),
                NativeScript::ScriptAll(vec![
                    NativeScript::ScriptPubkey([2; 28].into()),
                    NativeScript::ScriptAny(vec![
                        NativeScript::InvalidBefore(100),
                        NativeScript::InvalidHereafter(200),
                    ]),
                ]),
                NativeScript::ScriptPubkey([3; 28].into()),
            ],
        );

        let mapped = Mapper::<NoLedger>::map_native_script(&script);
        let Some(u5c::native_script::NativeScript::ScriptNOfK(n_of_k)) = &mapped.native_script
        else {
            panic!("expected ScriptNOfK, got {:?}", mapped.native_script);
        };
        assert_eq!(n_of_k.k, 2);
        assert_eq!(n_of_k.scripts.len(), 3);

        assert!(matches!(
            n_of_k.scripts[0].native_script,
            Some(u5c::native_script::NativeScript::ScriptPubkeyHash(ref b)) if b.as_ref() == [1; 28]
        ));
        assert!(matches!(
            n_of_k.scripts[2].native_script,
            Some(u5c::native_script::NativeScript::ScriptPubkeyHash(ref b)) if b.as_ref() == [3; 28]
        ));

        let Some(u5c::native_script::NativeScript::ScriptAll(all)) =
            &n_of_k.scripts[1].native_script
        else {
            panic!(
                "expected ScriptAll, got {:?}",
                n_of_k.scripts[1].native_script
            );
        };
        assert_eq!(all.items.len(), 2);
        assert!(matches!(
            all.items[0].native_script,
            Some(u5c::native_script::NativeScript::ScriptPubkeyHash(ref b)) if b.as_ref() == [2; 28]
        ));

        let Some(u5c::native_script::NativeScript::ScriptAny(any)) = &all.items[1].native_script
        else {
            panic!("expected ScriptAny, got {:?}", all.items[1].native_script);
        };
        assert_eq!(any.items.len(), 2);
        assert!(matches!(
            any.items[0].native_script,
            Some(u5c::native_script::NativeScript::InvalidBefore(100))
        ));
        assert!(matches!(
            any.items[1].native_script,
            Some(u5c::native_script::NativeScript::InvalidHereafter(200))
        ));
    }

    #[cfg(feature = "unstable")]
    fn fixture(name: &str) -> Vec<u8> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../test_data")
            .join(name);
        let hex_str = std::fs::read_to_string(&path).expect("the fixture is readable");
        hex::decode(hex_str.trim()).expect("the fixture holds hex")
    }

    fn key_credential(hex_hash: &str) -> u5c::StakeCredential {
        u5c::StakeCredential {
            stake_credential: Some(u5c::stake_credential::StakeCredential::AddrKeyHash(
                hex::decode(hex_hash).unwrap().into(),
            )),
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_guarding_redeemer_maps_to_the_guarding_purpose() {
        use pallas_primitives::dijkstra;

        let key = dijkstra::RedeemersKey {
            tag: dijkstra::RedeemerTag::Guarding,
            index: 0,
        };
        let value = dijkstra::RedeemersValue {
            data: pallas_primitives::PlutusData::BoundedBytes(vec![0x0f].into()),
            ex_units: pallas_primitives::ExUnits { mem: 1, steps: 2 },
        };

        let mapped =
            Mapper::new(NoLedger).map_redeemer(&trv::MultiEraRedeemer::from_dijkstra(&key, &value));

        assert_eq!(mapped.purpose, u5c::RedeemerPurpose::Guarding as i32);
    }

    #[test]
    fn guards_map_the_required_signers_of_an_alonzo_transaction() {
        let cbor = hex::decode(include_str!("../../../test_data/alonzo2.tx").trim()).unwrap();
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Alonzo, &cbor).unwrap();

        assert_eq!(
            Mapper::new(NoLedger).map_tx(&tx).guards,
            vec![key_credential(
                "c81ffcbc08ff49965d74f90c391541ff1cc2b043ffe41c81d840be87"
            )]
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_sub_transaction_maps_as_a_transaction_nested_in_its_parent() {
        let cbor = fixture("dijkstra-subtx.tx");
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        let mapped = Mapper::new(NoLedger).map_tx(&tx);

        assert_eq!(
            mapped.hash.to_vec(),
            hex::decode("74e2116ca6e0c809f156aa062a9d4ee8b156322618846f1bdea5d9ad7a206a12")
                .unwrap()
        );
        assert_eq!(mapped.inputs.len(), 1);
        assert_eq!(
            mapped.inputs[0].tx_hash.to_vec(),
            hex::decode("737549420add1f19d144809aa02e2ae68a89b1761368e3294fbb30ee1467e3e4")
                .unwrap()
        );
        assert_eq!(mapped.sub_transactions.len(), 1);

        let sub = &mapped.sub_transactions[0];
        assert_eq!(
            sub.hash.to_vec(),
            hex::decode("63ac433eefaed71cf0fa2c7134f82ea38f3c6c338b3d5129c686c47b44162691")
                .unwrap()
        );
        assert_eq!(
            sub.inputs
                .iter()
                .map(|x| (x.tx_hash.to_vec(), x.output_index))
                .collect::<Vec<_>>(),
            vec![(
                hex::decode("2ed1285cced47acb5f08502843d980d7231665af9d90ef33fe6751e0f9e0b171")
                    .unwrap(),
                0
            )]
        );
        assert_eq!(sub.outputs.len(), 1);
        assert_eq!(sub.outputs[0].coin, u64_to_bigint(3_000_000));
        assert_eq!(
            sub.guards,
            vec![key_credential(
                "81aa16380175bef2da792c64e694d92e76f0b6c56b333499ea2662df"
            )]
        );

        let redeemers = &sub.witnesses.as_ref().unwrap().redeemers;
        assert_eq!(redeemers.len(), 1);
        assert_eq!(redeemers[0].purpose, u5c::RedeemerPurpose::Spend as i32);
        assert_eq!(
            redeemers[0].ex_units,
            Some(u5c::ExUnits {
                memory: 1_000_000,
                steps: 500_000_000,
            })
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_sub_transaction_maps_the_verdict_on_its_parent() {
        let cbor = fixture("dijkstra-subtx.tx");
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();
        let parent = tx.as_dijkstra().unwrap();

        for success in [true, false] {
            let mut flagged = parent.clone();
            flagged.success = success;

            let mapped = Mapper::new(NoLedger).map_tx(&trv::MultiEraTx::from_dijkstra(&flagged));

            assert_eq!(
                (mapped.successful, mapped.sub_transactions[0].successful),
                (success, success)
            );
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn guards_map_the_key_and_script_credentials_of_a_dijkstra_body() {
        use pallas_codec::minicbor::{Encoder, data::Tag};

        // A body of one input, no outputs, a fee, and key 14 with a key and a script credential.
        let mut e = Encoder::new(Vec::new());
        e.array(4).unwrap();
        e.map(4).unwrap();
        e.u8(0).unwrap();
        e.tag(Tag::new(258)).unwrap();
        e.array(1).unwrap();
        e.array(2).unwrap();
        e.bytes(&[0x11; 32]).unwrap();
        e.u8(0).unwrap();
        e.u8(1).unwrap();
        e.array(0).unwrap();
        e.u8(2).unwrap();
        e.u32(1_000).unwrap();
        e.u8(14).unwrap();
        e.tag(Tag::new(258)).unwrap();
        e.array(2).unwrap();
        e.array(2).unwrap();
        e.u8(0).unwrap();
        e.bytes(&[0x61; 28]).unwrap();
        e.array(2).unwrap();
        e.u8(1).unwrap();
        e.bytes(&[0x62; 28]).unwrap();
        e.map(0).unwrap();
        e.null().unwrap();
        e.bool(true).unwrap();

        let cbor = e.into_writer();
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        assert_eq!(
            Mapper::new(NoLedger).map_tx(&tx).guards,
            vec![
                key_credential(&"61".repeat(28)),
                u5c::StakeCredential {
                    stake_credential: Some(u5c::stake_credential::StakeCredential::ScriptHash(
                        vec![0x62; 28].into()
                    )),
                },
            ]
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_witness_guard_maps_to_the_script_require_guard_clause() {
        let cbor = fixture("dijkstra-scripts.tx");
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        let scripts = Mapper::new(NoLedger).collect_all_scripts(&tx);

        assert_eq!(
            scripts,
            vec![u5c::Script {
                script: Some(u5c::script::Script::Native(u5c::NativeScript {
                    native_script: Some(u5c::native_script::NativeScript::ScriptRequireGuard(
                        key_credential(&"7a".repeat(28))
                    )),
                })),
            }]
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_pool_registration_maps_its_bls_key() {
        let cbor = fixture("dijkstra2.block");
        let block = trv::MultiEraBlock::decode(&cbor).unwrap();
        let txs = block.txs();
        let certs = txs[0].certs();

        let mapped = Mapper::new(NoLedger)
            .map_cert(&certs[0], &txs[0], 0)
            .and_then(|x| x.certificate);

        let Some(u5c::certificate::Certificate::PoolRegistration(pool)) = mapped else {
            panic!("the block's certificate is a pool registration");
        };
        assert_eq!(
            pool.operator.to_vec(),
            hex::decode("a90f00d409d055d7eb3472b430618eda74a906af92e2fa177e56ccea").unwrap()
        );
        assert_eq!(
            pool.bls_key,
            Some(u5c::PoolBlsKey {
                bls_pubkey: hex::decode(concat!(
                    "b0d04d6492c59fa7aae9354078c77adc8ba04db59982fe1a842b0356ac720846",
                    "ea098aa1e93972c027ae10c8b91f30e30c541a6675e2feeeb177c60f6a67159e",
                    "b5751666ea5875a4983bfea53fed819958bc530cc0ad884eeaf0b85e8f3a46a5",
                ))
                .unwrap()
                .into(),
                bls_possession_proof: hex::decode(concat!(
                    "a9964d2780f1fb6f7ba8553a89bfc4ddf831021ec051744b",
                    "13dddfea412c7bd7314d65c1f276e22c114c523645af1854",
                ))
                .unwrap()
                .into(),
            })
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_proposal_maps_its_endorser_block_reference_script_limit() {
        let cbor = fixture("dijkstra-proposal.tx");
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        let mapped = Mapper::new(NoLedger).map_gov_proposal(&tx.gov_proposals()[0]);

        let Some(u5c::governance_action::GovernanceAction::ParameterChangeAction(change)) =
            mapped.gov_action.and_then(|x| x.governance_action)
        else {
            panic!("the proposal carries a parameter change");
        };
        assert_eq!(
            change.protocol_param_update,
            Some(u5c::PParams {
                max_ref_script_size_per_endorser_block: 20_000,
                ..Default::default()
            })
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_parameter_change_maps_every_key_only_the_era_has() {
        use pallas_codec::minicbor::{Encoder, data::Tag};

        // [0, null, {34: 1, ..., 48: 15}, null], with rationals under tag 30.
        let mut e = Encoder::new(Vec::new());
        e.array(4).unwrap();
        e.u8(0).unwrap();
        e.null().unwrap();
        e.map(15).unwrap();
        for (key, value) in [(34, 1), (35, 2), (36, 3)] {
            e.u8(key).unwrap();
            e.u8(value).unwrap();
        }
        for (key, numerator) in [(37, 4), (38, 5), (39, 6)] {
            e.u8(key).unwrap();
            e.tag(Tag::new(30)).unwrap();
            e.array(2).unwrap();
            e.u8(numerator).unwrap();
            e.u8(100).unwrap();
        }
        for (key, value) in [(40, 7), (41, 8), (42, 9), (43, 10)] {
            e.u8(key).unwrap();
            e.u8(value).unwrap();
        }
        e.u8(44).unwrap();
        e.tag(Tag::new(30)).unwrap();
        e.array(2).unwrap();
        e.u8(11).unwrap();
        e.u8(100).unwrap();
        for (key, value) in [(45, 12), (46, 13)] {
            e.u8(key).unwrap();
            e.u8(value).unwrap();
        }
        e.u8(47).unwrap();
        e.array(2).unwrap();
        e.u8(14).unwrap();
        e.u8(140).unwrap();
        e.u8(48).unwrap();
        e.u8(15).unwrap();
        e.null().unwrap();

        let cbor = e.into_writer();
        let action: pallas_primitives::dijkstra::GovAction =
            pallas_codec::minicbor::decode(&cbor).unwrap();

        let mapped = Mapper::new(NoLedger)
            .map_multi_era_gov_action(&trv::MultiEraGovAction::from_dijkstra(&action));

        let Some(u5c::governance_action::GovernanceAction::ParameterChangeAction(change)) =
            mapped.governance_action
        else {
            panic!("a parameter change maps to a parameter change action");
        };
        let percent = |numerator| {
            Some(u5c::RationalNumber {
                numerator,
                denominator: 100,
            })
        };
        assert_eq!(
            change.protocol_param_update,
            Some(u5c::PParams {
                max_ref_script_size_per_block: 1,
                max_ref_script_size_per_tx: 2,
                ref_script_cost_stride: 3,
                ref_script_cost_multiplier: percent(4),
                max_pledge_leverage: percent(5),
                min_pool_margin: percent(6),
                leios_announcement_period_length: 7,
                leios_vote_period_length: 8,
                leios_diffusion_period_length: 9,
                leios_committee_size: 10,
                leios_quorum_stake_threshold: percent(11),
                max_endorser_block_references_size: 12,
                max_endorser_block_txs_size: 13,
                max_endorser_block_execution_units: Some(u5c::ExUnits {
                    memory: 14,
                    steps: 140,
                }),
                max_ref_script_size_per_endorser_block: 15,
                ..Default::default()
            })
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn the_account_keys_of_a_dijkstra_body_map_one_message_per_entry() {
        use pallas_codec::minicbor::{Encoder, data::Tag};

        let account = |byte: u8| {
            let mut bytes = vec![0xe0];
            bytes.extend([byte; 28]);
            bytes
        };

        // A body of one input, no outputs, a fee and keys 24 to 27.
        let mut e = Encoder::new(Vec::new());
        e.array(4).unwrap();
        e.map(7).unwrap();
        e.u8(0).unwrap();
        e.tag(Tag::new(258)).unwrap();
        e.array(1).unwrap();
        e.array(2).unwrap();
        e.bytes(&[0x11; 32]).unwrap();
        e.u8(0).unwrap();
        e.u8(1).unwrap();
        e.array(0).unwrap();
        e.u8(2).unwrap();
        e.u32(1_000).unwrap();

        // A key credential without a datum, and a script credential with one.
        e.u8(24).unwrap();
        e.map(2).unwrap();
        e.array(2).unwrap();
        e.u8(0).unwrap();
        e.bytes(&[0x21; 28]).unwrap();
        e.null().unwrap();
        e.array(2).unwrap();
        e.u8(1).unwrap();
        e.bytes(&[0x22; 28]).unwrap();
        e.u8(7).unwrap();

        e.u8(25).unwrap();
        e.map(1).unwrap();
        e.bytes(&account(0x31)).unwrap();
        e.u32(5_000_000).unwrap();

        // One interval of each of the four shapes.
        e.u8(26).unwrap();
        e.map(4).unwrap();
        e.bytes(&account(0x41)).unwrap();
        e.array(2).unwrap();
        e.u8(10).unwrap();
        e.null().unwrap();
        e.bytes(&account(0x42)).unwrap();
        e.array(2).unwrap();
        e.u8(10).unwrap();
        e.u8(20).unwrap();
        e.bytes(&account(0x43)).unwrap();
        e.array(2).unwrap();
        e.null().unwrap();
        e.u8(20).unwrap();
        e.bytes(&account(0x44)).unwrap();
        e.u8(30).unwrap();

        e.u8(27).unwrap();
        e.map(1).unwrap();
        e.bytes(&account(0x51)).unwrap();
        e.u8(40).unwrap();

        e.map(0).unwrap();
        e.null().unwrap();
        e.bool(true).unwrap();

        let cbor = e.into_writer();
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        let mapped = Mapper::new(NoLedger).map_tx(&tx);

        assert_eq!(
            mapped.required_top_level_guards,
            vec![
                u5c::RequiredTopLevelGuard {
                    credential: Some(u5c::StakeCredential {
                        stake_credential: Some(u5c::stake_credential::StakeCredential::ScriptHash(
                            vec![0x22; 28].into()
                        )),
                    }),
                    datum: Some(u5c::PlutusData {
                        plutus_data: Some(u5c::plutus_data::PlutusData::BigInt(u5c::BigInt {
                            big_int: Some(u5c::big_int::BigInt::Int(7)),
                        })),
                    }),
                },
                u5c::RequiredTopLevelGuard {
                    credential: Some(key_credential(&"21".repeat(28))),
                    datum: None,
                },
            ]
        );
        assert_eq!(
            mapped.direct_deposits,
            vec![u5c::DirectDeposit {
                reward_account: account(0x31).into(),
                coin: u64_to_bigint(5_000_000),
            }]
        );

        let range = |lower: Option<u64>, upper: Option<u64>| {
            Some(u5c::account_balance_interval::Interval::Range(
                u5c::AccountBalanceRange {
                    inclusive_lower_bound: lower.and_then(u64_to_bigint),
                    exclusive_upper_bound: upper.and_then(u64_to_bigint),
                },
            ))
        };
        assert_eq!(
            mapped.account_balance_intervals,
            vec![
                u5c::AccountBalanceInterval {
                    reward_account: account(0x41).into(),
                    interval: range(Some(10), None),
                },
                u5c::AccountBalanceInterval {
                    reward_account: account(0x42).into(),
                    interval: range(Some(10), Some(20)),
                },
                u5c::AccountBalanceInterval {
                    reward_account: account(0x43).into(),
                    interval: range(None, Some(20)),
                },
                u5c::AccountBalanceInterval {
                    reward_account: account(0x44).into(),
                    interval: Some(u5c::account_balance_interval::Interval::Exact(
                        u64_to_bigint(30).unwrap()
                    )),
                },
            ]
        );
        assert_eq!(
            mapped.starting_account_balance_intervals,
            vec![u5c::AccountBalanceInterval {
                reward_account: account(0x51).into(),
                interval: Some(u5c::account_balance_interval::Interval::Exact(
                    u64_to_bigint(40).unwrap()
                )),
            }]
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_transaction_with_one_vote_maps_it() {
        use pallas_codec::minicbor::{Encoder, data::Tag};

        let mut e = Encoder::new(Vec::new());
        e.array(4).unwrap();

        // A body of one input, no outputs, a fee and the voting procedures.
        e.map(4).unwrap();
        e.u8(0).unwrap();
        e.tag(Tag::new(258)).unwrap();
        e.array(1).unwrap();
        e.array(2).unwrap();
        e.bytes(&[0x11; 32]).unwrap();
        e.u8(0).unwrap();
        e.u8(1).unwrap();
        e.array(0).unwrap();
        e.u8(2).unwrap();
        e.u32(1_000).unwrap();

        // A DRep key voter casting a yes vote with an anchor on action 3.
        e.u8(19).unwrap();
        e.map(1).unwrap();
        e.array(2).unwrap();
        e.u8(2).unwrap();
        e.bytes(&[0x77; 28]).unwrap();
        e.map(1).unwrap();
        e.array(2).unwrap();
        e.bytes(&[0x88; 32]).unwrap();
        e.u32(3).unwrap();
        e.array(2).unwrap();
        e.u8(1).unwrap();
        e.array(2).unwrap();
        e.str("https://example.invalid/vote").unwrap();
        e.bytes(&[0x99; 32]).unwrap();

        e.map(0).unwrap();
        e.null().unwrap();
        e.bool(true).unwrap();

        let cbor = e.into_writer();
        let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();

        assert_eq!(
            Mapper::new(NoLedger).map_votes(&tx),
            vec![u5c::VoterVotes {
                voter: Some(u5c::voter_votes::Voter::Drep(u5c::StakeCredential {
                    stake_credential: Some(u5c::stake_credential::StakeCredential::AddrKeyHash(
                        vec![0x77; 28].into()
                    )),
                })),
                votes: vec![u5c::VotingProcedure {
                    gov_action_id: Some(u5c::GovernanceActionId {
                        transaction_id: vec![0x88; 32].into(),
                        governance_action_index: 3,
                    }),
                    vote: u5c::Vote::Yes as i32,
                    anchor: Some(u5c::Anchor {
                        url: "https://example.invalid/vote".to_string(),
                        content_hash: vec![0x99; 32].into(),
                    }),
                }],
            }]
        );
    }
}
