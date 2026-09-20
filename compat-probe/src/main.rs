// A consumer of pallas as 1.4.0 published it, outside the workspace, with no
// feature enabled. Every item below was callable at the signature written
// here, so a release that compiles this one is one a caller upgrades into
// without editing.
//
// Each binding coerces a method to a function pointer of the 1.4.0 type.
// Coercion is exact, so a changed argument or return type fails to compile
// the same way a removed method does.
#![allow(deprecated)]

use std::collections::HashMap;

use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use utxorpc_spec::utxorpc::v1alpha::cardano as a;
use utxorpc_spec::utxorpc::v1beta::cardano as b;

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        Some(HashMap::new())
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

macro_rules! shared_surface {
    ($name:ident, $v:ident, $u5c:ident) => {
        fn $name() {
            use pallas_utxorpc::$v::Mapper;

            type M = Mapper<NoLedger>;

            let _: fn(C) -> M = M::new;
            let _: fn(&M, prost_types::FieldMask) -> M = M::masked;

            let _: fn(&M, &pallas_primitives::conway::RedeemerTag) -> $u5c::RedeemerPurpose =
                M::map_purpose;
            let _: fn(&M, &pallas_traverse::MultiEraRedeemer) -> $u5c::Redeemer = M::map_redeemer;
            let _: fn(
                &M,
                &pallas_traverse::MultiEraInput,
                &pallas_traverse::MultiEraTx,
                u32,
                &Option<UtxoMap>,
            ) -> $u5c::TxInput = M::map_tx_input;
            let _: fn(
                &M,
                &pallas_traverse::MultiEraInput,
                &Option<UtxoMap>,
                &pallas_traverse::MultiEraTx,
            ) -> $u5c::TxInput = M::map_tx_reference_input;
            let _: fn(
                &M,
                &pallas_traverse::MultiEraInput,
                &Option<UtxoMap>,
                &pallas_traverse::MultiEraTx,
            ) -> $u5c::TxInput = M::map_tx_collateral;
            let _: fn(&pallas_primitives::alonzo::NativeScript) -> $u5c::NativeScript =
                M::map_native_script;
            let _: fn(&M, &pallas_primitives::conway::ScriptRef) -> $u5c::Script =
                M::map_any_script;
            let _: fn(&M, &pallas_primitives::babbage::StakeCredential) -> $u5c::StakeCredential =
                M::map_stake_credential;
            let _: fn(&M, &pallas_primitives::alonzo::Relay) -> $u5c::Relay = M::map_relay;
            let _: fn(&M, &(&[u8], u64), &pallas_traverse::MultiEraTx, u32) -> $u5c::Withdrawal =
                M::map_withdrawals;
            let _: fn(&M, &pallas_primitives::alonzo::VKeyWitness) -> $u5c::VKeyWitness =
                M::map_vkey_witness;
            let _: fn(
                &M,
                &pallas_primitives::alonzo::Constr<pallas_primitives::alonzo::PlutusData>,
            ) -> $u5c::Constr = M::map_plutus_constr;
            let _: fn(
                &M,
                &pallas_codec::utils::KeyValuePairs<
                    pallas_primitives::alonzo::PlutusData,
                    pallas_primitives::alonzo::PlutusData,
                >,
            ) -> $u5c::PlutusDataMap = M::map_plutus_map;
            let _: fn(&M, &[pallas_primitives::alonzo::PlutusData]) -> $u5c::PlutusDataArray =
                M::map_plutus_array;
            let _: fn(&M, &pallas_primitives::alonzo::BigInt) -> $u5c::BigInt = M::map_plutus_bigint;
            let _: fn(&M, &pallas_primitives::alonzo::PlutusData) -> $u5c::PlutusData =
                M::map_plutus_datum;
            let _: fn(
                &M,
                &Option<pallas_primitives::conway::GovActionId>,
            ) -> Option<$u5c::GovernanceActionId> = M::map_gov_action_id;
            let _: fn(&M, &pallas_traverse::MultiEraProposal) -> $u5c::GovernanceActionProposal =
                M::map_gov_proposal;
            let _: fn(&pallas_primitives::alonzo::Metadatum) -> $u5c::Metadatum = M::map_metadatum;
            let _: fn(&M, u64, &pallas_primitives::alonzo::Metadatum) -> $u5c::Metadata =
                M::map_metadata;
            let _: fn(&M, &pallas_traverse::MultiEraBlock) -> $u5c::Block = M::map_block;
            let _: fn(&M, &[u8]) -> $u5c::Block = M::map_block_cbor;
            let _: fn(
                &M,
                &pallas_primitives::alonzo::Certificate,
                &pallas_traverse::MultiEraTx,
                u32,
            ) -> $u5c::Certificate = M::map_alonzo_compatible_cert;
            let _: fn(&M, &pallas_primitives::conway::DRep) -> $u5c::DRep = M::map_drep;
            let _: fn(
                &M,
                &pallas_primitives::conway::Certificate,
                &pallas_traverse::MultiEraTx,
                u32,
            ) -> $u5c::Certificate = M::map_conway_cert;
            let _: fn(
                &M,
                &pallas_traverse::MultiEraCert,
                &pallas_traverse::MultiEraTx,
                u32,
            ) -> Option<$u5c::Certificate> = M::map_cert;
            let _: fn(&M, pallas_validate::utils::MultiEraProtocolParameters) -> $u5c::PParams =
                M::map_pparams;
            let _: fn(&M, &pallas_primitives::conway::ProtocolParamUpdate) -> $u5c::PParams =
                M::map_conway_pparams_update;

            let _: fn(
                &M,
                &pallas_traverse::MultiEraOutput,
                Option<&pallas_traverse::MultiEraTx>,
            ) -> $u5c::Datum = M::map_tx_datum;
            let _: fn(
                &M,
                &pallas_traverse::MultiEraOutput,
                Option<&pallas_traverse::MultiEraTx>,
            ) -> $u5c::TxOutput = M::map_tx_output;
            let _: fn(&M, &pallas_traverse::MultiEraAsset) -> $u5c::Asset = M::map_asset;
            let _: fn(&M, &pallas_traverse::MultiEraPolicyAssets) -> $u5c::Multiasset =
                M::map_policy_assets;
            let _: fn(&M, &pallas_primitives::conway::GovAction) -> $u5c::GovernanceAction =
                M::map_conway_gov_action;
            let _: fn(&M, &pallas_traverse::MultiEraTx) -> $u5c::Tx = M::map_tx;
        }
    };
}

type C = NoLedger;

shared_surface!(v1alpha_surface, v1alpha, a);
shared_surface!(v1beta_surface, v1beta, b);

fn v1beta_only_surface() {
    use pallas_utxorpc::v1beta::Mapper;

    type M = Mapper<NoLedger>;

    let _: fn(&M, &pallas_primitives::alonzo::BootstrapWitness) -> b::BootstrapWitness =
        M::map_bootstrap_witness;
    let _: fn(&M, &pallas_primitives::conway::Vote) -> b::Vote = M::map_vote;
    let _: fn(
        &M,
        &pallas_primitives::conway::GovActionId,
        &pallas_primitives::conway::VotingProcedure,
    ) -> b::VotingProcedure = M::map_voting_procedure;
    let _: fn(&M, &pallas_traverse::MultiEraTx) -> Vec<b::VoterVotes> = M::map_votes;
}

// Matches with no wildcard arm over the two traverse enums that carried no
// non_exhaustive in 1.4.0, so a caller could name every variant. A variant
// added outside the unstable feature, or an unconditional non_exhaustive,
// stops these compiling.
fn exhaustive_header(header: &pallas_traverse::MultiEraHeader) -> u8 {
    use pallas_traverse::MultiEraHeader as H;

    match header {
        H::EpochBoundary(_) => 0,
        H::ShelleyCompatible(_) => 1,
        H::BabbageCompatible(_) => 2,
        H::Byron(_) => 3,
    }
}

fn exhaustive_error(error: &pallas_traverse::Error) -> u8 {
    use pallas_traverse::Error as E;

    match error {
        E::InvalidCbor(_) => 0,
        E::UnknownCbor(_) => 1,
        E::UnknownEra(_) => 2,
        E::InvalidEra(_) => 3,
        E::InvalidUtxoRef(_) => 4,
    }
}

fn main() {
    v1alpha_surface();
    v1beta_surface();
    v1beta_only_surface();

    assert_eq!(
        exhaustive_error(&pallas_traverse::Error::UnknownEra(9)),
        2,
        "the error a 1.4.0 caller matched by name still reaches the arm it named"
    );
    assert_eq!(
        exhaustive_error(&pallas_traverse::Error::InvalidUtxoRef(String::new())),
        4
    );
    let _ = exhaustive_header;

    println!("pallas 1.4.0 consumer surface compiles");
}
