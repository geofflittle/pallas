use std::ops::Deref;

use pallas_primitives::alonzo;

#[cfg(feature = "unstable")]
use pallas_primitives::{PlutusScript, dijkstra};

#[cfg(feature = "unstable")]
use crate::MultiEraNativeScript;
use crate::MultiEraTx;

impl MultiEraTx<'_> {
    /// PlutusV1 scripts carried by the transaction's auxiliary data.
    pub fn aux_plutus_v1_scripts(&self) -> &[alonzo::PlutusScript<1>] {
        #[cfg(feature = "unstable")]
        if let Some(aux_data) = self.dijkstra_aux_data()
            && let dijkstra::AuxiliaryData::PostAlonzo(x) = aux_data.deref()
            && let Some(plutus) = &x.plutus_v1_scripts
        {
            return plutus.as_ref();
        }

        if let Some(aux_data) = self.aux_data()
            && let alonzo::AuxiliaryData::PostAlonzo(x) = aux_data.deref()
            && let Some(plutus) = &x.plutus_scripts
        {
            return plutus.as_ref();
        }

        &[]
    }

    /// PlutusV2 scripts carried by the transaction's auxiliary data.
    ///
    /// Only Dijkstra's auxiliary data type models the key. Conway's CDDL
    /// defines it and the type this crate carries for Conway does not, so an
    /// earlier era answers empty here whatever its bytes hold.
    #[cfg(feature = "unstable")]
    pub fn aux_plutus_v2_scripts(&self) -> &[PlutusScript<2>] {
        if let Some(aux_data) = self.dijkstra_aux_data()
            && let dijkstra::AuxiliaryData::PostAlonzo(x) = aux_data.deref()
            && let Some(plutus) = &x.plutus_v2_scripts
        {
            return plutus.as_ref();
        }

        &[]
    }

    /// PlutusV3 scripts carried by the transaction's auxiliary data. Modelled
    /// for Dijkstra alone, for the reason [`MultiEraTx::aux_plutus_v2_scripts`]
    /// gives.
    #[cfg(feature = "unstable")]
    pub fn aux_plutus_v3_scripts(&self) -> &[PlutusScript<3>] {
        if let Some(aux_data) = self.dijkstra_aux_data()
            && let dijkstra::AuxiliaryData::PostAlonzo(x) = aux_data.deref()
            && let Some(plutus) = &x.plutus_v3_scripts
        {
            return plutus.as_ref();
        }

        &[]
    }

    /// PlutusV4 scripts carried by the transaction's auxiliary data. This key
    /// is new in Dijkstra, and it and a reference script are the two routes a
    /// V4 script has into a transaction.
    #[cfg(feature = "unstable")]
    pub fn aux_plutus_v4_scripts(&self) -> &[PlutusScript<4>] {
        if let Some(aux_data) = self.dijkstra_aux_data()
            && let dijkstra::AuxiliaryData::PostAlonzo(x) = aux_data.deref()
            && let Some(plutus) = &x.plutus_v4_scripts
        {
            return plutus.as_ref();
        }

        &[]
    }

    /// Native scripts carried by the transaction's auxiliary data, in the
    /// shared type every era through Conway uses.
    ///
    /// A Dijkstra transaction answers empty here whatever its auxiliary data
    /// holds, because Dijkstra's native script clause set is wider than this
    /// type. `MultiEraTx::any_aux_native_scripts` answers for every era.
    #[cfg_attr(
        feature = "unstable",
        deprecated(note = "returns empty for a Dijkstra transaction, use any_aux_native_scripts")
    )]
    pub fn aux_native_scripts(&self) -> &[alonzo::NativeScript] {
        if let Some(aux_data) = self.aux_data() {
            match aux_data.deref() {
                alonzo::AuxiliaryData::PostAlonzo(x) => {
                    if let Some(scripts) = &x.native_scripts {
                        return scripts.as_ref();
                    }
                }
                alonzo::AuxiliaryData::ShelleyMa(x) => {
                    if let Some(scripts) = &x.auxiliary_scripts {
                        return scripts.as_ref();
                    }
                }
                _ => (),
            }
        }

        &[]
    }

    /// Native scripts carried by the transaction's auxiliary data, in the
    /// era's own type for the same reason [`MultiEraTx::any_native_scripts`]
    /// carries it.
    #[cfg(feature = "unstable")]
    pub fn any_aux_native_scripts(&self) -> Vec<MultiEraNativeScript<'_>> {
        if let Some(aux_data) = self.dijkstra_aux_data() {
            return match aux_data.deref() {
                dijkstra::AuxiliaryData::PostAlonzo(x) => x
                    .native_scripts
                    .iter()
                    .flat_map(|s| s.iter())
                    .map(MultiEraNativeScript::from_dijkstra)
                    .collect(),
                dijkstra::AuxiliaryData::ShelleyMa(x) => x
                    .auxiliary_scripts
                    .iter()
                    .flat_map(|s| s.iter())
                    .map(MultiEraNativeScript::from_dijkstra)
                    .collect(),
                dijkstra::AuxiliaryData::Shelley(_) => vec![],
            };
        }

        if let Some(aux_data) = self.aux_data() {
            return match aux_data.deref() {
                alonzo::AuxiliaryData::PostAlonzo(x) => x
                    .native_scripts
                    .iter()
                    .flat_map(|s| s.iter())
                    .map(MultiEraNativeScript::from_alonzo_compatible)
                    .collect(),
                alonzo::AuxiliaryData::ShelleyMa(x) => x
                    .auxiliary_scripts
                    .iter()
                    .flat_map(|s| s.iter())
                    .map(MultiEraNativeScript::from_alonzo_compatible)
                    .collect(),
                alonzo::AuxiliaryData::Shelley(_) => vec![],
            };
        }

        vec![]
    }
}
