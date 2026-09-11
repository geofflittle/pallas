use std::{borrow::Cow, ops::Deref};

use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{conway, dijkstra};

use crate::{ComputeHash as _, Era, MultiEraNativeScript, MultiEraScriptRef, OriginalHash as _};

impl<'b> MultiEraScriptRef<'b> {
    pub fn from_conway(script: &'b conway::ScriptRef<'b>) -> Self {
        Self::Conway(Cow::Borrowed(script))
    }

    pub fn from_dijkstra(script: &'b dijkstra::ScriptRef<'b>) -> Self {
        Self::Dijkstra(Cow::Borrowed(script))
    }

    pub fn as_conway(&self) -> Option<&conway::ScriptRef<'b>> {
        match self {
            Self::Conway(x) => Some(x),
            Self::Dijkstra(_) => None,
        }
    }

    pub fn as_dijkstra(&self) -> Option<&dijkstra::ScriptRef<'b>> {
        match self {
            Self::Dijkstra(x) => Some(x),
            Self::Conway(_) => None,
        }
    }

    /// The era whose `script` rule carries this value. The Conway type serves
    /// Babbage as well, so that variant names Conway rather than the era the
    /// output came from.
    pub fn era(&self) -> Era {
        match self {
            Self::Conway(_) => Era::Conway,
            Self::Dijkstra(_) => Era::Dijkstra,
        }
    }

    /// The script's language, which is what a hash is tagged with and what a
    /// consumer needs in order to know which evaluator applies.
    pub fn language(&self) -> ScriptLanguage {
        match self {
            Self::Conway(x) => match x.deref() {
                conway::ScriptRef::NativeScript(_) => ScriptLanguage::Native,
                conway::ScriptRef::PlutusV1Script(_) => ScriptLanguage::PlutusV1,
                conway::ScriptRef::PlutusV2Script(_) => ScriptLanguage::PlutusV2,
                conway::ScriptRef::PlutusV3Script(_) => ScriptLanguage::PlutusV3,
            },
            Self::Dijkstra(x) => match x.deref() {
                dijkstra::ScriptRef::NativeScript(_) => ScriptLanguage::Native,
                dijkstra::ScriptRef::PlutusV1Script(_) => ScriptLanguage::PlutusV1,
                dijkstra::ScriptRef::PlutusV2Script(_) => ScriptLanguage::PlutusV2,
                dijkstra::ScriptRef::PlutusV3Script(_) => ScriptLanguage::PlutusV3,
                dijkstra::ScriptRef::PlutusV4Script(_) => ScriptLanguage::PlutusV4,
            },
        }
    }

    /// The native script behind this reference, for the one language that has
    /// one. A Plutus reference answers `None`, and [`Self::language`] says
    /// which one it is.
    pub fn native_script(&self) -> Option<MultiEraNativeScript<'_>> {
        match self {
            Self::Conway(x) => match x.deref() {
                conway::ScriptRef::NativeScript(s) => {
                    Some(MultiEraNativeScript::from_alonzo_compatible(s.deref()))
                }
                _ => None,
            },
            Self::Dijkstra(x) => match x.deref() {
                dijkstra::ScriptRef::NativeScript(s) => {
                    Some(MultiEraNativeScript::from_dijkstra(s.deref()))
                }
                _ => None,
            },
        }
    }

    /// The compiled Plutus script behind this reference, for the languages
    /// that have one. A native reference answers `None` and reaches its
    /// content through [`Self::native_script`].
    pub fn plutus_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Conway(x) => match x.deref() {
                conway::ScriptRef::NativeScript(_) => None,
                conway::ScriptRef::PlutusV1Script(s) => Some(s.0.as_ref()),
                conway::ScriptRef::PlutusV2Script(s) => Some(s.0.as_ref()),
                conway::ScriptRef::PlutusV3Script(s) => Some(s.0.as_ref()),
            },
            Self::Dijkstra(x) => match x.deref() {
                dijkstra::ScriptRef::NativeScript(_) => None,
                dijkstra::ScriptRef::PlutusV1Script(s) => Some(s.0.as_ref()),
                dijkstra::ScriptRef::PlutusV2Script(s) => Some(s.0.as_ref()),
                dijkstra::ScriptRef::PlutusV3Script(s) => Some(s.0.as_ref()),
                dijkstra::ScriptRef::PlutusV4Script(s) => Some(s.0.as_ref()),
            },
        }
    }

    /// The script hash the ledger keys this script by, blake2b-224 over the
    /// script behind its language tag.
    ///
    /// A native script hashes its original CBOR rather than a re-encoding, so
    /// a script written non canonically still answers the hash the ledger
    /// committed to.
    pub fn hash(&self) -> Hash<28> {
        match self {
            Self::Conway(x) => match x.deref() {
                conway::ScriptRef::NativeScript(s) => s.original_hash(),
                conway::ScriptRef::PlutusV1Script(s) => s.compute_hash(),
                conway::ScriptRef::PlutusV2Script(s) => s.compute_hash(),
                conway::ScriptRef::PlutusV3Script(s) => s.compute_hash(),
            },
            Self::Dijkstra(x) => match x.deref() {
                dijkstra::ScriptRef::NativeScript(s) => s.original_hash(),
                dijkstra::ScriptRef::PlutusV1Script(s) => s.compute_hash(),
                dijkstra::ScriptRef::PlutusV2Script(s) => s.compute_hash(),
                dijkstra::ScriptRef::PlutusV3Script(s) => s.compute_hash(),
                dijkstra::ScriptRef::PlutusV4Script(s) => s.compute_hash(),
            },
        }
    }

    /// The `script` CBOR, in the era's own encoding.
    pub fn encode(&self) -> Vec<u8> {
        // to_vec is infallible
        match self {
            Self::Conway(x) => minicbor::to_vec(x).unwrap(),
            Self::Dijkstra(x) => minicbor::to_vec(x).unwrap(),
        }
    }

    /// Read a `script` in the era's own encoding.
    ///
    /// The era is a parameter because the encodings overlap: variants 0
    /// through 3 are identical, so bytes alone cannot say which rule wrote
    /// them, and only Dijkstra's rule admits variant 4.
    pub fn decode(era: Era, cbor: &'b [u8]) -> Result<Self, minicbor::decode::Error> {
        match era {
            Era::Dijkstra => Ok(Self::Dijkstra(Cow::Owned(minicbor::decode(cbor)?))),
            Era::Babbage | Era::Conway => Ok(Self::Conway(Cow::Owned(minicbor::decode(cbor)?))),
            Era::Byron | Era::Shelley | Era::Allegra | Era::Mary | Era::Alonzo => Err(
                minicbor::decode::Error::message(format!("{era} has no reference script rule")),
            ),
        }
    }
}

/// The language a script is written in, which is the byte its hash is tagged
/// with.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptLanguage {
    /// A native script, tagged 0.
    Native,
    /// Plutus V1, tagged 1.
    PlutusV1,
    /// Plutus V2, tagged 2.
    PlutusV2,
    /// Plutus V3, tagged 3.
    PlutusV3,
    /// Plutus V4, tagged 4, new in Dijkstra.
    PlutusV4,
}

impl ScriptLanguage {
    /// The tag byte the ledger prefixes to a script before hashing it.
    pub fn tag(&self) -> u8 {
        match self {
            Self::Native => 0,
            Self::PlutusV1 => 1,
            Self::PlutusV2 => 2,
            Self::PlutusV3 => 3,
            Self::PlutusV4 => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultiEraTx, testing};
    use pallas_crypto::hash::Hasher;

    const ADDRESS: [u8; 29] = [0x60; 29];
    const V4_BYTES: [u8; 3] = [0xd8, 0x79, 0x80];

    fn dijkstra_output_with_script(script: &[u8]) -> Vec<u8> {
        let output = testing::post_alonzo_output_with_script_ref(&ADDRESS, 2_000_000, script);
        testing::dijkstra_block_tx(
            &testing::body_with_output(&output),
            &testing::empty_witness_set(),
            None,
            true,
        )
    }

    /// A PlutusV4 reference script reaches an output, is reported as V4 rather
    /// than as one of the languages before it, and hashes under tag 4.
    ///
    /// No output on this chain carries a reference script, so this one is
    /// built. V4 is the arm that made a widened return type necessary in the
    /// first place, so it is the arm that has to be shown working.
    #[test]
    fn a_plutus_v4_reference_script_is_reported_as_v4() {
        let cbor = dijkstra_output_with_script(&testing::script_ref_plutus_v4(&V4_BYTES));
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");

        let output = &tx.outputs()[0];
        let script = output
            .any_script_ref()
            .expect("a written reference script must be readable");

        // and the Conway shaped accessor reports none, which is the whole of
        // what its deprecation note promises: it has no arm that could hold a
        // V4 script, so it says nothing rather than saying the wrong language
        #[allow(deprecated)]
        let narrow = output.script_ref();
        assert!(
            narrow.is_none(),
            "a PlutusV4 reference script has no Conway representation"
        );

        assert_eq!(script.era(), Era::Dijkstra);
        assert_eq!(script.language(), ScriptLanguage::PlutusV4);
        assert_eq!(script.language().tag(), 4);
        assert_eq!(script.plutus_bytes(), Some(V4_BYTES.as_slice()));
        assert!(script.native_script().is_none());
        assert!(script.as_conway().is_none());
        assert!(script.as_dijkstra().is_some());

        assert_eq!(
            script.hash(),
            Hasher::<224>::hash_tagged(&V4_BYTES, 4),
            "a V4 script hashes behind tag 4"
        );

        // and the value carries its own bytes back out
        assert_eq!(
            hex::encode(script.encode()),
            hex::encode(testing::script_ref_plutus_v4(&V4_BYTES))
        );
        let encoded = script.encode();
        let round = MultiEraScriptRef::decode(Era::Dijkstra, &encoded)
            .expect("a script re-encoded here must decode again");
        assert_eq!(round.language(), ScriptLanguage::PlutusV4);
    }

    /// A native reference script answers the other way round: a script rather
    /// than bytes, and a hash tagged 0. Without this the assertions above
    /// could pass against a type that reported everything as V4.
    #[test]
    fn a_native_reference_script_is_reported_as_native() {
        let native = testing::native_script_require_guard(0x11);
        let mut e = pallas_codec::minicbor::Encoder::new(Vec::new());
        e.array(2).unwrap();
        e.u8(0).unwrap();
        e.writer_mut().extend_from_slice(&native);
        let script_ref = e.into_writer();

        let cbor = dijkstra_output_with_script(&script_ref);
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");

        let output = &tx.outputs()[0];
        let script = output.any_script_ref().expect("readable");

        assert_eq!(script.language(), ScriptLanguage::Native);
        assert_eq!(script.language().tag(), 0);
        assert!(script.plutus_bytes().is_none());

        let inner = script.native_script().expect("a native script");
        assert_eq!(inner.era(), Era::Dijkstra);
        assert_eq!(hex::encode(inner.encode()), hex::encode(&native));

        assert_eq!(
            script.hash(),
            Hasher::<224>::hash_tagged(&native, 0),
            "a native script hashes its own CBOR behind tag 0"
        );
    }

    /// An output with no reference script answers `None`, so the readings
    /// above come from the output rather than from an accessor that always
    /// finds one.
    #[test]
    fn an_output_with_no_reference_script_reports_none() {
        let output = {
            let mut e = pallas_codec::minicbor::Encoder::new(Vec::new());
            e.array(2).unwrap();
            e.bytes(&ADDRESS).unwrap();
            e.u64(2_000_000).unwrap();
            e.into_writer()
        };
        let cbor = testing::dijkstra_block_tx(
            &testing::body_with_output(&output),
            &testing::empty_witness_set(),
            None,
            true,
        );
        let tx = MultiEraTx::decode_for_era(Era::Dijkstra, &cbor).expect("must decode");

        assert!(tx.outputs()[0].any_script_ref().is_none());
        assert!(tx.outputs()[0].datum().is_none());
        assert_eq!(tx.outputs()[0].value().coin(), 2_000_000);
    }

    /// An era with no reference script rule is refused rather than answered
    /// with an empty script.
    #[test]
    fn an_era_with_no_reference_script_rule_is_refused() {
        let bytes = testing::script_ref_plutus_v4(&V4_BYTES);

        for era in [
            Era::Byron,
            Era::Shelley,
            Era::Allegra,
            Era::Mary,
            Era::Alonzo,
        ] {
            assert!(
                MultiEraScriptRef::decode(era, &bytes).is_err(),
                "{era} has no reference script rule"
            );
        }

        // and Conway's rule stops at variant 3, so it refuses a V4 script
        assert!(MultiEraScriptRef::decode(Era::Conway, &bytes).is_err());
    }
}
