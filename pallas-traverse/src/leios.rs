//! The Leios endorsement layer, from a follower's side of the wire.
//!
//! On a Leios network most transactions never appear in a ranking block. A
//! ranking block header announces an endorser block, a later ranking block
//! certifies that announcement, and the certified endorser block carries the
//! transactions. A follower that reads only the ranking chain builds a ledger
//! that is silently short.
//!
//! The endorser block body is a network object rather than a ledger one. Its
//! rule is
//!
//! ```cddl
//! endorser_block = { * hash => word32 }
//! ```
//!
//! from the node to node `leios-fetch` CDDL of cardano-blueprint on the
//! `leios-prototype` branch, `src/network/node-to-node/leios-fetch/messages.cddl`,
//! quoted verbatim in the conformance test in
//! `pallas-network2/src/protocol/leiosfetch.rs`. It is absent from the Dijkstra
//! ledger CDDL, which carries only `leios_announcement`, `leios_certificate`
//! and the two header fields.
//!
//! Three properties of that wire form each cost a follower its ledger if it
//! guesses, so every one of them is carried in a type here rather than left to
//! a caller.
//!
//! 1. The protocol has no not found reply. An endorser block the peer does not
//!    hold comes back as `a0`, a well formed empty body, indistinguishable from
//!    one that committed no transactions. The announcement commits the body
//!    length before any fetch, so [`EndorserBlockBody::decode_announced`] is
//!    the constructor that pairs the two and refuses a length the announcement
//!    did not promise.
//! 2. The body keys a transaction by the blake2b-256 of the whole transaction,
//!    not by its transaction id, which is the blake2b-256 of the body alone. A
//!    follower that looks for transaction ids finds nothing, with no error.
//! 3. Each transaction arrives wrapped in a CBOR byte string, and the length in
//!    the body is the length of the unwrapped transaction.
//!
//! The certification rule needs nothing but ranking chain headers. Walk them in
//! order, carry the most recent announcement forward, and when a header sets
//! `leios_certified` the carried announcement is the endorser block to fetch and
//! apply at that header's block. An announcement superseded before any block
//! certifies it is abandoned network wide. [`CertificationTracker`] is that
//! walk.

use pallas_codec::minicbor::{Decoder, Encoder, data::Type};
use pallas_crypto::hash::{Hash, Hasher};
use pallas_primitives::dijkstra;

use crate::{Era, MultiEraHeader, MultiEraTx};

/// Everything that can go wrong between an announcement and an applied
/// endorser block. Every variant names the evidence, so no caller has to infer
/// a failure from a value that merely came back empty.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("endorser block body is not a cbor map of hash to size: {0}")]
    InvalidBody(String),

    #[error(
        "endorser block body is {found} bytes, the announcement committed to {announced}; \
         leios-fetch has no not-found reply, so a short body is a missing endorser block"
    )]
    BodySize { announced: u32, found: usize },

    #[error("endorser block names {named} transactions and {delivered} were delivered")]
    TxCount { named: usize, delivered: usize },

    #[error("transaction {index} is not wrapped in a cbor byte string: {reason}")]
    Envelope { index: usize, reason: String },

    #[error("transaction {index} is {found} bytes, the endorser block names {named}")]
    TxSize {
        index: usize,
        named: u32,
        found: usize,
    },

    #[error("transaction {index} hashes to {found}, the endorser block names {named}")]
    TxHash {
        index: usize,
        named: Hash<32>,
        found: Hash<32>,
    },

    #[error("transaction {index} does not decode as a Dijkstra transaction: {reason}")]
    TxDecode { index: usize, reason: String },

    #[error("the header at slot {slot} certifies an endorser block with no announcement pending")]
    CertifiesNothing { slot: u64 },

    #[error(
        "the block at slot {slot} certifies an endorser block and carries {count} transactions \
         of its own, which the chain inclusion rule forbids"
    )]
    CertifiesAndCarries { slot: u64, count: usize },
}

/// One entry of an endorser block body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlockEntry {
    /// blake2b-256 of the whole transaction, which is not its transaction id.
    pub hash: Hash<32>,
    /// Byte length of the transaction with its byte string envelope removed.
    pub size: u32,
}

/// The body of an endorser block: the transactions it commits to, in the order
/// the wire delivered them, which is the order the ledger applies them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlockBody {
    entries: Vec<EndorserBlockEntry>,
}

impl EndorserBlockBody {
    /// Decodes a body with nothing to check it against.
    ///
    /// Prefer [`Self::decode_announced`]. This constructor cannot tell an
    /// endorser block that committed no transactions from one the peer does not
    /// hold, because the wire gives both answers as `a0`.
    pub fn decode(cbor: &[u8]) -> Result<Self, Error> {
        let mut d = Decoder::new(cbor);
        let mut entries = Vec::new();

        let declared = d
            .map()
            .map_err(|e| Error::InvalidBody(format!("map header: {e}")))?;

        let read = |d: &mut Decoder| -> Result<EndorserBlockEntry, Error> {
            let key = d
                .bytes()
                .map_err(|e| Error::InvalidBody(format!("entry key: {e}")))?;
            let hash: [u8; 32] = key.try_into().map_err(|_| {
                Error::InvalidBody(format!("entry key is {} bytes, not 32", key.len()))
            })?;
            let size = d
                .u32()
                .map_err(|e| Error::InvalidBody(format!("entry size: {e}")))?;

            Ok(EndorserBlockEntry {
                hash: Hash::from(hash),
                size,
            })
        };

        match declared {
            Some(n) => {
                for _ in 0..n {
                    entries.push(read(&mut d)?);
                }
            }
            None => loop {
                match d.datatype() {
                    Ok(Type::Break) => {
                        d.skip()
                            .map_err(|e| Error::InvalidBody(format!("break: {e}")))?;
                        break;
                    }
                    Ok(_) => entries.push(read(&mut d)?),
                    Err(e) => return Err(Error::InvalidBody(format!("entry datatype: {e}"))),
                }
            },
        }

        if d.position() != cbor.len() {
            return Err(Error::InvalidBody(format!(
                "{} trailing bytes after the map",
                cbor.len() - d.position()
            )));
        }

        Ok(Self { entries })
    }

    /// Decodes a body fetched for a specific announcement, refusing any body
    /// whose length the announcement did not commit to.
    pub fn decode_announced(
        cbor: &[u8],
        announcement: &dijkstra::LeiosAnnouncement,
    ) -> Result<Self, Error> {
        if cbor.len() as u64 != announcement.announced_eb_size as u64 {
            return Err(Error::BodySize {
                announced: announcement.announced_eb_size,
                found: cbor.len(),
            });
        }

        Self::decode(cbor)
    }

    /// The entries in wire order.
    pub fn entries(&self) -> &[EndorserBlockEntry] {
        &self.entries
    }

    /// How many transactions this endorser block commits to.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Re-encodes the body to the bytes the wire carried.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = Encoder::new(&mut out);

        e.map(self.entries.len() as u64).expect("write to a vec");
        for entry in &self.entries {
            e.bytes(entry.hash.as_ref()).expect("write to a vec");
            e.u32(entry.size).expect("write to a vec");
        }

        out
    }

    /// Pairs the delivered transactions with the entries that name them.
    ///
    /// `wire` is the leios-fetch values in body order, each still wrapped in its
    /// CBOR byte string. Every transaction is checked against the entry naming
    /// it, on count, length and hash, and then decoded as a Dijkstra
    /// transaction, so the returned list is either the whole endorser block or
    /// an error naming the transaction that failed.
    pub fn transactions<'b>(&self, wire: &'b [Vec<u8>]) -> Result<Vec<MultiEraTx<'b>>, Error> {
        if wire.len() != self.entries.len() {
            return Err(Error::TxCount {
                named: self.entries.len(),
                delivered: wire.len(),
            });
        }

        let mut out = Vec::with_capacity(wire.len());

        for (index, (entry, delivered)) in self.entries.iter().zip(wire.iter()).enumerate() {
            let inner = unwrap_tx(delivered).map_err(|reason| Error::Envelope { index, reason })?;

            if inner.len() as u64 != entry.size as u64 {
                return Err(Error::TxSize {
                    index,
                    named: entry.size,
                    found: inner.len(),
                });
            }

            let found = Hasher::<256>::hash(inner);
            if found != entry.hash {
                return Err(Error::TxHash {
                    index,
                    named: entry.hash,
                    found,
                });
            }

            let tx =
                MultiEraTx::decode_for_era(Era::Dijkstra, inner).map_err(|e| Error::TxDecode {
                    index,
                    reason: e.to_string(),
                })?;

            out.push(tx);
        }

        Ok(out)
    }
}

/// Removes the CBOR byte string envelope a leios-fetch transaction arrives in.
///
/// The length the endorser block body records is the length of the transaction
/// inside the envelope, so the envelope is invisible to the body's accounting
/// and a caller that forgets it fails every hash and every size check at once.
pub fn unwrap_tx(wire: &[u8]) -> Result<&[u8], String> {
    let mut d = Decoder::new(wire);

    let inner = d.bytes().map_err(|e| e.to_string())?;

    if d.position() != wire.len() {
        return Err(format!(
            "{} trailing bytes after the byte string",
            wire.len() - d.position()
        ));
    }

    Ok(inner)
}

/// An announcement carried forward by [`CertificationTracker`], and the point a
/// leios-fetch request needs.
///
/// The announcement itself has no slot in it. The slot a fetch point wants is
/// the slot of the ranking block that made the announcement, which is what the
/// node's own store keys the endorser block by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncedEndorserBlock {
    /// Slot of the ranking block that announced it.
    pub slot: u64,
    pub hash: Hash<32>,
    /// Byte length the announcement commits the body to.
    pub size: u32,
}

/// What one ranking block header says about the endorsement layer.
///
/// Both answers are held together because a header can certify the pending
/// announcement and make a new one of its own in the same block, and a caller
/// that reads only one of the two loses either the payload or the next
/// announcement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderOutcome {
    /// The endorser block this header certifies, to be fetched and applied at
    /// this header's block.
    pub certified: Option<AnnouncedEndorserBlock>,
    /// The announcement this header makes, which some later header may certify.
    pub announced: Option<AnnouncedEndorserBlock>,
}

/// The walk over ranking chain headers that decides which endorser blocks a
/// follower must fetch, and when.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CertificationTracker {
    pending: Option<AnnouncedEndorserBlock>,
}

impl CertificationTracker {
    /// Starts a walk with a carried announcement, for a follower resuming from
    /// a stored position rather than from origin.
    pub fn resume_from(pending: Option<AnnouncedEndorserBlock>) -> Self {
        Self { pending }
    }

    /// The announcement waiting for a certificate, if any.
    pub fn pending(&self) -> Option<&AnnouncedEndorserBlock> {
        self.pending.as_ref()
    }

    /// Observes the next header of the ranking chain.
    ///
    /// Certification is settled before the header's own announcement is
    /// recorded, because `leios_certified` names the announcement that precedes
    /// this header, never the one this header makes.
    pub fn observe(&mut self, header: &MultiEraHeader) -> Result<HeaderOutcome, Error> {
        let mut outcome = HeaderOutcome::default();

        if header.leios_certified() == Some(true) {
            match self.pending.take() {
                Some(pending) => outcome.certified = Some(pending),
                None => {
                    return Err(Error::CertifiesNothing {
                        slot: header.slot(),
                    });
                }
            }
        }

        if let Some(announcement) = header.leios_announcement() {
            let announced = AnnouncedEndorserBlock {
                slot: header.slot(),
                hash: announcement.announced_eb,
                size: announcement.announced_eb_size,
            };

            self.pending = Some(announced.clone());
            outcome.announced = Some(announced);
        }

        Ok(outcome)
    }
}

/// Refuses a block that both certifies an endorser block and carries
/// transactions of its own.
///
/// CIP-0164 step 5 states that a ranking block contains either a certificate
/// for the endorser block announced by its predecessor or a list of
/// transactions forming a valid extension, and never both, on the ground that
/// allowing both would force a validator to build the ledger state from every
/// endorsed transaction before it could validate the block's own. So the two
/// sets have no defined order, and a follower that met a block carrying both
/// would have to guess one.
pub fn refuse_certifying_block_with_own_txs(
    slot: u64,
    certifies: bool,
    own_tx_count: usize,
) -> Result<(), Error> {
    if certifies && own_tx_count > 0 {
        return Err(Error::CertifiesAndCarries {
            slot,
            count: own_tx_count,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MultiEraBlock;

    /// Every fixture is a real endorser block of the Musashi testnet, pulled
    /// over leios-fetch from the public relay and written down exactly as the
    /// wire delivered it. The `.ebbody` file is the body, the `.ebtxs` file is
    /// one hex transaction per line in body order, each still carrying its byte
    /// string envelope.
    struct Fixture {
        name: &'static str,
        body: &'static str,
        txs: &'static str,
        /// Transactions the body names, counted off the wire bytes before any
        /// of this module ran.
        count: usize,
        /// The `announced_eb_size` of the ranking block header that announced
        /// it, read out of the node's immutable database.
        announced_size: u32,
    }

    const FIXTURES: &[Fixture] = &[
        Fixture {
            name: "dijkstra-eb1",
            body: include_str!("../../test_data/dijkstra-eb1.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb1.ebtxs"),
            count: 1,
            announced_size: 37,
        },
        Fixture {
            name: "dijkstra-eb2",
            body: include_str!("../../test_data/dijkstra-eb2.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb2.ebtxs"),
            count: 30,
            announced_size: 1082,
        },
        Fixture {
            name: "dijkstra-eb3",
            body: include_str!("../../test_data/dijkstra-eb3.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb3.ebtxs"),
            count: 425,
            announced_size: 15303,
        },
    ];

    impl Fixture {
        fn body_bytes(&self) -> Vec<u8> {
            hex::decode(self.body.trim()).expect("fixture body is hex")
        }

        fn wire_txs(&self) -> Vec<Vec<u8>> {
            self.txs
                .split_whitespace()
                .map(|l| hex::decode(l).expect("fixture tx is hex"))
                .collect()
        }

        fn announcement(&self) -> dijkstra::LeiosAnnouncement {
            dijkstra::LeiosAnnouncement {
                announced_eb: Hash::new([0; 32]),
                announced_eb_size: self.announced_size,
            }
        }
    }

    /// MUST FIRE: each body decodes to exactly the transactions the wire named,
    /// and re-encodes to the bytes the wire carried.
    #[test]
    fn bodies_decode_to_their_wire_transaction_count_and_re_encode_byte_for_byte() {
        for f in FIXTURES {
            let raw = f.body_bytes();
            let body = EndorserBlockBody::decode_announced(&raw, &f.announcement())
                .unwrap_or_else(|e| panic!("{}: {e}", f.name));

            assert_eq!(body.len(), f.count, "{} transaction count", f.name);
            assert_eq!(
                body.entries().len(),
                f.count,
                "{} entries and len disagree",
                f.name
            );
            assert_eq!(
                body.to_cbor(),
                raw,
                "{} does not re-encode to its wire bytes",
                f.name
            );
        }
    }

    /// MUST FIRE: leios-fetch answers "I do not have it" with a well formed
    /// empty body, so the only thing separating a missing endorser block from an
    /// empty one is the length the announcement committed to.
    ///
    /// MUST NOT FIRE: the real body of the same announcement is accepted.
    #[test]
    fn an_empty_reply_is_refused_against_the_announcement_that_named_it() {
        let f = &FIXTURES[2];
        let announcement = f.announcement();

        // 0xa0 is CBOR for the empty map, which is what the relay returns for an
        // endorser block it does not hold.
        let err = EndorserBlockBody::decode_announced(&[0xa0], &announcement)
            .expect_err("an empty body must not pass as this endorser block");

        match err {
            Error::BodySize { announced, found } => {
                assert_eq!(announced, 15303);
                assert_eq!(found, 1);
            }
            other => panic!("wrong refusal: {other}"),
        }

        // and the real one is not refused
        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &announcement)
            .expect("the announced body must be accepted");
        assert_eq!(body.len(), 425);
    }

    /// MUST FIRE: a body of exactly the announced length that is not a map of
    /// hash to size is refused as malformed rather than read as empty.
    #[test]
    fn a_body_of_the_right_length_but_the_wrong_shape_is_refused() {
        let announcement = dijkstra::LeiosAnnouncement {
            announced_eb: Hash::new([0; 32]),
            announced_eb_size: 3,
        };

        // a three byte cbor array, not a map
        let err = EndorserBlockBody::decode_announced(&[0x83, 0x01, 0x02], &announcement)
            .expect_err("a non-map body must be refused");

        assert!(matches!(err, Error::InvalidBody(_)), "wrong refusal: {err}");
    }

    /// MUST FIRE: the delivered transactions are the ones the body names, in
    /// body order, unwrapped from their envelopes and decoded as Dijkstra.
    #[test]
    fn transactions_are_the_named_ones_in_body_order() {
        for f in FIXTURES {
            let raw = f.body_bytes();
            let body = EndorserBlockBody::decode_announced(&raw, &f.announcement()).unwrap();
            let wire = f.wire_txs();

            let txs = body
                .transactions(&wire)
                .unwrap_or_else(|e| panic!("{}: {e}", f.name));

            assert_eq!(txs.len(), f.count, "{} assembled count", f.name);

            for tx in &txs {
                assert_eq!(tx.era(), Era::Dijkstra, "{} era", f.name);
            }
        }
    }

    /// MUST FIRE: the transaction an earlier investigation named, at the index
    /// it named, with the output the ranking chain at slot 382536 spends.
    ///
    /// The endorser block keys it by the blake2b-256 of the whole transaction,
    /// `9f3ce353...`, while its transaction id is `fffa4361...`. Both are
    /// asserted here, because a follower that confuses them finds nothing and
    /// gets no error.
    #[test]
    fn the_producing_transaction_of_the_block_that_stalled_is_at_index_391() {
        let f = &FIXTURES[2];
        let raw = f.body_bytes();
        let body = EndorserBlockBody::decode_announced(&raw, &f.announcement()).unwrap();

        assert_eq!(
            body.entries()[391].hash.to_string(),
            "9f3ce353a58a5ee42503cb22c0e3379894e32b7907b67f678894855577c8f7e6",
            "the body keys a transaction by its whole hash, not its id"
        );
        assert_eq!(body.entries()[391].size, 201);

        let wire = f.wire_txs();
        let txs = body.transactions(&wire).unwrap();
        let tx = &txs[391];

        assert_eq!(
            tx.hash().to_string(),
            "fffa4361c5251f57f4840c94dcbd05164cdce9b2bcf9bbf75e2fa4baaf30cf87",
            "the transaction id differs from the endorser block key"
        );
        assert_eq!(tx.outputs().len(), 1);
        assert_eq!(tx.inputs().len(), 1);
    }

    /// MUST FIRE: a delivery in the wrong order is refused, because the body
    /// names which transaction goes where.
    #[test]
    fn a_permuted_delivery_is_refused() {
        let f = &FIXTURES[1];
        let raw = f.body_bytes();
        let body = EndorserBlockBody::decode_announced(&raw, &f.announcement()).unwrap();

        let mut wire = f.wire_txs();
        wire.swap(0, 1);

        let err = body
            .transactions(&wire)
            .expect_err("a permuted delivery must be refused");

        assert!(matches!(err, Error::TxHash { index: 0, .. }), "{err}");
    }

    /// MUST FIRE: a short delivery is refused rather than yielding the prefix
    /// that did arrive.
    #[test]
    fn a_short_delivery_is_refused() {
        let f = &FIXTURES[1];
        let raw = f.body_bytes();
        let body = EndorserBlockBody::decode_announced(&raw, &f.announcement()).unwrap();

        let mut wire = f.wire_txs();
        wire.pop();

        let err = body
            .transactions(&wire)
            .expect_err("a short delivery must be refused");

        match err {
            Error::TxCount { named, delivered } => {
                assert_eq!(named, 30);
                assert_eq!(delivered, 29);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// MUST FIRE: a transaction handed over without its byte string envelope is
    /// refused, which is the shape a caller that skipped the unwrap produces.
    #[test]
    fn a_transaction_delivered_without_its_envelope_is_refused() {
        let f = &FIXTURES[0];
        let raw = f.body_bytes();
        let body = EndorserBlockBody::decode_announced(&raw, &f.announcement()).unwrap();

        let wire = f.wire_txs();
        let inner = unwrap_tx(&wire[0])
            .expect("the fixture is enveloped")
            .to_vec();
        assert_eq!(inner.len(), 229);
        assert_ne!(inner.len(), wire[0].len(), "the envelope is real");

        let err = body
            .transactions(&[inner])
            .expect_err("an unwrapped transaction must be refused");

        assert!(matches!(err, Error::Envelope { index: 0, .. }), "{err}");
    }

    fn header_of(block_str: &str) -> Vec<u8> {
        let cbor = hex::decode(block_str.trim()).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        block.header().cbor().to_vec()
    }

    fn dijkstra_header(raw: &[u8]) -> MultiEraHeader<'_> {
        MultiEraHeader::decode(7, None, raw).unwrap()
    }

    /// MUST FIRE: a header that certifies with nothing pending is refused. A
    /// follower that answered this with "nothing to fetch" would skip a whole
    /// endorser block and never know.
    #[test]
    fn a_header_that_certifies_nothing_pending_is_refused() {
        let raw = header_of(include_str!("../../test_data/dijkstra1.block"));
        let header = dijkstra_header(&raw);
        assert_eq!(header.leios_certified(), Some(true), "fixture precondition");

        let mut tracker = CertificationTracker::default();
        let err = tracker
            .observe(&header)
            .expect_err("certifying with nothing pending must be refused");

        match err {
            Error::CertifiesNothing { slot } => assert_eq!(slot, 2514319),
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// MUST FIRE and MUST NOT FIRE together: the walk carries the most recent
    /// announcement forward, yields it at the header that certifies it, holds
    /// certification and announcement together when one header does both, and
    /// abandons an announcement superseded before any block certified it.
    ///
    /// The headers are four real Musashi blocks in chain order.
    #[test]
    fn certification_walks_the_headers_and_abandons_a_superseded_announcement() {
        let earlier = AnnouncedEndorserBlock {
            slot: 2514000,
            hash: Hash::new([9; 32]),
            size: 1234,
        };

        let mut tracker = CertificationTracker::resume_from(Some(earlier.clone()));

        // block 110203, slot 2514319: certifies the carried announcement and
        // makes one of its own in the same header.
        let raw1 = header_of(include_str!("../../test_data/dijkstra1.block"));
        let out = tracker.observe(&dijkstra_header(&raw1)).unwrap();
        assert_eq!(out.certified.as_ref(), Some(&earlier));
        let announced1 = out.announced.expect("110203 announces");
        assert_eq!(announced1.slot, 2514319);
        assert_eq!(
            announced1.hash.to_string(),
            "2baaaf7169be390e43ec401a12f664cc01c39bfe52c7ce7a13818a2d8922eac6"
        );
        assert_eq!(announced1.size, 28519);
        assert_eq!(tracker.pending(), Some(&announced1));

        // block 110260, slot 2515420: neither certifies nor announces, and must
        // not disturb what is pending.
        let raw2 = header_of(include_str!("../../test_data/dijkstra2.block"));
        let out = tracker.observe(&dijkstra_header(&raw2)).unwrap();
        assert_eq!(out, HeaderOutcome::default());
        assert_eq!(tracker.pending(), Some(&announced1));

        // block 110365, slot 2517946: announces without certifying, so 110203's
        // announcement is abandoned and never fetched.
        let raw3 = header_of(include_str!("../../test_data/dijkstra3.block"));
        let out = tracker.observe(&dijkstra_header(&raw3)).unwrap();
        assert_eq!(out.certified, None, "110365 certifies nothing");
        let announced3 = out.announced.expect("110365 announces");
        assert_eq!(
            announced3.hash.to_string(),
            "70178a5a3a3b7b1d6169f84821c629811d1a473038535a62ac0e2d891e2b1fc7"
        );
        assert_eq!(tracker.pending(), Some(&announced3));

        // block 110597, slot 2523265: announces again, so 110365's announcement
        // is abandoned in turn.
        let raw9 = header_of(include_str!("../../test_data/dijkstra9.block"));
        let out = tracker.observe(&dijkstra_header(&raw9)).unwrap();
        assert_eq!(out.certified, None);
        let announced9 = out.announced.expect("110597 announces");
        assert_eq!(
            announced9.hash.to_string(),
            "1783474fde6bcc46e11d1008ffe060048e3ff57df51ed323806ed6a42d932851"
        );
        assert_eq!(announced9.size, 74668);
        assert_eq!(tracker.pending(), Some(&announced9));
    }

    /// MUST NOT FIRE: a pre-Leios header certifies nothing and announces
    /// nothing, and must not be read as certifying with nothing pending either.
    #[test]
    fn a_pre_leios_header_certifies_nothing() {
        let cbor = hex::decode(include_str!("../../test_data/conway1.block").trim()).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        let raw = block.header().cbor().to_vec();
        let header = MultiEraHeader::decode(6, None, &raw).unwrap();
        assert_eq!(header.leios_certified(), None, "fixture precondition");

        let mut tracker = CertificationTracker::resume_from(Some(AnnouncedEndorserBlock {
            slot: 1,
            hash: Hash::new([1; 32]),
            size: 5,
        }));

        let out = tracker.observe(&header).unwrap();
        assert_eq!(out, HeaderOutcome::default());
        assert!(tracker.pending().is_some(), "pending is left alone");
    }

    /// MUST FIRE on the one illegal combination, MUST NOT FIRE on the other
    /// three. CIP-0164 forbids a ranking block that both certifies and carries
    /// its own transactions, and no such block exists on this chain, so the
    /// only way this refusal is ever exercised is here.
    #[test]
    fn a_certifying_block_with_its_own_transactions_is_refused() {
        assert!(refuse_certifying_block_with_own_txs(10, false, 0).is_ok());
        assert!(refuse_certifying_block_with_own_txs(10, false, 426).is_ok());
        assert!(refuse_certifying_block_with_own_txs(10, true, 0).is_ok());

        let err = refuse_certifying_block_with_own_txs(10, true, 426)
            .expect_err("certifying and carrying must be refused");

        match err {
            Error::CertifiesAndCarries { slot, count } => {
                assert_eq!(slot, 10);
                assert_eq!(count, 426);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }
}
