use std::collections::BTreeMap;

use super::{CostModel, PlutusData, Redeemers, WitnessSet};
use pallas_codec::minicbor::{self, Encode};
use pallas_codec::utils::{KeepRaw, NonEmptySet};
use serde::{Deserialize, Serialize};

pub type PlutusVersion = u8;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageViews(pub BTreeMap<PlutusVersion, CostModel>);

impl FromIterator<(PlutusVersion, CostModel)> for LanguageViews {
    fn from_iter<I: IntoIterator<Item = (PlutusVersion, CostModel)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<C> Encode<C> for LanguageViews {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        let order: Vec<u8> = self.0.keys().copied().collect();
        let mut canonical_order: Vec<u8> = order.into_iter().filter(|&k| k != 0).collect();
        canonical_order.sort();
        // PlutusV1 is CBOR encoded as 0x4100 so it goes last
        if self.0.contains_key(&0) {
            canonical_order.push(0);
        }

        e.map(self.0.len() as u64)?;
        for lang in canonical_order {
            let cost_model = self.0.get(&lang).unwrap();
            match lang {
                0 => {
                    let mut inner = vec![];
                    let mut sub = minicbor::Encoder::new(&mut inner);
                    sub.begin_array().unwrap();
                    for v in cost_model.iter() {
                        sub.encode_with(v, ctx).unwrap();
                    }
                    sub.end().unwrap();
                    e.bytes(&minicbor::to_vec(0).unwrap())?;
                    e.bytes(&inner)?;
                }
                _ => {
                    e.encode(lang)?;
                    e.encode(cost_model)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ScriptData<'b> {
    pub redeemers: Option<Redeemers>,
    pub datums: Option<KeepRaw<'b, NonEmptySet<KeepRaw<'b, PlutusData>>>>,
    pub language_views: Option<LanguageViews>,
}

impl ScriptData<'_> {
    pub fn hash(&self) -> pallas_crypto::hash::Hash<32> {
        let mut buf = vec![];

        if let Some(redeemers) = &self.redeemers {
            minicbor::encode(redeemers, &mut buf).unwrap(); // infallible
        } else {
            buf.push(0xa0);
        }

        if let Some(datums) = &self.datums {
            minicbor::encode(datums, &mut buf).unwrap(); // infallible
        }

        if let Some(language_views) = &self.language_views {
            minicbor::encode(language_views, &mut buf).unwrap(); // infallible
        } else {
            buf.push(0xa0);
        }

        pallas_crypto::hash::Hasher::<256>::hash(&buf)
    }
}

impl<'b> ScriptData<'b> {
    pub fn build_for(
        witness: &WitnessSet<'b>,
        language_views_opt: &Option<LanguageViews>,
    ) -> Option<Self> {
        let redeemers = witness.redeemer.as_ref().map(|x| x.to_owned().unwrap());
        let datums = witness.plutus_data.clone();

        if redeemers.is_none() && datums.is_none() {
            return None;
        }

        let language_views = if redeemers.is_some() && language_views_opt.is_some() {
            language_views_opt.clone()
        } else {
            None
        };

        Some(ScriptData {
            redeemers,
            datums,
            language_views,
        })
    }
}
