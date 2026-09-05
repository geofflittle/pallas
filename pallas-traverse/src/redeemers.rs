use std::borrow::Cow;

use pallas_codec::minicbor;
use pallas_primitives::{alonzo, conway, dijkstra};

use crate::MultiEraRedeemer;

impl<'b> MultiEraRedeemer<'b> {
    /// The redeemer's purpose tag.
    ///
    /// Reported in Dijkstra's tag space, which is a strict superset of every
    /// earlier era's: Dijkstra adds `Guarding` at 6 and changes nothing else.
    /// Widening rather than narrowing is what keeps a Dijkstra guarding
    /// redeemer from having to be squeezed into one of the six Conway tags.
    pub fn tag(&self) -> dijkstra::RedeemerTag {
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
            Self::Dijkstra(_, x) => &x.data,
        }
    }

    pub fn ex_units(&self) -> alonzo::ExUnits {
        match &self {
            Self::AlonzoCompatible(x) => x.ex_units,
            Self::Conway(_, x) => x.ex_units,
            Self::Dijkstra(_, x) => x.ex_units,
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Self::AlonzoCompatible(x) => x.index,
            Self::Conway(x, _) => x.index,
            Self::Dijkstra(x, _) => x.index,
        }
    }

    pub fn as_alonzo(&self) -> Option<&alonzo::Redeemer> {
        match self {
            Self::AlonzoCompatible(x) => Some(x),
            Self::Conway(..) => None,
            Self::Dijkstra(..) => None,
        }
    }

    pub fn as_conway(&self) -> Option<(&conway::RedeemersKey, &conway::RedeemersValue)> {
        match self {
            Self::AlonzoCompatible(_) => None,
            Self::Conway(x, y) => Some((x, y)),
            Self::Dijkstra(..) => None,
        }
    }

    pub fn as_dijkstra(&self) -> Option<(&dijkstra::RedeemersKey, &dijkstra::RedeemersValue)> {
        match self {
            Self::AlonzoCompatible(_) => None,
            Self::Conway(..) => None,
            Self::Dijkstra(x, y) => Some((x, y)),
        }
    }

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
            MultiEraRedeemer::Dijkstra(k, v) => minicbor::to_vec((k, v)).unwrap(),
        }
    }
}
