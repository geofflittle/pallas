use std::{borrow::Cow, ops::Deref};

use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{alonzo, babbage, byron, conway};

#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;

use crate::{
    Era, Error, MultiEraBlock, MultiEraHeader, MultiEraTx, MultiEraUpdate, probe, support,
};

type BlockWrapper<T> = (u16, T);

impl<'b> MultiEraBlock<'b> {
    pub fn decode_epoch_boundary(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<byron::EbBlock> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::EpochBoundary(Box::new(block)))
    }

    pub fn decode_byron(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<byron::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::Byron(Box::new(block)))
    }

    pub fn decode_shelley(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<alonzo::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::AlonzoCompatible(Box::new(block), Era::Shelley))
    }

    pub fn decode_allegra(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<alonzo::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::AlonzoCompatible(Box::new(block), Era::Allegra))
    }

    pub fn decode_mary(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<alonzo::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::AlonzoCompatible(Box::new(block), Era::Mary))
    }

    pub fn decode_alonzo(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<alonzo::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::AlonzoCompatible(Box::new(block), Era::Alonzo))
    }

    pub fn decode_babbage(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<babbage::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::Babbage(Box::new(block)))
    }

    pub fn decode_conway(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<conway::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::Conway(Box::new(block)))
    }

    #[cfg(feature = "unstable")]
    pub fn decode_dijkstra(cbor: &'b [u8]) -> Result<Self, Error> {
        let (_, block): BlockWrapper<dijkstra::Block> =
            minicbor::decode(cbor).map_err(Error::invalid_cbor)?;

        Ok(Self::Dijkstra(Box::new(block)))
    }

    pub fn decode(cbor: &'b [u8]) -> Result<MultiEraBlock<'b>, Error> {
        match probe::block_era(cbor) {
            probe::Outcome::EpochBoundary => Self::decode_epoch_boundary(cbor),
            probe::Outcome::Matched(era) => match era {
                Era::Byron => Self::decode_byron(cbor),
                Era::Shelley => Self::decode_shelley(cbor),
                Era::Allegra => Self::decode_allegra(cbor),
                Era::Mary => Self::decode_mary(cbor),
                Era::Alonzo => Self::decode_alonzo(cbor),
                Era::Babbage => Self::decode_babbage(cbor),
                Era::Conway => Self::decode_conway(cbor),
                #[cfg(feature = "unstable")]
                Era::Dijkstra => Self::decode_dijkstra(cbor),
            },
            probe::Outcome::Inconclusive => Err(Error::unknown_cbor(cbor)),
        }
    }

    pub fn header(&self) -> MultiEraHeader<'_> {
        match self {
            MultiEraBlock::EpochBoundary(x) => {
                MultiEraHeader::EpochBoundary(Cow::Borrowed(&x.header))
            }
            MultiEraBlock::Byron(x) => MultiEraHeader::Byron(Cow::Borrowed(&x.header)),
            MultiEraBlock::AlonzoCompatible(x, _) => {
                MultiEraHeader::ShelleyCompatible(Cow::Borrowed(&x.header))
            }
            MultiEraBlock::Babbage(x) => {
                MultiEraHeader::BabbageCompatible(Cow::Borrowed(&x.header))
            }
            MultiEraBlock::Conway(x) => MultiEraHeader::BabbageCompatible(Cow::Borrowed(&x.header)),
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(x) => MultiEraHeader::Dijkstra(Cow::Borrowed(&x.header)),
        }
    }

    /// Returns the block number (aka: height)
    pub fn number(&self) -> u64 {
        self.header().number()
    }

    pub fn era(&self) -> Era {
        match self {
            MultiEraBlock::EpochBoundary(_) => Era::Byron,
            MultiEraBlock::AlonzoCompatible(_, x) => *x,
            MultiEraBlock::Babbage(_) => Era::Babbage,
            MultiEraBlock::Byron(_) => Era::Byron,
            MultiEraBlock::Conway(_) => Era::Conway,
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(_) => Era::Dijkstra,
        }
    }

    pub fn hash(&self) -> Hash<32> {
        self.header().hash()
    }

    pub fn slot(&self) -> u64 {
        self.header().slot()
    }

    /// Builds a vec with the Txs of the block
    pub fn txs(&self) -> Vec<MultiEraTx<'_>> {
        match self {
            MultiEraBlock::AlonzoCompatible(x, era) => support::clone_alonzo_txs(x)
                .into_iter()
                .map(|x| MultiEraTx::AlonzoCompatible(Box::new(Cow::Owned(x)), *era))
                .collect(),
            MultiEraBlock::Babbage(x) => support::clone_babbage_txs(x)
                .into_iter()
                .map(|x| MultiEraTx::Babbage(Box::new(Cow::Owned(x))))
                .collect(),
            MultiEraBlock::Byron(x) => support::clone_byron_txs(x)
                .into_iter()
                .map(|x| MultiEraTx::Byron(Box::new(Cow::Owned(x))))
                .collect(),
            MultiEraBlock::Conway(x) => support::clone_conway_txs(x)
                .into_iter()
                .map(|x| MultiEraTx::Conway(Box::new(Cow::Owned(x))))
                .collect(),
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(x) => support::clone_dijkstra_txs(x)
                .into_iter()
                .map(|x| MultiEraTx::Dijkstra(Box::new(Cow::Owned(x))))
                .collect(),
            MultiEraBlock::EpochBoundary(_) => vec![],
        }
    }

    /// Returns true if the there're no tx in the block
    pub fn is_empty(&self) -> bool {
        match self {
            MultiEraBlock::EpochBoundary(_) => true,
            MultiEraBlock::AlonzoCompatible(x, _) => x.transaction_bodies.is_empty(),
            MultiEraBlock::Babbage(x) => x.transaction_bodies.is_empty(),
            MultiEraBlock::Byron(x) => x.body.tx_payload.is_empty(),
            MultiEraBlock::Conway(x) => x.transaction_bodies.is_empty(),
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(x) => x.block_body.transactions.is_empty(),
        }
    }

    /// Returns the count of txs in the block
    pub fn tx_count(&self) -> usize {
        match self {
            MultiEraBlock::EpochBoundary(_) => 0,
            MultiEraBlock::AlonzoCompatible(x, _) => x.transaction_bodies.len(),
            MultiEraBlock::Babbage(x) => x.transaction_bodies.len(),
            MultiEraBlock::Byron(x) => x.body.tx_payload.len(),
            MultiEraBlock::Conway(x) => x.transaction_bodies.len(),
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(x) => x.block_body.transactions.len(),
        }
    }

    /// Returns true if the block has any auxiliary data
    pub fn has_aux_data(&self) -> bool {
        match self {
            MultiEraBlock::EpochBoundary(_) => false,
            MultiEraBlock::AlonzoCompatible(x, _) => !x.auxiliary_data_set.is_empty(),
            MultiEraBlock::Babbage(x) => !x.auxiliary_data_set.is_empty(),
            MultiEraBlock::Byron(_) => false,
            MultiEraBlock::Conway(x) => !x.auxiliary_data_set.is_empty(),
            // Dijkstra has no segregated auxiliary data set: each transaction
            // carries its own auxiliary data inline, so this asks whether any
            // of them does. Only a written value counts, so a nil slot and an
            // undefined one both read as absent.
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(x) => x
                .block_body
                .transactions
                .iter()
                .any(|tx| matches!(tx.auxiliary_data, pallas_primitives::Nullable::Some(_))),
        }
    }

    /// Returns any block-level param update proposals (byron-specific)
    pub fn update(&self) -> Option<MultiEraUpdate<'_>> {
        match self {
            MultiEraBlock::Byron(x) => {
                if let Some(up) = x.body.upd_payload.proposal.deref() {
                    let epoch = x.header.consensus_data.0.epoch;
                    Some(MultiEraUpdate::Byron(
                        epoch,
                        Box::new(Cow::Owned(up.clone())),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn as_alonzo(&self) -> Option<&alonzo::Block<'_>> {
        match self {
            MultiEraBlock::AlonzoCompatible(x, _) => Some(x),
            _ => None,
        }
    }

    pub fn as_babbage(&self) -> Option<&babbage::Block<'_>> {
        match self {
            MultiEraBlock::Babbage(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_byron(&self) -> Option<&byron::Block<'_>> {
        match self {
            MultiEraBlock::Byron(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_conway(&self) -> Option<&conway::Block<'_>> {
        match self {
            MultiEraBlock::Conway(x) => Some(x),
            _ => None,
        }
    }

    #[cfg(feature = "unstable")]
    pub fn as_dijkstra(&self) -> Option<&dijkstra::Block<'_>> {
        match self {
            MultiEraBlock::Dijkstra(x) => Some(x),
            _ => None,
        }
    }

    /// The Leios certificate carried by a Dijkstra block body, if it carries
    /// one. `None` for every earlier era, which has no such field.
    #[cfg(feature = "unstable")]
    pub fn leios_certificate(&self) -> Option<&dijkstra::LeiosCertificate> {
        match self {
            MultiEraBlock::Dijkstra(x) => match &x.block_body.leios_certificate {
                pallas_primitives::Nullable::Some(c) => Some(c),
                _ => None,
            },
            _ => None,
        }
    }

    /// The Peras certificate carried by a Dijkstra block body, if it carries
    /// one. `None` for every earlier era.
    #[cfg(feature = "unstable")]
    pub fn peras_certificate(&self) -> Option<&dijkstra::PerasCertificate> {
        match self {
            MultiEraBlock::Dijkstra(x) => match &x.block_body.peras_certificate {
                pallas_primitives::Nullable::Some(c) => Some(c),
                _ => None,
            },
            _ => None,
        }
    }

    /// Return the size of the serialised block in bytes
    pub fn size(&self) -> usize {
        match self {
            MultiEraBlock::EpochBoundary(b) => minicbor::to_vec(b).unwrap().len(),
            MultiEraBlock::Byron(b) => minicbor::to_vec(b).unwrap().len(),
            MultiEraBlock::AlonzoCompatible(b, _) => minicbor::to_vec(b).unwrap().len(),
            MultiEraBlock::Babbage(b) => minicbor::to_vec(b).unwrap().len(),
            MultiEraBlock::Conway(b) => minicbor::to_vec(b).unwrap().len(),
            #[cfg(feature = "unstable")]
            MultiEraBlock::Dijkstra(b) => minicbor::to_vec(b).unwrap().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iteration() {
        let blocks = vec![
            (include_str!("../../test_data/byron2.block"), 2usize),
            (include_str!("../../test_data/shelley1.block"), 4),
            (include_str!("../../test_data/mary1.block"), 14),
            (include_str!("../../test_data/allegra1.block"), 3),
            (include_str!("../../test_data/alonzo1.block"), 5),
        ];

        for (block_str, tx_count) in blocks.into_iter() {
            let cbor = hex::decode(block_str).expect("invalid hex");
            let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");
            assert_eq!(block.txs().len(), tx_count);
        }
    }

    fn dijkstra_block(hex_str: &str) -> Vec<u8> {
        hex::decode(hex_str).expect("invalid hex")
    }

    /// Without the era compiled in, a Dijkstra block is bytes this crate does
    /// not know, and it is refused with the error stock pallas gives any block
    /// whose wrapper tag it cannot place.
    ///
    /// The gate is only worth having if this holds. An era number that decodes
    /// with no types behind it, or a Dijkstra block quietly read as Conway,
    /// would be the half state the gate exists to prevent.
    #[cfg(not(feature = "unstable"))]
    #[test]
    fn a_dijkstra_block_is_refused_without_the_feature() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra1.block"));

        let err = MultiEraBlock::decode(&cbor)
            .expect_err("a Dijkstra block must not decode without the unstable feature");

        assert!(
            matches!(err, Error::UnknownCbor(_)),
            "expected the unknown-cbor refusal, found {err:?}"
        );
    }

    /// The pair to it: a Conway block from the same entry point still decodes,
    /// so the refusal above is the era tag speaking and not a decoder that
    /// refuses everything.
    #[cfg(not(feature = "unstable"))]
    #[test]
    fn a_conway_block_still_decodes_without_the_feature() {
        let cbor = dijkstra_block(include_str!("../../test_data/conway5.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Conway);
    }

    /// A Dijkstra block reaches the multi-era entry point as Dijkstra, and the
    /// header that comes back out of it is a Dijkstra header rather than a
    /// Babbage one.
    ///
    /// The fixture is the first Dijkstra block of the chain, block 4255 at
    /// slot 86463. Both Leios fields read as their empty value, because no
    /// block on this chain has been given a non empty one yet. That is a
    /// decoded value, not a skipped field, and the byte level round trip in
    /// `pallas-primitives` is what proves the bytes were there to read.
    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_block_decodes_as_dijkstra() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra1.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert!(block.as_dijkstra().is_some());
        assert!(block.as_conway().is_none());
        assert!(block.header().as_dijkstra().is_some());
        assert!(block.header().as_babbage().is_none());

        assert_eq!(block.tx_count(), 0);
        assert!(block.is_empty());
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(false));
        assert!(block.header().eb_announcement().is_none());
        assert!(block.leios_certificate().is_none());
        assert!(block.peras_certificate().is_none());
    }

    /// The block immediately before it is Conway, and must come back as Conway
    /// from the same entry point. Without this pair the test above could pass
    /// against an entry point that called everything Dijkstra.
    #[cfg(feature = "unstable")]
    #[test]
    fn the_block_before_the_fork_is_conway() {
        let cbor = dijkstra_block(include_str!("../../test_data/conway5.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Conway);
        assert!(block.as_conway().is_some());
        assert!(block.as_dijkstra().is_none());

        // The entry point dispatches on the wrapper tag, so the assertion
        // above cannot see a Dijkstra body decoder that accepted a Conway
        // body. This is the one that can.
        assert!(
            MultiEraBlock::decode_dijkstra(&cbor).is_err(),
            "a Conway block body must be refused by the Dijkstra type"
        );

        // and a Conway block has no field to answer these with at all, which
        // is a different answer from a Dijkstra block that answers false
        assert_eq!(block.header().block_body_contains_leios_cert(), None);
        assert!(block.leios_certificate().is_none());
    }

    /// A Dijkstra block taken apart, given the two body certificates and an
    /// invalid transaction, and put back together.
    ///
    /// Every Dijkstra block on this chain reads `nil` for both certificate
    /// slots and `true` for every validity flag, so those three accessors are
    /// otherwise pinned to a value the chain happens to have. The header and
    /// the transaction bodies are the node's bytes and only the three fields
    /// under test are written here.
    #[cfg(feature = "unstable")]
    fn hand_built_block(leios: bool, peras: bool, valid: bool) -> Vec<u8> {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra3.block"));
        let (_, mut block): (u16, dijkstra::Block) =
            minicbor::decode(&cbor).expect("the fixture must decode");

        if leios {
            block.block_body.leios_certificate =
                pallas_primitives::Nullable::Some(dijkstra::LeiosCertificate {
                    signers: vec![0x0f; 4].into(),
                    signature: vec![0x11; 48].into(),
                });
        }

        if peras {
            block.block_body.peras_certificate =
                pallas_primitives::Nullable::Some(vec![0x22; 16].into());
        }

        let mut txs = block.block_body.transactions.to_vec();
        for tx in txs.iter_mut() {
            tx.success = valid;
        }
        block.block_body.transactions = pallas_codec::utils::MaybeIndefArray::Def(txs);

        minicbor::to_vec((8u16, &block)).expect("to_vec is infallible")
    }

    /// The populated half of the block body pair. Both certificate slots
    /// answer with what was written, and the validity flag answers false.
    ///
    /// Hand built, and still the only reading of two of these three shapes.
    /// A real fixture now carries the Leios certificate, and
    /// `a_block_that_carries_a_leios_certificate_reports_it` reads it from
    /// the chain. No block on this chain carries a Peras certificate, whose
    /// slot is reserved and nil on every one of the 14452 Dijkstra blocks
    /// scanned, and no transaction on it reports itself invalid, so those
    /// two readings exist only here.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_block_body_that_carries_certificates_reports_them() {
        let cbor = hand_built_block(true, true, false);
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);

        let leios = block
            .leios_certificate()
            .expect("a written Leios certificate must be readable");
        assert_eq!(leios.signature.len(), 48);
        assert_eq!(leios.signers.as_ref(), [0x0f; 4]);

        let peras = block
            .peras_certificate()
            .expect("a written Peras certificate must be readable");
        assert_eq!(peras.len(), 16);

        assert_eq!(block.tx_count(), 1);
        for tx in block.txs() {
            assert!(
                !tx.is_valid(),
                "the flag written as false must be read as false"
            );
        }
    }

    /// The same builder writing the empty values comes back empty, so the
    /// test above reads what was written rather than what the builder always
    /// writes, and the accessors are not answering a constant either way.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_block_body_that_carries_no_certificates_reports_none() {
        let cbor = hand_built_block(false, false, true);
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert!(block.leios_certificate().is_none());
        assert!(block.peras_certificate().is_none());
        for tx in block.txs() {
            assert!(tx.is_valid());
        }
    }

    /// The Leios certificate read off the chain rather than written here.
    ///
    /// The block level slot and the header level flag are two independent
    /// statements about the same block, and a reader that took either from
    /// the other would pass on a block where they agree. dijkstra11.block is
    /// the block where both say no, so the pair is read in both directions.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_block_that_carries_a_leios_certificate_reports_it() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra8.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(true));

        let leios = block
            .leios_certificate()
            .expect("dijkstra8 carries a Leios certificate");
        assert_eq!(
            hex::encode(leios.signers.as_slice()),
            "f218a17300000e08200200"
        );
        assert_eq!(
            leios.signature.len(),
            48,
            "a BLS12-381 signature is 48 bytes"
        );

        // Peras is reserved and nil on every block of this chain, so the two
        // slots of the same body answer differently and the reading above is
        // of the first slot rather than of whatever the body holds.
        assert!(block.peras_certificate().is_none());

        // A second certificate, different bytes, so neither is a constant.
        let other = dijkstra_block(include_str!("../../test_data/dijkstra9.block"));
        let other = MultiEraBlock::decode(&other).expect("invalid cbor");
        let second = other
            .leios_certificate()
            .expect("dijkstra9 carries one too");
        assert_ne!(second.signers, leios.signers);
        assert_ne!(second.signature, leios.signature);

        // And the block where the header flag and the body slot both say no.
        let plain = dijkstra_block(include_str!("../../test_data/dijkstra11.block"));
        let plain = MultiEraBlock::decode(&plain).expect("invalid cbor");
        assert_eq!(plain.header().block_body_contains_leios_cert(), Some(false));
        assert!(plain.leios_certificate().is_none());
    }

    /// Auxiliary data is the one optional field this chain populates, so it is
    /// the one presence and absence pair the fixtures give for free.
    ///
    /// `dijkstra4.block` is the first block with auxiliary data and
    /// `dijkstra3.block` is the plain transfer beside it.
    #[cfg(feature = "unstable")]
    #[test]
    fn auxiliary_data_is_reported_only_where_a_block_carries_it() {
        let with = dijkstra_block(include_str!("../../test_data/dijkstra4.block"));
        let with = MultiEraBlock::decode(&with).expect("invalid cbor");
        assert!(with.has_aux_data(), "this fixture carries body key 7");
        let txs = with.txs();
        let meta = txs[0].metadata();
        let labels = meta.collect::<Vec<_>>();
        assert!(!labels.is_empty(), "and the metadata must be readable");

        let without = dijkstra_block(include_str!("../../test_data/dijkstra3.block"));
        let without = MultiEraBlock::decode(&without).expect("invalid cbor");
        assert!(!without.has_aux_data());
        let txs = without.txs();
        let meta = txs[0].metadata();
        assert!(meta.collect::<Vec<_>>().is_empty());
    }

    /// The output path over every Dijkstra fixture that carries one.
    ///
    /// Twelve outputs across the seven fixtures, all of them the legacy array
    /// form, so this is what the fixtures can prove about `outputs()`. The
    /// post Alonzo form, the datum option and the script reference have no
    /// case on this chain and are covered where they are built by hand.
    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_outputs_carry_an_address_and_a_value() {
        let cases = [
            (include_str!("../../test_data/dijkstra2.block"), 1usize),
            (include_str!("../../test_data/dijkstra3.block"), 1),
            (include_str!("../../test_data/dijkstra4.block"), 1),
            (include_str!("../../test_data/dijkstra5.block"), 4),
            (include_str!("../../test_data/dijkstra6.block"), 4),
            (include_str!("../../test_data/dijkstra7.block"), 1),
            (include_str!("../../test_data/dijkstra10.block"), 7),
            (include_str!("../../test_data/dijkstra11.block"), 1),
            (include_str!("../../test_data/dijkstra12.block"), 24),
            (include_str!("../../test_data/dijkstra13.block"), 1),
        ];

        let mut seen = 0usize;
        for (block_str, outputs) in cases {
            let cbor = dijkstra_block(block_str);
            let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

            let txs = block.txs();
            let all: Vec<_> = txs.iter().flat_map(|tx| tx.outputs()).collect();
            assert_eq!(all.len(), outputs, "output count");

            for output in all.iter() {
                assert_eq!(output.era(), Era::Dijkstra);
                assert!(output.as_dijkstra().is_some());
                output.address().expect("every output address must parse");
                assert!(output.value().coin() > 0, "every output holds lovelace");
                assert!(output.value().assets().is_empty());
                assert!(
                    output.datum().is_none(),
                    "no output on this chain carries a datum"
                );
                assert!(
                    output.any_script_ref().is_none(),
                    "no output on this chain carries a reference script"
                );
                seen += 1;
            }
        }

        assert_eq!(seen, 45, "the fixtures hold forty five Dijkstra outputs");
    }

    /// The same accessors over a post Alonzo map output, which is the form
    /// every transaction on this chain has used since slot 351096 and which
    /// no fixture reached until dijkstra10.block and dijkstra11.block.
    ///
    /// The counts are what make this worth its own test. A reader that
    /// handled only the legacy array form would decode nothing written on
    /// this chain in its last fifty thousand slots, and the test above would
    /// not say which form it had been reading.
    #[cfg(feature = "unstable")]
    #[test]
    // The deprecated narrow accessor is called on purpose, to assert what it
    // answers for this era, which is the whole point of its deprecation note.
    #[allow(deprecated)]
    fn a_map_form_output_is_read_through_the_multi_era_accessors() {
        use pallas_primitives::dijkstra::TransactionOutput;

        let mut map_form = 0usize;
        let mut array_form = 0usize;

        for block_str in [
            include_str!("../../test_data/dijkstra10.block"),
            include_str!("../../test_data/dijkstra11.block"),
        ] {
            let cbor = dijkstra_block(block_str);
            let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");
            let txs = block.txs();

            for output in txs.iter().flat_map(|tx| tx.outputs()) {
                match output.as_dijkstra().expect("a Dijkstra output") {
                    TransactionOutput::PostAlonzo(_) => {
                        map_form += 1;

                        // Every reading the accessors offer, over the form
                        // that had never been read from real bytes before.
                        let address = output.address().expect("a map output has an address");
                        assert!(!address.to_vec().is_empty());
                        assert!(output.value().coin() > 0);
                        assert!(output.value().assets().is_empty());
                        assert!(
                            output.datum().is_none(),
                            "the datum option is absent on this chain, and it is the map form that has one to be absent"
                        );
                        assert!(output.any_script_ref().is_none());
                        assert!(
                            output.script_ref().is_none(),
                            "the narrow accessor answers None for a Dijkstra output by design"
                        );
                    }
                    TransactionOutput::Legacy(_) => array_form += 1,
                }
            }
        }

        // Both forms, so the readings above are of the map form rather than
        // of whatever these two blocks happened to hold.
        assert_eq!(
            (map_form, array_form),
            (6, 2),
            "dijkstra10 carries five map outputs and two legacy ones, dijkstra11 one map output"
        );
    }

    /// The transaction bearing Dijkstra fixtures reach their transactions
    /// through the multi-era accessors, with the counts the provenance file
    /// records, and every transaction reports itself valid.
    ///
    /// Validity is the assertion worth having here. It rides as the fourth
    /// element of each transaction in this era, so a model that dropped the
    /// element would still decode the block and would answer this wrong.
    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_blocks_yield_their_transactions() {
        let cases = [
            (include_str!("../../test_data/dijkstra3.block"), 1usize),
            (include_str!("../../test_data/dijkstra5.block"), 3),
            (include_str!("../../test_data/dijkstra6.block"), 4),
        ];

        let mut seen = 0usize;
        for (block_str, tx_count) in cases.iter() {
            let cbor = dijkstra_block(block_str);
            let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

            assert_eq!(block.era(), Era::Dijkstra);
            assert_eq!(block.tx_count(), *tx_count);
            assert_eq!(block.txs().len(), *tx_count);

            for tx in block.txs() {
                assert!(tx.is_valid(), "every transaction on this chain is valid");
                assert!(!tx.inputs().is_empty());
                assert!(tx.fee().is_some());
                seen += 1;
            }
        }

        assert_eq!(seen, 8);
    }

    /// A plain transfer reports none of the optional bodies. Its transaction
    /// body carries keys 0, 1 and 2 and nothing else, so every accessor below
    /// has a real absence to report rather than an unread field. The pair to
    /// this is the test below.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_plain_dijkstra_block_reports_no_redeemers_or_withdrawals() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra3.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert_eq!(block.tx_count(), 1);

        for tx in block.txs() {
            assert!(tx.redeemers().is_empty());
            assert!(tx.withdrawals_sorted_set().is_empty());
            assert!(tx.certs().is_empty());
            assert!(tx.sub_transactions().is_empty());
        }
    }

    /// The same accessors do find things when a transaction has them, so the
    /// absences above are the fixture speaking and not the accessor returning
    /// a constant.
    ///
    /// The certificate comes from a Dijkstra block. The redeemers have to come
    /// from a Babbage fixture, because no Dijkstra block on this chain carries
    /// a script witness of any kind, and neither does any Conway block that
    /// decodes without the `relaxed` feature.
    #[cfg(feature = "unstable")]
    #[test]
    fn the_same_accessors_find_what_is_there() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra2.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        let certs: usize = block.txs().iter().map(|tx| tx.certs().len()).sum();
        assert!(
            certs > 0,
            "this fixture carries transaction body key 4, so certs() must find one"
        );

        let cbor = dijkstra_block(include_str!("../../test_data/babbage6.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Babbage);
        let redeemers: usize = block.txs().iter().map(|tx| tx.redeemers().len()).sum();
        assert!(
            redeemers > 0,
            "this fixture carries witness set key 5, so redeemers() must find one"
        );
    }
}
