//! Compile probe: a consumer written against the public mapper API of main.
#![allow(deprecated, dead_code)]

use pallas_codec::utils::KeyValuePairs;
use pallas_primitives::{alonzo, babbage, conway};
use pallas_traverse as trv;
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use prost_types::FieldMask;

#[derive(Clone, Default)]
struct Ledger;

impl LedgerContext for Ledger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

macro_rules! probe_shared {
    ($m:ident, $u5c:path) => {
        mod $m {
            use super::*;
            use pallas_utxorpc::$m::Mapper;
            use $u5c as u5c;
            type M = Mapper<Ledger>;

            pub fn probe() {
                let _: fn(Ledger) -> M = M::new;
                let _: fn(&M, FieldMask) -> M = M::masked;
                let _: fn(&M, &alonzo::Certificate, &trv::MultiEraTx, u32) -> u5c::Certificate =
                    M::map_alonzo_compatible_cert;
                let _: fn(&M, &conway::ScriptRef) -> u5c::Script = M::map_any_script;
                let _: fn(&M, &[u8]) -> u5c::Block = M::map_block_cbor;
                let _: fn(&M, &trv::MultiEraBlock) -> u5c::Block = M::map_block;
                let _: fn(&M, &trv::MultiEraCert, &trv::MultiEraTx, u32) -> Option<u5c::Certificate> =
                    M::map_cert;
                let _: fn(&M, &conway::Certificate, &trv::MultiEraTx, u32) -> u5c::Certificate =
                    M::map_conway_cert;
                let _: fn(&M, &conway::ProtocolParamUpdate) -> u5c::PParams =
                    M::map_conway_pparams_update;
                let _: fn(&M, &conway::DRep) -> u5c::DRep = M::map_drep;
                let _: fn(&M, &Option<conway::GovActionId>) -> Option<u5c::GovernanceActionId> =
                    M::map_gov_action_id;
                let _: fn(&M, &trv::MultiEraProposal) -> u5c::GovernanceActionProposal =
                    M::map_gov_proposal;
                let _: fn(&M, u64, &alonzo::Metadatum) -> u5c::Metadata = M::map_metadata;
                let _: fn(&alonzo::Metadatum) -> u5c::Metadatum = M::map_metadatum;
                let _: fn(&alonzo::NativeScript) -> u5c::NativeScript = M::map_native_script;
                let _: fn(&M, &[alonzo::PlutusData]) -> u5c::PlutusDataArray = M::map_plutus_array;
                let _: fn(&M, &alonzo::BigInt) -> u5c::BigInt = M::map_plutus_bigint;
                let _: fn(&M, &alonzo::Constr<alonzo::PlutusData>) -> u5c::Constr =
                    M::map_plutus_constr;
                let _: fn(&M, &alonzo::PlutusData) -> u5c::PlutusData = M::map_plutus_datum;
                let _: fn(&M, &KeyValuePairs<alonzo::PlutusData, alonzo::PlutusData>) -> u5c::PlutusDataMap =
                    M::map_plutus_map;
                let _: fn(&M, pallas_validate::utils::MultiEraProtocolParameters) -> u5c::PParams =
                    M::map_pparams;
                let _: fn(&M, &conway::RedeemerTag) -> u5c::RedeemerPurpose = M::map_purpose;
                let _: fn(&M, &trv::MultiEraRedeemer) -> u5c::Redeemer = M::map_redeemer;
                let _: fn(&M, &alonzo::Relay) -> u5c::Relay = M::map_relay;
                let _: fn(&M, &babbage::StakeCredential) -> u5c::StakeCredential =
                    M::map_stake_credential;
                let _: fn(&M, &trv::MultiEraInput, &Option<UtxoMap>, &trv::MultiEraTx) -> u5c::TxInput =
                    M::map_tx_collateral;
                let _: fn(&M, &trv::MultiEraInput, &trv::MultiEraTx, u32, &Option<UtxoMap>) -> u5c::TxInput =
                    M::map_tx_input;
                let _: fn(&M, &trv::MultiEraInput, &Option<UtxoMap>, &trv::MultiEraTx) -> u5c::TxInput =
                    M::map_tx_reference_input;
                let _: fn(&M, &alonzo::VKeyWitness) -> u5c::VKeyWitness = M::map_vkey_witness;
                let _: fn(&M, &(&[u8], u64), &trv::MultiEraTx, u32) -> u5c::Withdrawal =
                    M::map_withdrawals;
                let _: fn(&M, &trv::MultiEraAsset) -> u5c::Asset = M::map_asset;
                let _: fn(&M, &conway::GovAction) -> u5c::GovernanceAction = M::map_conway_gov_action;
                let _: fn(&M, &trv::MultiEraPolicyAssets) -> u5c::Multiasset = M::map_policy_assets;
                let _: fn(&M, &trv::MultiEraOutput, Option<&trv::MultiEraTx>) -> u5c::Datum =
                    M::map_tx_datum;
                let _: fn(&M, &trv::MultiEraOutput, Option<&trv::MultiEraTx>) -> u5c::TxOutput =
                    M::map_tx_output;
                let _: fn(&M, &trv::MultiEraTx) -> u5c::Tx = M::map_tx;
                let _ = M::default();
                let _ = M::new(Ledger).clone();
            }
        }
    };
}

probe_shared!(v1alpha, pallas_utxorpc::v1alpha::spec::cardano);
probe_shared!(v1beta, pallas_utxorpc::v1beta::spec::cardano);

mod v1beta_only {
    use super::*;
    use pallas_utxorpc::v1beta::Mapper;
    use pallas_utxorpc::v1beta::spec::cardano as u5c;
    type M = Mapper<Ledger>;

    pub fn probe() {
        let _: fn(&M, &alonzo::BootstrapWitness) -> u5c::BootstrapWitness = M::map_bootstrap_witness;
        let _: fn(&M, &conway::Vote) -> u5c::Vote = M::map_vote;
        let _: fn(&M, &trv::MultiEraTx) -> Vec<u5c::VoterVotes> = M::map_votes;
        let _: fn(&M, &conway::GovActionId, &conway::VotingProcedure) -> u5c::VotingProcedure =
            M::map_voting_procedure;
    }
}

fn main() {
    v1alpha::probe();
    v1beta::probe();
    v1beta_only::probe();
    let _: pallas_utxorpc::Mapper<Ledger> = pallas_utxorpc::Mapper::new(Ledger);
    let _: pallas_utxorpc::TxoIndex = 0u32;
    let _: Option<pallas_utxorpc::EraCbor> = None;
    let _: Option<pallas_utxorpc::DatumMap> = None;
    let _: Option<pallas_utxorpc::Cbor> = None;
    let _: Option<pallas_utxorpc::TxHash> = None;
    let _ = pallas_utxorpc::spec::cardano::Block::default();
}
