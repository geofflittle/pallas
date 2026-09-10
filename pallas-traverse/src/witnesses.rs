use pallas_codec::utils::KeepRaw;
use pallas_primitives::{
    Hash, PlutusData, PlutusScript,
    alonzo::{self, BootstrapWitness, NativeScript, VKeyWitness},
    conway, dijkstra,
};

use crate::{MultiEraRedeemer, MultiEraTx, OriginalHash as _};

impl<'b> MultiEraTx<'b> {
    pub fn vkey_witnesses(&self) -> &[VKeyWitness] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Babbage(x) => x
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    /// Native scripts for every era whose native script type is Alonzo's.
    ///
    /// **Dijkstra is not one of them and always yields an empty slice here.**
    /// Its `native_script` rule gains a seventh variant, `script_require_guard`,
    /// so it has its own type, which this signature cannot return. Use
    /// [`MultiEraTx::dijkstra_native_scripts`] for a Dijkstra transaction.
    pub fn native_scripts(&self) -> &[KeepRaw<'b, NativeScript>] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .native_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Babbage(x) => x
                .transaction_witness_set
                .native_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .native_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(..) => &[],
        }
    }

    /// Native scripts of a Dijkstra transaction, whose type carries the
    /// `script_require_guard` variant that no earlier era has.
    pub fn dijkstra_native_scripts(&self) -> &[KeepRaw<'b, dijkstra::NativeScript>] {
        match self {
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .native_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            _ => &[],
        }
    }

    pub fn bootstrap_witnesses(&self) -> &[BootstrapWitness] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .bootstrap_witness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Babbage(x) => x
                .transaction_witness_set
                .bootstrap_witness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .bootstrap_witness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .bootstrap_witness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    pub fn plutus_v1_scripts(&self) -> &[alonzo::PlutusScript<1>] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .plutus_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Babbage(x) => x
                .transaction_witness_set
                .plutus_v1_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .plutus_v1_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .plutus_v1_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    pub fn plutus_data(&self) -> &[KeepRaw<'b, PlutusData>] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .plutus_data
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Babbage(x) => x
                .transaction_witness_set
                .plutus_data
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .plutus_data
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .plutus_data
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    pub fn find_plutus_data(&self, hash: &Hash<32>) -> Option<&KeepRaw<'b, PlutusData>> {
        self.plutus_data()
            .iter()
            .find(|x| x.original_hash() == *hash)
    }

    pub fn redeemers(&self) -> Vec<MultiEraRedeemer<'_>> {
        match self {
            Self::Byron(_) => vec![],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .redeemer
                .iter()
                .flat_map(|x| x.iter())
                .map(MultiEraRedeemer::from_alonzo_compatible)
                .collect(),
            Self::Babbage(x) => x
                .transaction_witness_set
                .redeemer
                .iter()
                .flat_map(|x| x.iter())
                .map(MultiEraRedeemer::from_alonzo_compatible)
                .collect(),
            Self::Conway(x) => match x.transaction_witness_set.redeemer.as_deref() {
                Some(conway::Redeemers::Map(x)) => x
                    .iter()
                    .map(|(k, v)| MultiEraRedeemer::from_conway(k, v))
                    .collect(),
                Some(conway::Redeemers::List(x)) => x
                    .iter()
                    .map(MultiEraRedeemer::from_conway_deprecated)
                    .collect(),
                _ => vec![],
            },
            // Dijkstra deleted Conway's array arm, so redeemers are a map and
            // only a map here.
            Self::Dijkstra(x) => match x.transaction_witness_set.redeemer.as_deref() {
                Some(x) => x
                    .iter()
                    .map(|(k, v)| MultiEraRedeemer::from_dijkstra(k, v))
                    .collect(),
                None => vec![],
            },
        }
    }

    pub fn find_spend_redeemer(&self, input_order: u32) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.tag() == pallas_primitives::dijkstra::RedeemerTag::Spend && r.index() == input_order
        })
    }

    pub fn find_mint_redeemer(&self, mint_order: u32) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.tag() == pallas_primitives::dijkstra::RedeemerTag::Mint && r.index() == mint_order
        })
    }

    pub fn find_withdrawal_redeemer(&self, withdrawal_order: u32) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.tag() == pallas_primitives::dijkstra::RedeemerTag::Reward
                && r.index() == withdrawal_order
        })
    }

    pub fn find_certificate_redeemer(
        &self,
        certificate_order: u32,
    ) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.tag() == pallas_primitives::dijkstra::RedeemerTag::Cert
                && r.index() == certificate_order
        })
    }

    pub fn plutus_v2_scripts(&self) -> &[PlutusScript<2>] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(_, _) => &[],
            Self::Babbage(x) => x
                .transaction_witness_set
                .plutus_v2_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Conway(x) => x
                .transaction_witness_set
                .plutus_v2_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .plutus_v2_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    pub fn plutus_v3_scripts(&self) -> &[PlutusScript<3>] {
        match self {
            Self::Byron(_) => &[],
            Self::AlonzoCompatible(_, _) => &[],
            Self::Babbage(_) => &[],
            Self::Conway(x) => x
                .transaction_witness_set
                .plutus_v3_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .plutus_v3_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    /// PlutusV4 scripts attached to the witness set.
    ///
    /// Always empty, in every era including Dijkstra. Dijkstra's
    /// `transaction_witness_set` rule is byte identical to Conway's and stops
    /// at key 7, so there is no witness-set slot for a V4 script. V4 reaches a
    /// transaction through a reference script or through auxiliary data
    /// instead. This accessor exists so that the absence is stated rather than
    /// left to be inferred.
    pub fn plutus_v4_scripts(&self) -> &[PlutusScript<4>] {
        &[]
    }
}
