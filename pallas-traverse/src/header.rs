use std::borrow::Cow;
use std::ops::Deref;

use pallas_codec::minicbor;
use pallas_crypto::hash::Hash;
use pallas_primitives::{alonzo, babbage, byron};

#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;

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
            #[cfg(feature = "unstable")]
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

    /// The era whose header rule carries this value.
    ///
    /// The Shelley header shape serves Shelley through Alonzo and the Babbage
    /// shape serves Babbage and Conway, so those two variants name the first
    /// era of their group rather than the era the header came from.
    pub fn era(&self) -> Era {
        match self {
            MultiEraHeader::EpochBoundary(_) => Era::Byron,
            MultiEraHeader::Byron(_) => Era::Byron,
            MultiEraHeader::ShelleyCompatible(_) => Era::Shelley,
            MultiEraHeader::BabbageCompatible(_) => Era::Babbage,
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(_) => Era::Dijkstra,
        }
    }

    pub fn cbor(&self) -> &[u8] {
        match self {
            MultiEraHeader::EpochBoundary(x) => x.raw_cbor(),
            MultiEraHeader::ShelleyCompatible(x) => x.raw_cbor(),
            MultiEraHeader::BabbageCompatible(x) => x.raw_cbor(),
            MultiEraHeader::Byron(x) => x.raw_cbor(),
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
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
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => x.header_body.slot,
        }
    }

    pub fn hash(&self) -> Hash<32> {
        match self {
            MultiEraHeader::EpochBoundary(x) => x.original_hash(),
            MultiEraHeader::ShelleyCompatible(x) => x.original_hash(),
            MultiEraHeader::BabbageCompatible(x) => x.original_hash(),
            MultiEraHeader::Byron(x) => x.original_hash(),
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => x.original_hash(),
        }
    }

    pub fn previous_hash(&self) -> Option<Hash<32>> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => x.header_body.prev_hash,
            MultiEraHeader::BabbageCompatible(x) => x.header_body.prev_hash,
            MultiEraHeader::EpochBoundary(x) => Some(x.prev_block),
            MultiEraHeader::Byron(x) => Some(x.prev_block),
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => x.header_body.prev_hash,
        }
    }

    pub fn vrf_vkey(&self) -> Option<&[u8]> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => Some(x.header_body.vrf_vkey.as_ref()),
            MultiEraHeader::BabbageCompatible(x) => Some(x.header_body.vrf_vkey.as_ref()),
            MultiEraHeader::EpochBoundary(_) => None,
            MultiEraHeader::Byron(_) => None,
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => Some(x.header_body.vrf_vkey.as_ref()),
        }
    }

    pub fn issuer_vkey(&self) -> Option<&[u8]> {
        match self {
            MultiEraHeader::ShelleyCompatible(x) => Some(x.header_body.issuer_vkey.as_ref()),
            MultiEraHeader::BabbageCompatible(x) => Some(x.header_body.issuer_vkey.as_ref()),
            MultiEraHeader::EpochBoundary(_) => None,
            MultiEraHeader::Byron(_) => None,
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => Some(x.header_body.issuer_vkey.as_ref()),
        }
    }

    pub fn leader_vrf_output(&self) -> Result<Vec<u8>, Error> {
        match self {
            MultiEraHeader::EpochBoundary(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::ShelleyCompatible(x) => Ok(x.header_body.leader_vrf.0.to_vec()),
            MultiEraHeader::BabbageCompatible(x) => Ok(x.header_body.leader_vrf_output()),
            MultiEraHeader::Byron(_) => Err(Error::InvalidEra(Era::Byron)),
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => Ok(x.header_body.leader_vrf_output()),
        }
    }

    pub fn nonce_vrf_output(&self) -> Result<Vec<u8>, Error> {
        match self {
            MultiEraHeader::EpochBoundary(_) => Err(Error::InvalidEra(Era::Byron)),
            MultiEraHeader::ShelleyCompatible(x) => Ok(x.header_body.nonce_vrf.0.to_vec()),
            MultiEraHeader::BabbageCompatible(x) => Ok(x.header_body.nonce_vrf_output()),
            MultiEraHeader::Byron(_) => Err(Error::InvalidEra(Era::Byron)),
            #[cfg(feature = "unstable")]
            MultiEraHeader::Dijkstra(x) => Ok(x.header_body.nonce_vrf_output()),
        }
    }

    /// Whether this block's body carries a Leios certificate.
    /// `None` for every era before Dijkstra, which has no such field.
    #[cfg(feature = "unstable")]
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
    #[cfg(feature = "unstable")]
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

    #[cfg(feature = "unstable")]
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

    fn header_of(block_str: &str) -> Vec<u8> {
        let cbor = hex::decode(block_str).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        block.header().cbor().to_vec()
    }

    #[cfg(feature = "unstable")]
    fn dijkstra_header_bytes() -> Vec<u8> {
        header_of(include_str!("../../test_data/dijkstra1.block"))
    }

    /// Envelope tag 7 gives a Dijkstra header, and both fields the twelve
    /// field body adds are reachable.
    ///
    /// Both read as their empty value on this chain, and `Some(false)` is the
    /// assertion that matters: an era with no such field answers `None`, so
    /// the two cases stay distinguishable.
    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_header_decodes_as_dijkstra() {
        let raw = dijkstra_header_bytes();
        let header = MultiEraHeader::decode(7, None, &raw).unwrap();

        assert!(matches!(header, MultiEraHeader::Dijkstra(_)));
        assert_eq!(header.era(), Era::Dijkstra);
        assert!(header.as_dijkstra().is_some());
        assert_eq!(header.block_body_contains_leios_cert(), Some(false));
        assert!(header.eb_announcement().is_none());
        assert_eq!(header.cbor(), raw.as_slice());
    }

    /// The populated half of the pair above, from the first block on the
    /// chain that ever carried it. The announcement is a real endorser block
    /// reference and its size is the one the node's own Leios database
    /// records for that hash at that slot.
    ///
    /// Without a block that sets them, both accessors could return their
    /// empty value as a constant and every other test here would pass.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_header_that_announces_an_endorser_block_says_so() {
        let raw = header_of(include_str!("../../test_data/dijkstra8.block"));
        let header = MultiEraHeader::decode(7, None, &raw).unwrap();

        assert_eq!(header.era(), Era::Dijkstra);
        assert_eq!(header.block_body_contains_leios_cert(), Some(true));

        let announcement = header
            .eb_announcement()
            .expect("dijkstra8 announces an endorser block");
        assert_eq!(announcement.eb_size, 39_495);
        assert_eq!(
            hex::encode(announcement.eb_hash),
            "de5f4b812d0e6dc3129510c6663de4ea99bbd6bf019ec2d541853242926fd446"
        );
    }

    /// The two fields travel separately, and the chain sets them separately,
    /// so each accessor is read against a block where the other says the
    /// opposite. Without this pair a reading of either could be coming from
    /// the other field.
    #[cfg(feature = "unstable")]
    #[test]
    fn the_certificate_flag_and_the_announcement_are_read_separately() {
        // Certifies, announces nothing.
        let raw = header_of(include_str!("../../test_data/dijkstra9.block"));
        let header = MultiEraHeader::decode(7, None, &raw).unwrap();
        assert_eq!(header.block_body_contains_leios_cert(), Some(true));
        assert!(
            header.eb_announcement().is_none(),
            "dijkstra9 certifies without announcing"
        );

        // Neither.
        let raw = header_of(include_str!("../../test_data/dijkstra11.block"));
        let header = MultiEraHeader::decode(7, None, &raw).unwrap();
        assert_eq!(header.block_body_contains_leios_cert(), Some(false));
        assert!(header.eb_announcement().is_none());
    }

    /// An era with no such field answers `None` to the flag, which is what
    /// keeps `Some(false)` above meaning "this block does not certify" rather
    /// than "nobody asked".
    ///
    /// The announcement accessor cannot draw that line: it answers `None`
    /// both here and for dijkstra9.block, which does have the field and
    /// leaves it nil. Those are two different facts with one answer, and a
    /// caller that needs to tell them apart has to match on the era first.
    #[cfg(feature = "unstable")]
    #[test]
    fn an_earlier_era_header_has_no_leios_fields_at_all() {
        let raw = header_of(include_str!("../../test_data/conway1.block"));
        let header = MultiEraHeader::decode(6, None, &raw).unwrap();

        assert_eq!(header.era(), Era::Babbage);
        assert_eq!(
            header.block_body_contains_leios_cert(),
            None,
            "a ten field header has no certificate flag to report"
        );
        assert!(header.eb_announcement().is_none());

        // The same `None` that dijkstra9.block's populated era produces.
        let dijkstra = header_of(include_str!("../../test_data/dijkstra9.block"));
        let dijkstra = MultiEraHeader::decode(7, None, &dijkstra).unwrap();
        assert!(dijkstra.eb_announcement().is_none());
        assert_ne!(
            dijkstra.block_body_contains_leios_cert(),
            header.block_body_contains_leios_cert(),
            "the flag is what separates the two, and it does"
        );
    }

    /// A header a live node sent, decoded under the envelope tag it arrived
    /// with.
    ///
    /// The tag is the whole of era detection on the header path and nothing
    /// else on the wire repeats it, so it is measured rather than assumed.
    /// This header came off a node-to-node chainsync against the Musashi
    /// prototype node, network magic 164, which sent it under envelope tag 7.
    /// Its provenance is recorded beside the block fixtures.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_header_a_node_sent_decodes_under_the_tag_it_arrived_with() {
        let raw =
            hex::decode(include_str!("../../test_data/dijkstra-header-envelope.hex")).unwrap();

        let header = MultiEraHeader::decode(7, None, &raw).unwrap();

        assert_eq!(header.era(), Era::Dijkstra);
        assert_eq!(header.number(), 16273);
        assert_eq!(header.slot(), 340473);
        assert_eq!(
            header.hash().to_string(),
            "e1ab9eadcf98f83671eed6f0fbd1c947676a5d358a7e4132c29ecd7ca5ec7b46"
        );
        assert_eq!(header.block_body_contains_leios_cert(), Some(false));
    }

    /// The same bytes under Conway's envelope tag must not come back as a
    /// Dijkstra header, and must not quietly succeed as a Babbage one either.
    /// This is the pair that stops era detection being decorative.
    #[cfg(feature = "unstable")]
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
        let raw = header_of(include_str!("../../test_data/conway1.block"));

        for tag in [8u8, 9, 200] {
            assert!(
                MultiEraHeader::decode(tag, None, &raw).is_err(),
                "era tag {tag} must be refused, not decoded as Babbage"
            );
        }
    }

    /// Envelope tag 7 is Dijkstra's only where the era is compiled in. Without
    /// the feature it is a tag this crate does not know, and it has to be
    /// refused like any other, or the gate would leave a tag behind that
    /// decodes into the wrong era's header type.
    #[cfg(not(feature = "unstable"))]
    #[test]
    fn envelope_tag_seven_is_refused_without_the_feature() {
        let raw = header_of(include_str!("../../test_data/conway1.block"));

        assert!(
            MultiEraHeader::decode(7, None, &raw).is_err(),
            "envelope tag 7 must be refused when the Dijkstra era is not compiled in"
        );
    }

    /// A Conway header must still decode through its own tag, so the change
    /// above did not narrow an era that already worked.
    #[test]
    fn conway_header_still_decodes() {
        let raw = header_of(include_str!("../../test_data/conway1.block"));

        let header = MultiEraHeader::decode(6, None, &raw).unwrap();
        assert!(matches!(header, MultiEraHeader::BabbageCompatible(_)));
        assert_eq!(header.era(), Era::Babbage);
        #[cfg(feature = "unstable")]
        assert_eq!(header.block_body_contains_leios_cert(), None);
    }

    /// Every variant answers `era()`, so a consumer that matched a header and
    /// found a variant it does not name can still say which era it caught.
    #[test]
    fn every_header_variant_names_its_era() {
        #[allow(unused_mut)]
        let mut cases = vec![
            (
                0u8,
                header_of(include_str!("../../test_data/byron1.block")),
                Era::Byron,
            ),
            (
                2,
                header_of(include_str!("../../test_data/shelley1.block")),
                Era::Shelley,
            ),
            (
                6,
                header_of(include_str!("../../test_data/conway1.block")),
                Era::Babbage,
            ),
        ];
        #[cfg(feature = "unstable")]
        cases.push((7, dijkstra_header_bytes(), Era::Dijkstra));

        for (tag, raw, era) in cases {
            assert_eq!(
                MultiEraHeader::decode(tag, None, &raw).unwrap().era(),
                era,
                "envelope tag {tag}"
            );
        }
    }
}
