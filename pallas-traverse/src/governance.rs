use std::{borrow::Cow, ops::Deref};

use pallas_primitives::conway::{self, GovActionId};

#[cfg(feature = "unstable")]
use pallas_codec::utils::Nullable;
#[cfg(feature = "unstable")]
use pallas_primitives::{
    ExUnits, RationalNumber,
    conway::{DRepVotingThresholds, ExUnitPrices, PoolVotingThresholds},
    dijkstra,
};

use crate::{MultiEraGovAction, MultiEraProposal};

#[cfg(feature = "unstable")]
use crate::{Era, MultiEraParamUpdate};

impl<'b> MultiEraProposal<'b> {
    pub fn from_conway(x: &'b conway::ProposalProcedure) -> Self {
        Self::Conway(Box::new(Cow::Borrowed(x)))
    }

    #[cfg(feature = "unstable")]
    pub fn from_dijkstra(x: &'b dijkstra::ProposalProcedure) -> Self {
        Self::Dijkstra(Box::new(Cow::Borrowed(x)))
    }

    pub fn as_conway(&self) -> Option<&conway::ProposalProcedure> {
        match self {
            MultiEraProposal::Conway(x) => Some(x.deref()),
            #[cfg(feature = "unstable")]
            MultiEraProposal::Dijkstra(_) => None,
        }
    }

    #[cfg(feature = "unstable")]
    pub fn as_dijkstra(&self) -> Option<&dijkstra::ProposalProcedure> {
        match self {
            MultiEraProposal::Dijkstra(x) => Some(x.deref()),
            MultiEraProposal::Conway(_) => None,
        }
    }

    pub fn deposit(&self) -> u64 {
        match self {
            MultiEraProposal::Conway(x) => x.deposit,
            #[cfg(feature = "unstable")]
            MultiEraProposal::Dijkstra(x) => x.deposit,
        }
    }

    pub fn reward_account(&self) -> &[u8] {
        match self {
            MultiEraProposal::Conway(x) => x.reward_account.as_ref(),
            #[cfg(feature = "unstable")]
            MultiEraProposal::Dijkstra(x) => x.reward_account.as_ref(),
        }
    }

    pub fn gov_action(&self) -> MultiEraGovAction<'_> {
        match self {
            MultiEraProposal::Conway(x) => {
                MultiEraGovAction::Conway(Box::new(Cow::Borrowed(&x.gov_action)))
            }
            #[cfg(feature = "unstable")]
            MultiEraProposal::Dijkstra(x) => {
                MultiEraGovAction::Dijkstra(Box::new(Cow::Borrowed(&x.gov_action)))
            }
        }
    }

    /// Both eras carry the same anchor type.
    pub fn anchor(&self) -> &conway::Anchor {
        match self {
            MultiEraProposal::Conway(x) => &x.anchor,
            #[cfg(feature = "unstable")]
            MultiEraProposal::Dijkstra(x) => &x.anchor,
        }
    }
}

impl<'b> MultiEraGovAction<'b> {
    pub fn from_conway(x: &'b conway::GovAction) -> Self {
        Self::Conway(Box::new(Cow::Borrowed(x)))
    }

    #[cfg(feature = "unstable")]
    pub fn from_dijkstra(x: &'b dijkstra::GovAction) -> Self {
        Self::Dijkstra(Box::new(Cow::Borrowed(x)))
    }

    pub fn as_conway(&self) -> Option<&conway::GovAction> {
        match self {
            MultiEraGovAction::Conway(x) => Some(x.deref()),
            #[cfg(feature = "unstable")]
            MultiEraGovAction::Dijkstra(_) => None,
        }
    }

    #[cfg(feature = "unstable")]
    pub fn as_dijkstra(&self) -> Option<&dijkstra::GovAction> {
        match self {
            MultiEraGovAction::Dijkstra(x) => Some(x.deref()),
            MultiEraGovAction::Conway(_) => None,
        }
    }

    pub fn id(&self) -> Option<GovActionId> {
        match self {
            MultiEraGovAction::Conway(x) => match x.deref().deref().clone() {
                conway::GovAction::ParameterChange(id, ..) => id,
                conway::GovAction::HardForkInitiation(id, ..) => id,
                conway::GovAction::NoConfidence(id) => id,
                conway::GovAction::UpdateCommittee(id, ..) => id,
                conway::GovAction::NewConstitution(id, ..) => id,
                _ => None,
            },
            #[cfg(feature = "unstable")]
            MultiEraGovAction::Dijkstra(x) => match x.deref().deref() {
                dijkstra::GovAction::ParameterChange(id, ..) => id.clone(),
                dijkstra::GovAction::HardForkInitiation(id, ..) => id.clone(),
                dijkstra::GovAction::NoConfidence(id) => id.clone(),
                dijkstra::GovAction::UpdateCommittee(id, ..) => id.clone(),
                dijkstra::GovAction::NewConstitution(id, ..) => id.clone(),
                _ => None,
            },
        }
    }

    /// The protocol parameter update a parameter change action proposes, in
    /// the era's own type. Any other action proposes none.
    #[cfg(feature = "unstable")]
    pub fn parameter_update(&self) -> Option<MultiEraParamUpdate<'_>> {
        match self {
            MultiEraGovAction::Conway(x) => match x.deref().deref() {
                conway::GovAction::ParameterChange(_, update, _) => Some(
                    MultiEraParamUpdate::Conway(Box::new(Cow::Borrowed(update.as_ref()))),
                ),
                _ => None,
            },
            MultiEraGovAction::Dijkstra(x) => match x.deref().deref() {
                dijkstra::GovAction::ParameterChange(_, update, _) => Some(
                    MultiEraParamUpdate::Dijkstra(Box::new(Cow::Borrowed(update.as_ref()))),
                ),
                _ => None,
            },
        }
    }
}

/// What one protocol parameter reads in an update.
///
/// Three answers, because two of them are otherwise the same `None`. An era
/// whose `protocol_param_update` rule has no such key is a different fact
/// from an update that could have proposed the parameter and did not, and a
/// caller that has to tell a Leios parameter change from a proposal that
/// changes nothing needs both.
#[cfg(feature = "unstable")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamRead<T> {
    /// This era's `protocol_param_update` rule has no key for the parameter.
    NoSuchParameter,
    /// The era has the key and this update leaves the parameter alone.
    Unchanged,
    /// The update proposes this value.
    Proposed(T),
}

#[cfg(feature = "unstable")]
impl<T> ParamRead<T> {
    /// The proposed value, for the one answer that carries one.
    pub fn proposed(self) -> Option<T> {
        match self {
            ParamRead::Proposed(x) => Some(x),
            ParamRead::NoSuchParameter | ParamRead::Unchanged => None,
        }
    }

    /// Whether the era this update belongs to has the parameter at all.
    pub fn is_known_parameter(&self) -> bool {
        !matches!(self, ParamRead::NoSuchParameter)
    }
}

/// A parameter both eras' rules carry, read from whichever one this is.
#[cfg(feature = "unstable")]
macro_rules! shared_param {
    ($name:ident: $type_:ty) => {
        pub fn $name(&self) -> Option<$type_> {
            match self {
                MultiEraParamUpdate::Conway(x) => x.$name.clone(),
                MultiEraParamUpdate::Dijkstra(x) => x.$name.clone(),
            }
        }
    };
}

/// A parameter only Dijkstra's rule carries. Conway answers
/// [`ParamRead::NoSuchParameter`] rather than the `None` an unchanged
/// parameter would give.
#[cfg(feature = "unstable")]
macro_rules! dijkstra_param {
    ($name:ident: $type_:ty) => {
        pub fn $name(&self) -> ParamRead<$type_> {
            match self {
                MultiEraParamUpdate::Conway(_) => ParamRead::NoSuchParameter,
                MultiEraParamUpdate::Dijkstra(x) => match x.$name.clone() {
                    Some(v) => ParamRead::Proposed(v),
                    None => ParamRead::Unchanged,
                },
            }
        }
    };
}

#[cfg(feature = "unstable")]
impl MultiEraParamUpdate<'_> {
    pub fn as_conway(&self) -> Option<&conway::ProtocolParamUpdate> {
        match self {
            MultiEraParamUpdate::Conway(x) => Some(x.deref()),
            MultiEraParamUpdate::Dijkstra(_) => None,
        }
    }

    pub fn as_dijkstra(&self) -> Option<&dijkstra::ProtocolParamUpdate> {
        match self {
            MultiEraParamUpdate::Dijkstra(x) => Some(x.deref()),
            MultiEraParamUpdate::Conway(_) => None,
        }
    }

    /// The era whose `protocol_param_update` rule wrote this update.
    pub fn era(&self) -> Era {
        match self {
            MultiEraParamUpdate::Conway(_) => Era::Conway,
            MultiEraParamUpdate::Dijkstra(_) => Era::Dijkstra,
        }
    }

    // keys 0 to 33, which both eras carry under the same names

    shared_param!(minfee_a: u64);
    shared_param!(minfee_b: u64);
    shared_param!(max_block_body_size: u64);
    shared_param!(max_transaction_size: u64);
    shared_param!(max_block_header_size: u64);
    shared_param!(key_deposit: u64);
    shared_param!(pool_deposit: u64);
    shared_param!(maximum_epoch: u64);
    shared_param!(desired_number_of_stake_pools: u64);
    shared_param!(pool_pledge_influence: RationalNumber);
    shared_param!(expansion_rate: RationalNumber);
    shared_param!(treasury_growth_rate: RationalNumber);
    shared_param!(min_pool_cost: u64);
    shared_param!(ada_per_utxo_byte: u64);
    shared_param!(execution_costs: ExUnitPrices);
    shared_param!(max_tx_ex_units: ExUnits);
    shared_param!(max_block_ex_units: ExUnits);
    shared_param!(max_value_size: u64);
    shared_param!(collateral_percentage: u64);
    shared_param!(max_collateral_inputs: u64);
    shared_param!(pool_voting_thresholds: PoolVotingThresholds);
    shared_param!(drep_voting_thresholds: DRepVotingThresholds);
    shared_param!(min_committee_size: u64);
    shared_param!(committee_term_limit: u64);
    shared_param!(governance_action_validity_period: u64);
    shared_param!(governance_action_deposit: u64);
    shared_param!(drep_deposit: u64);
    shared_param!(drep_inactivity_period: u64);
    shared_param!(minfee_refscript_cost_per_byte: RationalNumber);

    // -- NEW IN DIJKSTRA: keys 34 to 48

    dijkstra_param!(max_ref_script_size_per_block: u64);
    dijkstra_param!(max_ref_script_size_per_tx: u64);
    dijkstra_param!(ref_script_cost_stride: u64);
    dijkstra_param!(ref_script_cost_multiplier: RationalNumber);
    dijkstra_param!(min_pool_margin: RationalNumber);
    dijkstra_param!(leios_announcement_period_length: u64);
    dijkstra_param!(leios_vote_period_length: u64);
    dijkstra_param!(leios_diffusion_period_length: u64);
    dijkstra_param!(leios_committee_size: u64);
    dijkstra_param!(leios_quorum_stake_threshold: RationalNumber);
    dijkstra_param!(max_endorser_block_references_size: u64);
    dijkstra_param!(max_endorser_block_txs_size: u64);
    dijkstra_param!(max_endorser_block_execution_units: ExUnits);
    dijkstra_param!(max_ref_script_size_per_endorser_block: u64);

    /// Key 38, whose slot is three state on the wire: absent, an explicit nil,
    /// or a value. The nil is a proposal to remove the cap, which is not the
    /// same as proposing nothing, so it is kept.
    pub fn max_pledge_leverage(&self) -> ParamRead<Nullable<RationalNumber>> {
        match self {
            MultiEraParamUpdate::Conway(_) => ParamRead::NoSuchParameter,
            MultiEraParamUpdate::Dijkstra(x) => match x.max_pledge_leverage.clone() {
                Some(v) => ParamRead::Proposed(v),
                None => ParamRead::Unchanged,
            },
        }
    }

    /// Key 18. The two eras' cost model types differ by a named PlutusV4 key,
    /// so this is the one shared key whose value cannot be reported in one
    /// type.
    pub fn conway_cost_models_for_script_languages(&self) -> Option<conway::CostModels> {
        match self {
            MultiEraParamUpdate::Conway(x) => x.cost_models_for_script_languages.clone(),
            MultiEraParamUpdate::Dijkstra(_) => None,
        }
    }

    /// Key 18, read through the era whose cost model map names PlutusV4.
    pub fn dijkstra_cost_models_for_script_languages(&self) -> Option<dijkstra::CostModels> {
        match self {
            MultiEraParamUpdate::Conway(_) => None,
            MultiEraParamUpdate::Dijkstra(x) => x.cost_models_for_script_languages.clone(),
        }
    }
}

#[cfg(all(test, feature = "unstable"))]
mod tests {
    use pallas_codec::minicbor;

    use super::ParamRead;
    use crate::{Era, MultiEraTx, testing};

    /// A `proposal_procedure` whose governance action is a
    /// `parameter_change_action` setting one `protocol_param_update` key.
    ///
    /// No fixture on this chain carries a proposal, so these bytes are built
    /// rather than read. They live in `test_data` because `pallas-primitives`
    /// reads the same two files: a proposal is the only thing that carries a
    /// parameter update, and both crates have to agree on what one looks like.
    fn proposal_key48() -> Vec<u8> {
        hex::decode(include_str!(
            "../../test_data/proposal-param-change-key48.hex"
        ))
        .unwrap()
    }

    fn proposal_key0() -> Vec<u8> {
        hex::decode(include_str!(
            "../../test_data/proposal-param-change-key0.hex"
        ))
        .unwrap()
    }

    /// An `info_action`, which proposes no parameter change at all.
    fn proposal_information() -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(4).unwrap();
        e.u64(1_000_000).unwrap();
        e.bytes(&[0xe0; 29]).unwrap();
        e.array(1).unwrap();
        e.u8(6).unwrap();
        e.array(2).unwrap();
        e.str("https://example.invalid/anchor").unwrap();
        e.bytes(&[0x00; 32]).unwrap();
        e.into_writer()
    }

    fn dijkstra_tx(proposal: &[u8]) -> Vec<u8> {
        testing::dijkstra_block_tx(
            &testing::body_with_proposal(proposal),
            &testing::empty_witness_set(),
            None,
            true,
        )
    }

    fn conway_tx(proposal: &[u8]) -> Vec<u8> {
        testing::conway_tx(
            &testing::body_with_proposal(proposal),
            &testing::empty_witness_set(),
            None,
            true,
        )
    }

    /// Key 48 is `max_ref_script_size_per_endorser_block`, which only this
    /// era's `protocol_param_update` has a field for. A proposal read through
    /// Conway's type decodes without it, so a caller cannot tell a Leios
    /// parameter change from a proposal that changes nothing.
    #[test]
    fn a_dijkstra_proposal_reaches_the_dijkstra_parameter_update() {
        let cbor = dijkstra_tx(&proposal_key48());
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor)
            .expect("a built Dijkstra transaction must decode");

        assert_eq!(tx.era(), Era::Dijkstra);

        let proposals = tx.gov_proposals();
        assert_eq!(
            proposals.len(),
            1,
            "the body carries key 20 with one proposal"
        );

        let proposal = &proposals[0];
        assert_eq!(proposal.deposit(), 1_000_000);
        assert!(
            proposal.as_dijkstra().is_some(),
            "a Dijkstra transaction's proposal must come back as Dijkstra"
        );
        assert!(proposal.as_conway().is_none());

        let action = proposal.gov_action();
        let update = action
            .parameter_update()
            .expect("a parameter change action must yield an update");
        assert_eq!(update.era(), Era::Dijkstra);

        // the four parameters new in Dijkstra, and key 48, answer through the
        // update the proposal path reaches rather than only through a downcast
        assert_eq!(
            update.max_ref_script_size_per_endorser_block(),
            ParamRead::Proposed(20_000),
            "key 48 decoded into no field"
        );
        assert_eq!(
            update.max_ref_script_size_per_block(),
            ParamRead::Unchanged,
            "a key this era has and this proposal did not set"
        );
        assert_eq!(update.max_ref_script_size_per_tx(), ParamRead::Unchanged);
        assert_eq!(update.ref_script_cost_stride(), ParamRead::Unchanged);
        assert_eq!(update.ref_script_cost_multiplier(), ParamRead::Unchanged);
        assert!(
            update.minfee_a().is_none(),
            "and a shared key it did not set"
        );

        let inner = update
            .as_dijkstra()
            .expect("the update must carry this era's type");

        assert_eq!(
            inner.max_ref_script_size_per_endorser_block,
            Some(20_000),
            "key 48 decoded into no field"
        );
    }

    /// The same bytes read as Conway come back as Conway, so the test above is
    /// about the era that read them rather than about every proposal being
    /// reported as Dijkstra.
    #[test]
    fn a_conway_proposal_reaches_the_conway_parameter_update() {
        let cbor = conway_tx(&proposal_key0());
        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor)
            .expect("a built Conway transaction must decode");

        assert_eq!(tx.era(), Era::Conway);

        let proposals = tx.gov_proposals();
        assert_eq!(proposals.len(), 1);

        let proposal = &proposals[0];
        assert!(proposal.as_conway().is_some());
        assert!(proposal.as_dijkstra().is_none());

        let action = proposal.gov_action();
        let update = action
            .parameter_update()
            .expect("a parameter change action must yield an update");
        let conway_update = update
            .as_conway()
            .expect("the update must carry Conway's type");

        assert!(update.as_dijkstra().is_none());
        assert_eq!(update.era(), Era::Conway);
        assert_eq!(conway_update.minfee_a, Some(1_000));
        assert_eq!(update.minfee_a(), Some(1_000));

        // and the parameters this era's rule has no key for say so, rather
        // than answering the way an unset parameter would
        assert_eq!(
            update.max_ref_script_size_per_endorser_block(),
            ParamRead::NoSuchParameter,
            "Conway has no key 48 at all"
        );
        assert!(!update.max_ref_script_size_per_block().is_known_parameter());
        assert!(
            update
                .max_ref_script_size_per_endorser_block()
                .proposed()
                .is_none()
        );
    }

    /// An action that is not a parameter change proposes no update at all,
    /// which is a different answer from a parameter change whose update the
    /// reader could not reach.
    #[test]
    fn an_info_action_proposes_no_parameter_update() {
        let cbor = dijkstra_tx(&proposal_information());
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor)
            .expect("a built Dijkstra transaction must decode");

        let proposals = tx.gov_proposals();
        assert_eq!(proposals.len(), 1);

        let action = proposals[0].gov_action();
        assert!(action.as_dijkstra().is_some());
        assert!(action.parameter_update().is_none());
    }
}
