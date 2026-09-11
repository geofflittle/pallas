use pallas_codec::utils::KeepRaw;
use pallas_primitives::{
    Hash, PlutusData, PlutusScript,
    alonzo::{self, BootstrapWitness, NativeScript, VKeyWitness},
    conway,
};

#[cfg(feature = "unstable")]
use std::{borrow::Cow, ops::Deref};

#[cfg(feature = "unstable")]
use pallas_crypto::hash::Hash as CryptoHash;

#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;

use crate::{MultiEraRedeemer, MultiEraTx, OriginalHash as _};

#[cfg(feature = "unstable")]
use crate::{ComputeHash as _, Era, MultiEraNativeScript};

#[cfg(feature = "unstable")]
impl<'b> MultiEraNativeScript<'b> {
    pub fn from_alonzo_compatible(script: &'b alonzo::NativeScript) -> Self {
        Self::AlonzoCompatible(Cow::Borrowed(script))
    }

    pub fn from_dijkstra(script: &'b dijkstra::NativeScript) -> Self {
        Self::Dijkstra(Cow::Borrowed(script))
    }

    pub fn as_alonzo_compatible(&self) -> Option<&alonzo::NativeScript> {
        match self {
            Self::AlonzoCompatible(x) => Some(x),
            Self::Dijkstra(_) => None,
        }
    }

    pub fn as_dijkstra(&self) -> Option<&dijkstra::NativeScript> {
        match self {
            Self::Dijkstra(x) => Some(x),
            Self::AlonzoCompatible(_) => None,
        }
    }

    /// The era whose native script type carries this value. The Alonzo type
    /// serves every era from Shelley through Conway, so that variant names
    /// Alonzo rather than the era the transaction came from.
    pub fn era(&self) -> Era {
        match self {
            Self::AlonzoCompatible(_) => Era::Alonzo,
            Self::Dijkstra(_) => Era::Dijkstra,
        }
    }

    /// The script hash, blake2b-224 over the script CBOR behind a zero
    /// language tag.
    pub fn hash(&self) -> CryptoHash<28> {
        match self {
            Self::AlonzoCompatible(x) => x.compute_hash(),
            Self::Dijkstra(x) => x.compute_hash(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        // to_vec is infallible
        match self {
            Self::AlonzoCompatible(x) => pallas_codec::minicbor::to_vec(x).unwrap(),
            Self::Dijkstra(x) => pallas_codec::minicbor::to_vec(x).unwrap(),
        }
    }
}

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
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .vkeywitness
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }

    /// The native scripts in the transaction's witness set, in the shared type
    /// every era through Conway uses.
    ///
    /// A Dijkstra transaction answers empty here whatever its witness set
    /// holds, because Dijkstra's `native_script` rule gains a seventh clause,
    /// `script_require_guard`, which this type cannot hold.
    /// `MultiEraTx::any_native_scripts` answers for every era.
    #[cfg_attr(
        feature = "unstable",
        deprecated(note = "returns empty for a Dijkstra transaction, use any_native_scripts")
    )]
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
            #[cfg(feature = "unstable")]
            Self::Dijkstra(_) => &[],
        }
    }

    /// The native scripts in the transaction's witness set, in the era's own
    /// type.
    ///
    /// The return type carries the era because Dijkstra's `native_script`
    /// rule gains a seventh clause, `script_require_guard`, which the type
    /// every earlier era shares cannot hold.
    #[cfg(feature = "unstable")]
    pub fn any_native_scripts(&self) -> Vec<MultiEraNativeScript<'_>> {
        match self {
            Self::Byron(_) => vec![],
            Self::AlonzoCompatible(x, _) => x
                .transaction_witness_set
                .native_script
                .iter()
                .flat_map(|x| x.iter())
                .map(|x| MultiEraNativeScript::from_alonzo_compatible(x.deref()))
                .collect(),
            Self::Babbage(x) => x
                .transaction_witness_set
                .native_script
                .iter()
                .flat_map(|x| x.iter())
                .map(|x| MultiEraNativeScript::from_alonzo_compatible(x.deref()))
                .collect(),
            Self::Conway(x) => x
                .transaction_witness_set
                .native_script
                .iter()
                .flat_map(|x| x.iter())
                .map(|x| MultiEraNativeScript::from_alonzo_compatible(x.deref()))
                .collect(),
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .native_script
                .iter()
                .flat_map(|x| x.iter())
                .map(|x| MultiEraNativeScript::from_dijkstra(x.deref()))
                .collect(),
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
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
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
            r.conway_tag() == Some(conway::RedeemerTag::Spend) && r.index() == input_order
        })
    }

    pub fn find_mint_redeemer(&self, mint_order: u32) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers()
            .into_iter()
            .find(|r| r.conway_tag() == Some(conway::RedeemerTag::Mint) && r.index() == mint_order)
    }

    pub fn find_withdrawal_redeemer(&self, withdrawal_order: u32) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.conway_tag() == Some(conway::RedeemerTag::Reward) && r.index() == withdrawal_order
        })
    }

    pub fn find_certificate_redeemer(
        &self,
        certificate_order: u32,
    ) -> Option<MultiEraRedeemer<'_>> {
        self.redeemers().into_iter().find(|r| {
            r.conway_tag() == Some(conway::RedeemerTag::Cert) && r.index() == certificate_order
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
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x) => x
                .transaction_witness_set
                .plutus_v3_script
                .as_ref()
                .map(|x| x.as_ref())
                .unwrap_or(&[]),
        }
    }
}

#[cfg(all(test, feature = "unstable"))]
mod tests {
    use super::*;
    use crate::{MultiEraBlock, testing};

    /// A Dijkstra transaction's native script witnesses reach the same
    /// accessor every other era's do, and the clause no earlier era can hold
    /// survives the trip.
    ///
    /// No Dijkstra block on this chain carries a script witness of any kind,
    /// so the witness set is built here.
    #[test]
    fn a_dijkstra_native_script_witness_is_readable() {
        let guard = testing::native_script_require_guard(0x7a);
        let cbor = testing::dijkstra_block_tx(
            &testing::minimal_body(),
            &testing::witness_set_with_native_script(&guard),
            None,
            true,
        );

        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");
        let scripts = tx.any_native_scripts();

        assert_eq!(scripts.len(), 1, "the witness set carries one script");
        assert_eq!(scripts[0].era(), Era::Dijkstra);

        // and the narrow accessor answers empty for it, which is the whole of
        // what its deprecation note promises
        #[allow(deprecated)]
        let narrow = tx.native_scripts();
        assert!(
            narrow.is_empty(),
            "the Conway shaped accessor cannot carry a guard clause, so it must answer empty"
        );
        assert!(
            scripts[0].as_alonzo_compatible().is_none(),
            "a Dijkstra script must not be reported in the shared type"
        );

        match scripts[0]
            .as_dijkstra()
            .expect("readable as Dijkstra's type")
        {
            dijkstra::NativeScript::ScriptRequireGuard(
                pallas_primitives::StakeCredential::AddrKeyhash(h),
            ) => assert_eq!(h.as_ref(), [0x7a; 28]),
            other => panic!("expected a guard clause, found {other:?}"),
        }

        assert_eq!(
            hex::encode(scripts[0].encode()),
            hex::encode(&guard),
            "the script re-encodes to the bytes it was read from"
        );
    }

    /// The same accessor on a Conway transaction answers in the type every
    /// era through Conway shares, so the reading above is the era speaking
    /// rather than the accessor answering Dijkstra for everything.
    #[test]
    fn a_conway_native_script_witness_is_read_in_the_shared_type() {
        let script = testing::native_script_pubkey(0x5c);
        let cbor = testing::conway_tx(
            &testing::minimal_body(),
            &testing::witness_set_with_native_script(&script),
            None,
            true,
        );

        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).expect("must decode");
        let scripts = tx.any_native_scripts();

        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].era(), Era::Alonzo);
        assert!(scripts[0].as_dijkstra().is_none());
        assert!(scripts[0].as_alonzo_compatible().is_some());

        // the narrow accessor is not wrong for this era, so it finds it too
        #[allow(deprecated)]
        let narrow = tx.native_scripts();
        assert_eq!(narrow.len(), 1);
    }

    /// A transaction with no script witnesses answers empty, so the counts
    /// above are the witness set speaking rather than the accessor inventing
    /// a script.
    #[test]
    fn a_transaction_with_no_script_witness_reports_none() {
        let cbor = testing::dijkstra_block_tx(
            &testing::minimal_body(),
            &testing::empty_witness_set(),
            None,
            true,
        );
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");
        assert!(tx.any_native_scripts().is_empty());

        let cbor = hex::decode(include_str!("../../test_data/dijkstra3.block")).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        for tx in block.txs() {
            assert!(tx.any_native_scripts().is_empty());
        }
    }

    /// Auxiliary data carries scripts too, and Dijkstra's map gains a key for
    /// a PlutusV4 script, which is one of the two routes a V4 script has into
    /// a transaction.
    #[test]
    fn dijkstra_auxiliary_data_scripts_are_readable() {
        let guard = testing::native_script_require_guard(0x3b);
        let aux = testing::post_alonzo_aux_data(&guard, Some(&[0xd8, 0x79, 0x80]));

        let cbor = testing::dijkstra_block_tx(
            &testing::minimal_body(),
            &testing::empty_witness_set(),
            Some(&aux),
            true,
        );
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");

        let scripts = tx.any_aux_native_scripts();
        assert_eq!(scripts.len(), 1, "the auxiliary data carries one script");
        assert_eq!(scripts[0].era(), Era::Dijkstra);
        assert!(scripts[0].as_dijkstra().is_some());

        // and the narrow accessor answers empty for it, which is what its
        // deprecation note promises
        #[allow(deprecated)]
        let narrow = tx.aux_native_scripts();
        assert!(
            narrow.is_empty(),
            "the Conway shaped accessor reads this era's auxiliary data through a type it has not got"
        );

        assert_eq!(tx.aux_plutus_v4_scripts().len(), 1, "and one V4 script");
        assert_eq!(tx.aux_plutus_v4_scripts()[0].0.as_ref(), [0xd8, 0x79, 0x80]);

        // the witness set carries neither, so these came from the auxiliary
        // data and not from a second reading of the same field
        assert!(tx.any_native_scripts().is_empty());
        assert!(tx.plutus_v1_scripts().is_empty());
    }

    /// The narrow auxiliary data accessor on a Conway transaction does find
    /// the script, so the empty reading above is the era speaking rather than
    /// an accessor that answers empty for everything.
    #[test]
    fn conway_auxiliary_data_scripts_are_read_in_the_shared_type() {
        let script = testing::native_script_pubkey(0x5c);
        let aux = testing::post_alonzo_aux_data(&script, None);

        let cbor = testing::conway_tx(
            &testing::minimal_body(),
            &testing::empty_witness_set(),
            Some(&aux),
            true,
        );
        let tx = MultiEraTx::decode_for_era(Era::Conway, &cbor).expect("must decode");

        #[allow(deprecated)]
        let narrow = tx.aux_native_scripts();
        assert_eq!(narrow.len(), 1, "the auxiliary data carries one script");
        assert_eq!(tx.any_aux_native_scripts().len(), 1);
    }

    /// The same auxiliary data with no V4 script answers empty, so the count
    /// above is the map key speaking.
    #[test]
    fn auxiliary_data_with_no_plutus_v4_script_reports_none() {
        let guard = testing::native_script_require_guard(0x3b);
        let aux = testing::post_alonzo_aux_data(&guard, None);

        let cbor = testing::dijkstra_block_tx(
            &testing::minimal_body(),
            &testing::empty_witness_set(),
            Some(&aux),
            true,
        );
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");

        assert_eq!(tx.any_aux_native_scripts().len(), 1);
        assert!(tx.aux_plutus_v4_scripts().is_empty());
        assert!(tx.aux_plutus_v2_scripts().is_empty());
        assert!(tx.aux_plutus_v3_scripts().is_empty());
    }
}
