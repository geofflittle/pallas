use std::borrow::Cow;

use pallas_codec::minicbor;
use pallas_primitives::{alonzo, conway};

#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;

use crate::MultiEraRedeemer;

impl<'b> MultiEraRedeemer<'b> {
    /// The redeemer's purpose in Conway's tag space, or `None` for a purpose
    /// that space has no tag for.
    ///
    /// Dijkstra's `Guarding` is the only such purpose today. Every search in
    /// this crate compares against this rather than against [`Self::tag`], so
    /// a guarding redeemer is passed over rather than mistaken for one of the
    /// six or made to panic.
    pub fn conway_tag(&self) -> Option<conway::RedeemerTag> {
        match &self {
            Self::AlonzoCompatible(x) => Some(match x.tag {
                alonzo::RedeemerTag::Cert => conway::RedeemerTag::Cert,
                alonzo::RedeemerTag::Spend => conway::RedeemerTag::Spend,
                alonzo::RedeemerTag::Mint => conway::RedeemerTag::Mint,
                alonzo::RedeemerTag::Reward => conway::RedeemerTag::Reward,
            }),
            Self::Conway(x, _) => Some(x.tag),
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x, _) => match x.tag {
                dijkstra::RedeemerTag::Cert => Some(conway::RedeemerTag::Cert),
                dijkstra::RedeemerTag::Spend => Some(conway::RedeemerTag::Spend),
                dijkstra::RedeemerTag::Mint => Some(conway::RedeemerTag::Mint),
                dijkstra::RedeemerTag::Reward => Some(conway::RedeemerTag::Reward),
                dijkstra::RedeemerTag::Vote => Some(conway::RedeemerTag::Vote),
                dijkstra::RedeemerTag::Propose => Some(conway::RedeemerTag::Propose),
                dijkstra::RedeemerTag::Guarding => None,
            },
        }
    }

    /// The redeemer's purpose, in Conway's tag space.
    ///
    /// Dijkstra's tag space is one wider, and its seventh purpose, `Guarding`,
    /// has no Conway tag to report. There is no `None` to return here and no
    /// Conway tag that means "something else", so a guarding redeemer read
    /// through this accessor panics rather than being reported as one of the
    /// six. `MultiEraRedeemer::any_tag` answers for every era.
    #[cfg_attr(
        feature = "unstable",
        deprecated(note = "panics on Dijkstra's guarding purpose, use any_tag")
    )]
    pub fn tag(&self) -> conway::RedeemerTag {
        self.conway_tag().unwrap_or_else(|| {
            panic!(
                "this redeemer's purpose has no Conway tag, read it with MultiEraRedeemer::any_tag"
            )
        })
    }

    /// The redeemer's purpose, in Dijkstra's tag space.
    ///
    /// That space is a strict superset of every earlier era's: Dijkstra adds
    /// `Guarding` at 6 and changes nothing else. Reporting in the wider space
    /// is what keeps a guarding redeemer from having to be squeezed into one
    /// of the six Conway tags.
    #[cfg(feature = "unstable")]
    pub fn any_tag(&self) -> dijkstra::RedeemerTag {
        match &self {
            Self::AlonzoCompatible(x) => match x.tag {
                alonzo::RedeemerTag::Cert => dijkstra::RedeemerTag::Cert,
                alonzo::RedeemerTag::Spend => dijkstra::RedeemerTag::Spend,
                alonzo::RedeemerTag::Mint => dijkstra::RedeemerTag::Mint,
                alonzo::RedeemerTag::Reward => dijkstra::RedeemerTag::Reward,
            },
            Self::Conway(x, _) => match x.tag {
                conway::RedeemerTag::Cert => dijkstra::RedeemerTag::Cert,
                conway::RedeemerTag::Spend => dijkstra::RedeemerTag::Spend,
                conway::RedeemerTag::Mint => dijkstra::RedeemerTag::Mint,
                conway::RedeemerTag::Reward => dijkstra::RedeemerTag::Reward,
                conway::RedeemerTag::Vote => dijkstra::RedeemerTag::Vote,
                conway::RedeemerTag::Propose => dijkstra::RedeemerTag::Propose,
            },
            Self::Dijkstra(x, _) => x.tag,
        }
    }

    pub fn data(&self) -> &alonzo::PlutusData {
        match &self {
            Self::AlonzoCompatible(x) => &x.data,
            Self::Conway(_, x) => &x.data,
            #[cfg(feature = "unstable")]
            Self::Dijkstra(_, x) => &x.data,
        }
    }

    pub fn ex_units(&self) -> alonzo::ExUnits {
        match &self {
            Self::AlonzoCompatible(x) => x.ex_units,
            Self::Conway(_, x) => x.ex_units,
            #[cfg(feature = "unstable")]
            Self::Dijkstra(_, x) => x.ex_units,
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Self::AlonzoCompatible(x) => x.index,
            Self::Conway(x, _) => x.index,
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x, _) => x.index,
        }
    }

    pub fn as_alonzo(&self) -> Option<&alonzo::Redeemer> {
        match self {
            Self::AlonzoCompatible(x) => Some(x),
            Self::Conway(..) => None,
            #[cfg(feature = "unstable")]
            Self::Dijkstra(..) => None,
        }
    }

    pub fn as_conway(&self) -> Option<(&conway::RedeemersKey, &conway::RedeemersValue)> {
        match self {
            Self::AlonzoCompatible(_) => None,
            Self::Conway(x, y) => Some((x, y)),
            #[cfg(feature = "unstable")]
            Self::Dijkstra(..) => None,
        }
    }

    #[cfg(feature = "unstable")]
    pub fn as_dijkstra(&self) -> Option<(&dijkstra::RedeemersKey, &dijkstra::RedeemersValue)> {
        match self {
            Self::AlonzoCompatible(_) => None,
            Self::Conway(..) => None,
            Self::Dijkstra(x, y) => Some((x, y)),
        }
    }

    #[cfg(feature = "unstable")]
    pub fn from_dijkstra(
        redeemers_key: &'b dijkstra::RedeemersKey,
        redeemers_val: &'b dijkstra::RedeemersValue,
    ) -> Self {
        Self::Dijkstra(
            Box::new(Cow::Borrowed(redeemers_key)),
            Box::new(Cow::Borrowed(redeemers_val)),
        )
    }

    pub fn into_conway_deprecated(&self) -> Option<conway::Redeemer> {
        match self {
            Self::AlonzoCompatible(_) => None,
            Self::Conway(x, y) => Some(conway::Redeemer {
                tag: x.tag,
                index: x.index,
                data: y.data.clone(),
                ex_units: y.ex_units,
            }),
            #[cfg(feature = "unstable")]
            Self::Dijkstra(..) => None,
        }
    }

    pub fn from_alonzo_compatible(redeemer: &'b alonzo::Redeemer) -> Self {
        Self::AlonzoCompatible(Box::new(Cow::Borrowed(redeemer)))
    }

    pub fn from_conway(
        redeemers_key: &'b conway::RedeemersKey,
        redeemers_val: &'b conway::RedeemersValue,
    ) -> Self {
        Self::Conway(
            Box::new(Cow::Borrowed(redeemers_key)),
            Box::new(Cow::Borrowed(redeemers_val)),
        )
    }

    pub fn from_conway_deprecated(redeemer: &'b conway::Redeemer) -> Self {
        Self::Conway(
            Box::new(Cow::Owned(conway::RedeemersKey {
                tag: redeemer.tag,
                index: redeemer.index,
            })),
            Box::new(Cow::Owned(conway::RedeemersValue {
                data: redeemer.data.clone(),
                ex_units: redeemer.ex_units,
            })),
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            MultiEraRedeemer::AlonzoCompatible(x) => minicbor::to_vec(x).unwrap(),
            MultiEraRedeemer::Conway(k, v) => minicbor::to_vec((k, v)).unwrap(),
            #[cfg(feature = "unstable")]
            MultiEraRedeemer::Dijkstra(k, v) => minicbor::to_vec((k, v)).unwrap(),
        }
    }
}

#[cfg(all(test, feature = "unstable"))]
mod tests {
    use super::*;
    use crate::{Era, MultiEraTx, testing};

    fn redeemer_tx(tag: u8) -> Vec<u8> {
        testing::dijkstra_block_tx(
            &testing::minimal_body(),
            &testing::witness_set_with_redeemer(tag, 0),
            None,
            true,
        )
    }

    /// Decode a transaction carrying one redeemer of the given purpose and
    /// hand that redeemer to the caller. It borrows from the decoded
    /// transaction, so it cannot outlive this call.
    fn with_only_redeemer<T>(tag: u8, f: impl FnOnce(&MultiEraRedeemer<'_>) -> T) -> T {
        let cbor = redeemer_tx(tag);
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");
        let redeemers = tx.redeemers();
        assert_eq!(redeemers.len(), 1, "the witness set carries one redeemer");
        f(&redeemers[0])
    }

    /// The guarding purpose is Dijkstra's seventh redeemer tag, and no earlier
    /// era's tag space has a member for it.
    ///
    /// No fixture on this chain carries a redeemer of any purpose, so these
    /// bytes are built. The pair of readings is the point: the wide accessor
    /// names the purpose, and the Conway shaped one says it has no tag for it
    /// rather than reporting one of the six that mean something else.
    #[test]
    fn a_guarding_redeemer_is_named_by_the_wide_accessor_alone() {
        with_only_redeemer(6, |redeemer| {
            assert_eq!(redeemer.any_tag(), dijkstra::RedeemerTag::Guarding);
            assert_eq!(
                redeemer.conway_tag(),
                None,
                "the guarding purpose has no Conway tag to be reported as"
            );
            assert_eq!(redeemer.index(), 0);
        });
    }

    /// A purpose both tag spaces have answers through both accessors, so the
    /// `None` above is the purpose speaking rather than the accessor answering
    /// `None` for every Dijkstra redeemer.
    #[test]
    fn a_spend_redeemer_is_named_by_both_accessors() {
        with_only_redeemer(0, |redeemer| {
            assert_eq!(redeemer.any_tag(), dijkstra::RedeemerTag::Spend);
            assert_eq!(redeemer.conway_tag(), Some(conway::RedeemerTag::Spend));

            #[allow(deprecated)]
            let narrow = redeemer.tag();
            assert_eq!(narrow, conway::RedeemerTag::Spend);
        });
    }

    /// The deprecated accessor returns a bare tag, so it has no `None` to give
    /// for a guarding redeemer. It panics rather than picking one of the six,
    /// which is what its deprecation note says.
    #[test]
    #[should_panic(expected = "no Conway tag")]
    fn the_deprecated_accessor_refuses_a_guarding_redeemer() {
        with_only_redeemer(6, |redeemer| {
            #[allow(deprecated)]
            let _ = redeemer.tag();
        });
    }
}
