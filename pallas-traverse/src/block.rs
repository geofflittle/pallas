use std::{borrow::Cow, ops::Deref};

use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{alonzo, babbage, byron, conway, dijkstra};

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
            // of them does.
            MultiEraBlock::Dijkstra(x) => x
                .block_body
                .transactions
                .iter()
                .any(|tx| !matches!(tx.auxiliary_data, pallas_primitives::Nullable::Null)),
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

    pub fn as_dijkstra(&self) -> Option<&dijkstra::Block<'_>> {
        match self {
            MultiEraBlock::Dijkstra(x) => Some(x),
            _ => None,
        }
    }

    /// The Leios certificate carried by a Dijkstra block body, if it carries
    /// one. `None` for every earlier era, which has no such field.
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

    /// A Dijkstra block reaches the multi-era entry point as Dijkstra, and the
    /// header that comes back out of it is a Dijkstra header rather than a
    /// Babbage one.
    #[test]
    fn dijkstra_block_decodes_as_dijkstra() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra1.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert!(block.as_dijkstra().is_some());
        assert!(block.as_conway().is_none());
        assert!(block.header().as_dijkstra().is_some());
        assert!(block.header().as_babbage().is_none());

        // this fixture is an empty block that carries both Leios fields
        assert_eq!(block.tx_count(), 0);
        assert!(block.is_empty());
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(true));
        assert!(block.header().eb_announcement().is_some());
        assert!(block.leios_certificate().is_some());
        assert!(block.peras_certificate().is_none());
    }

    /// The batch block, and the accessors the brief names.
    ///
    /// The transaction at index 1 is the twenty one order batch: it spends
    /// twenty one script inputs, each with its own spend redeemer, and carries
    /// a single withdrawal of zero. Every count here is read off the wire
    /// bytes independently before being asserted, so none of these assertions
    /// can pass by nothing firing.
    #[test]
    fn batch_block_yields_21_script_inputs_and_one_zero_withdrawal() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra7.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert_eq!(block.tx_count(), 426);

        let txs = block.txs();
        let batch = &txs[1];

        assert_eq!(
            batch.hash().to_string(),
            "577c7dd3b9c0a92e7d89faeebd1299cef4dbbbf8f5dddd8431ce53ca3e5c2e69"
        );

        // 21 spend redeemers, at indices 0 through 20
        let mut spend_indexes: Vec<u32> = batch
            .redeemers()
            .iter()
            .filter(|r| r.tag() == pallas_primitives::dijkstra::RedeemerTag::Spend)
            .map(|r| r.index())
            .collect();
        spend_indexes.sort_unstable();

        assert_eq!(spend_indexes.len(), 21, "expected 21 script inputs");
        assert_eq!(spend_indexes, (0..21).collect::<Vec<u32>>());

        // one reward redeemer for the withdrawal, so 22 redeemers in total
        assert_eq!(batch.redeemers().len(), 22);

        // exactly one withdrawal, and it is zero
        let withdrawals: Vec<(&[u8], u64)> = batch.withdrawals_sorted_set();
        assert_eq!(withdrawals.len(), 1, "expected exactly one withdrawal");
        assert_eq!(withdrawals[0].1, 0, "the withdrawal must be zero");

        // the inputs the redeemers point at are really there
        assert_eq!(batch.inputs().len(), 22);
        assert!(batch.is_valid());
    }

    /// MUST NOT FIRE: the same accessors on a block that has none of these
    /// things report none of them. Without this, the test above could pass
    /// against an accessor that returned a constant.
    #[test]
    fn a_plain_dijkstra_block_reports_no_redeemers_or_withdrawals() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra4.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        assert_eq!(block.era(), Era::Dijkstra);
        assert_eq!(block.tx_count(), 2);

        for tx in block.txs() {
            assert!(tx.redeemers().is_empty());
            assert!(tx.withdrawals_sorted_set().is_empty());
            assert!(tx.certs().is_empty());
        }

        // and this block carries neither Leios field, where dijkstra1 carries both
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(false));
        assert!(block.header().eb_announcement().is_none());
        assert!(block.leios_certificate().is_none());
    }

    /// The one certificate in the fixture set, and the one `guards` value,
    /// both of which exercise types that no earlier era can represent.
    #[test]
    fn dijkstra_certificates_and_guards_are_reachable() {
        let cbor = dijkstra_block(include_str!("../../test_data/dijkstra9.block"));
        let block = MultiEraBlock::decode(&cbor).expect("invalid cbor");

        let guards: Vec<_> = block
            .txs()
            .into_iter()
            .filter_map(|tx| tx.required_signers().as_dijkstra().cloned())
            .collect();

        assert_eq!(
            guards.len(),
            1,
            "dijkstra9 carries exactly one guards value"
        );

        // it is the credential arm, which a Conway required-signers decoder
        // typed as a set of key hashes cannot read at all
        assert!(matches!(
            guards[0],
            pallas_primitives::dijkstra::Guards::Credentials(_)
        ));
    }
}
