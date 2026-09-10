use std::borrow::Cow;
use std::ops::Deref;

use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{alonzo, babbage, byron, dijkstra};

use crate::{Era, Error, MultiEraHeader, OriginalHash, wellknown::GenesisValues};

impl<'b> MultiEraHeader<'b> {
    /// Decode a header given the era tag of the chainsync header envelope.
    ///
    /// Note that this tag is **not** the block wrapper tag. The wrapper counts
    /// Byron twice, once for the epoch boundary block and once for the main
    /// block, so from Shelley onward the envelope tag is one less than the
    /// wrapper tag: Babbage is wrapper 6 and envelope 5, Conway is wrapper 7
    /// and envelope 6, Dijkstra is wrapper 8 and envelope 7.
    pub fn decode(tag: u8, subtag: Option<u8>, cbor: &'b [u8]) -> Result<Self, Error> {
        match tag {
            0 => match subtag {
                Some(0) => {
                    let header = minicbor::decode(cbor).map_err(Error::invalid_cbor)?;
                    Ok(MultiEraHeader::EpochBoundary(Cow::Owned(header)))
                }
                _ => {
                    let header = minicbor::decode(cbor).map_err(Error::invalid_cbor)?;
                    Ok(MultiEraHeader::Byron(Cow::Owned(header)))
                }
            },
            1..=4 => {
                let header = minicbor::decode(cbor).map_err(Error::invalid_cbor)?;
                Ok(MultiEraHeader::ShelleyCompatible(Cow::Owned(header)))
            }
            5 | 6 => {
                let header = minicbor::decode(cbor).map_err(Error::invalid_cbor)?;
                Ok(MultiEraHeader::BabbageCompatible(Cow::Owned(header)))
            }
            7 => {
                let header = minicbor::decode(cbor).map_err(Error::invalid_cbor)?;
                Ok(MultiEraHeader::Dijkstra(Cow::Owned(header)))
            }
            // Deliberately not a catch-all decoding as Babbage. Babbage's
            // header body is a ten field definite array and minicbor's derived
            // decoder skips whatever follows the fields it declares, so an
            // unknown era's longer header would decode without error, report a
            // correct slot, number and hash, and re-encode short. An era this
            // crate has not been taught is refused instead.
            unknown => Err(Error::UnknownEra(unknown as u16)),
        }
    }

    pub fn cbor(&self) -> &[u8] {
        match self {
            MultiEraHeader::EpochBoundary(x) => x.raw_cbor(),
            MultiEraHeader::ShelleyCompatible(x) => x.raw_cbor(),
            MultiEraHeader::BabbageCompatible(x) => x.raw_cbor(),
            MultiEraHeader::Byron(x) => x.raw_cbor(),
            MultiEraHeader::Dijkstra(x) => x.raw_cbor(),
        }
    }

    pub fn number(&self) -> u64 {
        match self {
            MultiEraHeader::EpochBoundary(x) => x
                .consensus_data
                .difficulty
                .first()
                .cloned()
                .unwrap_or_default(),
            MultiEraHeader::ShelleyCompatible(x) => x.header_body.block_number,
            MultiEraHeader::BabbageCompatible(x) => x.header_body.block_number,
            MultiEraHeader::Byron(x) => x.consensus_data.2.first().cloned().unwrap_or_default(),
            MultiEraHeader::Dijkstra(x) => x.header_body.block_number,
        }
    }

    pub fn slot(&self) -> u64 {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => x.header_body.slot,
            MultiEraHeader::BabbageCompatible(x) => x.header_body.slot,
            MultiEraHeader::EpochBoundary(x) => {
                let genesis = GenesisValues::default();
                genesis.relative_slot_to_absolute(x.consensus_data.epoch_id, 0)
            }
            MultiEraHeader::Byron(x) => {
                let genesis = GenesisValues::default();
                genesis.relative_slot_to_absolute(x.consensus_data.0.epoch, x.consensus_data.0.slot)
            }
            MultiEraHeader::Dijkstra(x) => x.header_body.slot,
        }
    }

    pub fn hash(&self) -> Hash<32> {
        match self {
            MultiEraHeader::EpochBoundary(x) => x.original_hash(),
            MultiEraHeader::ShelleyCompatible(x) => x.original_hash(),
            MultiEraHeader::BabbageCompatible(x) => x.original_hash(),
            MultiEraHeader::Byron(x) => x.original_hash(),
            MultiEraHeader::Dijkstra(x) => x.original_hash(),
        }
    }

    pub fn previous_hash(&self) -> Option<Hash<32>> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => x.header_body.prev_hash,
            MultiEraHeader::BabbageCompatible(x) => x.header_body.prev_hash,
            MultiEraHeader::EpochBoundary(x) => Some(x.prev_block),
            MultiEraHeader::Byron(x) => Some(x.prev_block),
            MultiEraHeader::Dijkstra(x) => x.header_body.prev_hash,
        }
    }

    pub fn vrf_vkey(&self) -> Option<&[u8]> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => Some(x.header_body.vrf_vkey.as_ref()),
            MultiEraHeader::BabbageCompatible(x) => Some(x.header_body.vrf_vkey.as_ref()),
            MultiEraHeader::EpochBoundary(_) => None,
            MultiEraHeader::Byron(_) => None,
            MultiEraHeader::Dijkstra(x) => Some(x.header_body.vrf_vkey.as_ref()),
        }
    }

    pub fn issuer_vkey(&self) -> Option<&[u8]> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => Some(x.header_body.issuer_vkey.as_ref()),
            MultiEraHeader::BabbageCompatible(x) => Some(x.header_body.issuer_vkey.as_ref()),
            MultiEraHeader::EpochBoundary(_) => None,
            MultiEraHeader::Byron(_) => None,
            MultiEraHeader::Dijkstra(x) => Some(x.header_body.issuer_vkey.as_ref()),
        }
    }

    pub fn leader_vrf_output(&self) -> Result<Vec<u8>, Error> {
        match self {
            MultiEraHeader::EpochBoundary(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::ShelleyCompatible(x) => Ok(x.header_body.leader_vrf.0.to_vec()),
            MultiEraHeader::BabbageCompatible(x) => Ok(x.header_body.leader_vrf_output()),
            MultiEraHeader::Byron(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::Dijkstra(x) => Ok(x.header_body.leader_vrf_output()),
        }
    }

    pub fn nonce_vrf_output(&self) -> Result<Vec<u8>, Error> {
        match self {
            MultiEraHeader::EpochBoundary(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::ShelleyCompatible(x) => Ok(x.header_body.nonce_vrf.0.to_vec()),
            MultiEraHeader::BabbageCompatible(x) => Ok(x.header_body.nonce_vrf_output()),
            MultiEraHeader::Byron(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::Dijkstra(x) => Ok(x.header_body.nonce_vrf_output()),
        }
    }

    /// Whether this block's body carries a Leios certificate.
    /// `None` for every era before Dijkstra, which has no such field.
    pub fn block_body_contains_leios_cert(&self) -> Option<bool> {
        match self {
            MultiEraHeader::Dijkstra(x) => Some(x.header_body.block_body_contains_leios_cert),
            _ => None,
        }
    }

    /// The endorser block this header announces, if it announces one. `None`
    /// both for eras with no announcement field and for a Dijkstra header
    /// whose announcement is nil, so a caller that needs to tell those apart
    /// should match on the era first.
    pub fn eb_announcement(&self) -> Option<&dijkstra::EbAnnouncement> {
        match self {
            MultiEraHeader::Dijkstra(x) => match &x.header_body.eb_announcement {
                pallas_primitives::Nullable::Some(a) => Some(a),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_eb(&self) -> Option<&byron::EbbHead> {
        match self {
            MultiEraHeader::EpochBoundary(x) => Some(x.deref().deref()),
            _ => None,
        }
    }

    pub fn as_byron(&self) -> Option<&byron::BlockHead> {
        match self {
            MultiEraHeader::Byron(x) => Some(x.deref().deref()),
            _ => None,
        }
    }

    pub fn as_alonzo(&self) -> Option<&alonzo::Header> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => Some(x.deref().deref()),
            _ => None,
        }
    }

    pub fn as_babbage(&self) -> Option<&babbage::Header> {
        match self {
            MultiEraHeader::BabbageCompatible(x) => Some(x.deref()),
            _ => None,
        }
    }

    pub fn as_dijkstra(&self) -> Option<&dijkstra::Header> {
        match self {
            MultiEraHeader::Dijkstra(x) => Some(x.deref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MultiEraBlock;

    fn dijkstra_header_bytes() -> Vec<u8> {
        let block_str = include_str!("../../test_data/dijkstra-w36-2.block");
        let cbor = hex::decode(block_str).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        block.header().cbor().to_vec()
    }

    /// MUST FIRE: envelope tag 7 gives a Dijkstra header, and both fields the
    /// twelve field body adds are reachable.
    ///
    /// Both read as their empty value on this chain, and `Some(false)` is the
    /// assertion that matters: an era with no such field answers `None`, so
    /// the two cases stay distinguishable.
    #[test]
    fn dijkstra_header_decodes_as_dijkstra() {
        let raw = dijkstra_header_bytes();
        let header = MultiEraHeader::decode(7, None, &raw).unwrap();

        assert!(matches!(header, MultiEraHeader::Dijkstra(_)));
        assert!(header.as_dijkstra().is_some());
        assert_eq!(header.block_body_contains_leios_cert(), Some(false));
        assert!(header.eb_announcement().is_none());
        assert_eq!(header.cbor(), raw.as_slice());
    }

    /// MUST NOT FIRE: the same bytes under Conway's envelope tag must not come
    /// back as a Dijkstra header, and must not quietly succeed as a Babbage
    /// one either. This is the pair that stops era detection being decorative.
    #[test]
    fn dijkstra_header_is_not_reachable_through_the_conway_tag() {
        let raw = dijkstra_header_bytes();

        let as_conway = MultiEraHeader::decode(6, None, &raw).unwrap();
        assert!(
            !matches!(as_conway, MultiEraHeader::Dijkstra(_)),
            "tag 6 must not produce a Dijkstra header"
        );
        assert_eq!(
            as_conway.block_body_contains_leios_cert(),
            None,
            "a header decoded through the Conway tag cannot report Leios fields"
        );
    }

    /// An era tag this crate has not been taught must be refused rather than
    /// decoded as Babbage.
    #[test]
    fn unknown_era_tag_is_refused() {
        let raw = dijkstra_header_bytes();

        for tag in [8u8, 9, 200] {
            assert!(
                MultiEraHeader::decode(tag, None, &raw).is_err(),
                "era tag {tag} must be refused, not decoded as Babbage"
            );
        }
    }

    /// A Conway header must still decode through its own tag, so the change
    /// above did not narrow an era that already worked.
    #[test]
    fn conway_header_still_decodes() {
        let block_str = include_str!("../../test_data/conway1.block");
        let cbor = hex::decode(block_str).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        let raw = block.header().cbor().to_vec();

        let header = MultiEraHeader::decode(6, None, &raw).unwrap();
        assert!(matches!(header, MultiEraHeader::BabbageCompatible(_)));
        assert_eq!(header.block_body_contains_leios_cert(), None);
    }
}
