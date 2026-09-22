//! Leios endorser blocks as a follower receives them over leios-fetch.
//!
//! A ranking block that certifies an endorser block contains none of its
//! transactions, so a follower fetches the endorser block and puts its
//! transactions into the certifying block.

use std::borrow::Cow;

use pallas_codec::minicbor::{self, Decoder};
use pallas_codec::utils::MaybeIndefArray;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_primitives::dijkstra;

use crate::{Era, MultiEraBlock, MultiEraHeader, MultiEraTx};

/// Why an endorser block, a certificate, or a certifying block was refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The body is not a definite length CBOR map of hash to size.
    #[error("endorser block body is not a cbor map of hash to size: {0}")]
    InvalidBody(String),

    /// The body length differs from the announced length.
    #[error(
        "endorser block body is {found} bytes and the announcement committed to {announced}, \
         so it is not the announced endorser block"
    )]
    BodySize { announced: u32, found: usize },

    /// The body hash differs from the announced hash.
    #[error("endorser block body hashes to {found} and the announcement named {announced}")]
    BodyHash {
        announced: Hash<32>,
        found: Hash<32>,
    },

    /// The number of delivered transactions differs from the number the body names.
    #[error("endorser block names {named} transactions and {delivered} were delivered")]
    TxCount { named: usize, delivered: usize },

    /// A delivered transaction is not a single CBOR byte string.
    #[error("transaction {index} is not wrapped in a cbor byte string: {reason}")]
    Envelope { index: usize, reason: String },

    /// A delivered transaction's length differs from the length its entry names.
    #[error("transaction {index} is {found} bytes, the endorser block names {named}")]
    TxSize {
        index: usize,
        named: u32,
        found: usize,
    },

    /// A delivered transaction's hash differs from the hash its entry names.
    #[error("transaction {index} hashes to {found}, the endorser block names {named}")]
    TxHash {
        index: usize,
        named: Hash<32>,
        found: Hash<32>,
    },

    /// A delivered transaction is not a Dijkstra mempool transaction.
    #[error("transaction {index} does not decode as a Dijkstra transaction: {reason}")]
    TxDecode { index: usize, reason: String },

    /// The header given as the parent is not the one the header names.
    #[error(
        "the header at slot {slot} names {previous:?} as its parent and the header given as its \
         parent is {parent:?}"
    )]
    NotParent {
        slot: u64,
        previous: Option<Hash<32>>,
        parent: Option<Hash<32>>,
    },

    /// A header certifies an endorser block and its parent announced none.
    #[error(
        "the header at slot {slot} certifies an endorser block and its parent header announced \
         none"
    )]
    CertifiesNothing { slot: u64 },

    /// A certifying block has transactions of its own.
    #[error(
        "the block at slot {slot} certifies an endorser block and has {count} transactions of \
         its own"
    )]
    CertifiesAndCarries { slot: u64, count: usize },

    /// The block certifies no endorser block.
    #[error("the block at slot {slot} certifies no endorser block")]
    NotCertifying { slot: u64 },

    /// The block is from an era with no Leios certificate.
    #[error("a {era} block cannot certify an endorser block")]
    NotLeiosEra { era: Era },

    /// The block CBOR does not decode as a block.
    #[error("the block cbor is not a ranking block: {0}")]
    InvalidBlock(String),
}

/// An endorser block body as leios-fetch delivers it, with the length and
/// blake2b-256 of the bytes it was decoded from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlockBody {
    block: dijkstra::EndorserBlock,
    hash: Hash<32>,
    size: usize,
}

impl EndorserBlockBody {
    /// Decodes a body, refusing one whose length or blake2b-256 over the bytes
    /// as received differs from the announcement.
    pub fn decode_announced(
        cbor: &[u8],
        announcement: &dijkstra::EbAnnouncement,
    ) -> Result<Self, Error> {
        if cbor.len() as u64 != announcement.eb_size as u64 {
            return Err(Error::BodySize {
                announced: announcement.eb_size,
                found: cbor.len(),
            });
        }

        let found = Hasher::<256>::hash(cbor);
        if found != announcement.eb_hash {
            return Err(Error::BodyHash {
                announced: announcement.eb_hash,
                found,
            });
        }

        let mut d = Decoder::new(cbor);

        let block = d.decode().map_err(|e| Error::InvalidBody(e.to_string()))?;

        if d.position() != cbor.len() {
            return Err(Error::InvalidBody(format!(
                "{} trailing bytes after the map",
                cbor.len() - d.position()
            )));
        }

        Ok(Self {
            block,
            hash: found,
            size: cbor.len(),
        })
    }

    /// Refuses `announced` unless it names this body's length and hash.
    fn check_announced(&self, announced: &AnnouncedEndorserBlock) -> Result<(), Error> {
        if self.size as u64 != announced.size as u64 {
            return Err(Error::BodySize {
                announced: announced.size,
                found: self.size,
            });
        }

        if self.hash != announced.hash {
            return Err(Error::BodyHash {
                announced: announced.hash,
                found: self.hash,
            });
        }

        Ok(())
    }

    /// The hash and length of each named transaction, in wire order.
    pub fn entries(&self) -> &[(Hash<32>, u32)] {
        &self.block
    }

    /// The number of transactions the body names.
    pub fn len(&self) -> usize {
        self.block.len()
    }

    /// Whether the body names no transactions.
    pub fn is_empty(&self) -> bool {
        self.block.is_empty()
    }

    /// Checks each enveloped transaction against its entry and decodes it.
    pub fn transactions<'b>(&self, wire: &'b [Vec<u8>]) -> Result<Vec<MultiEraTx<'b>>, Error> {
        let txs = self.checked_transactions(wire)?;

        Ok(txs
            .into_iter()
            .map(|tx| MultiEraTx::Dijkstra(Box::new(Cow::Owned(tx))))
            .collect())
    }

    fn checked_transactions<'b>(
        &self,
        wire: &'b [Vec<u8>],
    ) -> Result<Vec<dijkstra::BlockTransaction<'b>>, Error> {
        if wire.len() != self.block.len() {
            return Err(Error::TxCount {
                named: self.block.len(),
                delivered: wire.len(),
            });
        }

        let mut out = Vec::with_capacity(wire.len());

        for (index, ((named_hash, named_size), delivered)) in
            self.block.iter().zip(wire.iter()).enumerate()
        {
            let inner = unwrap_tx(delivered).map_err(|reason| Error::Envelope { index, reason })?;

            if inner.len() as u64 != *named_size as u64 {
                return Err(Error::TxSize {
                    index,
                    named: *named_size,
                    found: inner.len(),
                });
            }

            let found = Hasher::<256>::hash(inner);
            if found != *named_hash {
                return Err(Error::TxHash {
                    index,
                    named: *named_hash,
                    found,
                });
            }

            out.push(decode_closure_transaction(index, inner)?);
        }

        Ok(out)
    }
}

/// Removes the CBOR byte string envelope a leios-fetch transaction arrives in.
fn unwrap_tx(wire: &[u8]) -> Result<&[u8], String> {
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

/// An endorser block transaction is a ledger `mempool_transaction`, never the block form.
fn decode_closure_transaction(
    index: usize,
    inner: &[u8],
) -> Result<dijkstra::BlockTransaction<'_>, Error> {
    let mut d = Decoder::new(inner);

    let mempool: dijkstra::MempoolTransaction = d.decode().map_err(|e| Error::TxDecode {
        index,
        reason: e.to_string(),
    })?;

    Ok(mempool.to_block_transaction())
}

/// An announced endorser block, with the point a leios-fetch request needs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnnouncedEndorserBlock {
    /// Hash of the announcing header.
    pub header: Hash<32>,
    /// Slot of the announcing header, the first element of a leios-fetch point.
    pub slot: u64,
    /// The body hash the announcement commits to.
    pub hash: Hash<32>,
    /// The body length the announcement commits to.
    pub size: u32,
}

/// What one ranking block header certifies and announces, which can be both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct HeaderOutcome {
    /// The parent's announcement, which this header certifies.
    pub certified: Option<AnnouncedEndorserBlock>,
    /// The announcement this header makes.
    pub announced: Option<AnnouncedEndorserBlock>,
}

/// What a ranking header certifies and announces. A certificate names the
/// announcement of the header's parent and no earlier one, and `parent` is
/// `None` for a header that follows genesis.
pub fn certification(
    parent: Option<&MultiEraHeader>,
    header: &MultiEraHeader,
) -> Result<HeaderOutcome, Error> {
    let parent_hash = parent.map(|p| p.hash());

    if header.previous_hash() != parent_hash {
        return Err(Error::NotParent {
            slot: header.slot(),
            previous: header.previous_hash(),
            parent: parent_hash,
        });
    }

    let certified = if header.block_body_contains_leios_cert() == Some(true) {
        let announced = parent
            .and_then(announcement_of)
            .ok_or(Error::CertifiesNothing {
                slot: header.slot(),
            })?;

        Some(announced)
    } else {
        None
    };

    Ok(HeaderOutcome {
        certified,
        announced: announcement_of(header),
    })
}

fn announcement_of(header: &MultiEraHeader) -> Option<AnnouncedEndorserBlock> {
    header
        .eb_announcement()
        .map(|announcement| AnnouncedEndorserBlock {
            header: header.hash(),
            slot: header.slot(),
            hash: announcement.eb_hash,
            size: announcement.eb_size,
        })
}

/// Rewrites a certifying ranking block so its transactions are those
/// delivered for `body`. Refuses a `certified` announcement made by any header
/// but the block's parent, a `body` it does not name, and a delivered
/// transaction that differs from its entry.
pub fn resolve_certified_block(
    block_cbor: &[u8],
    certified: &AnnouncedEndorserBlock,
    body: &EndorserBlockBody,
    wire: &[Vec<u8>],
) -> Result<Vec<u8>, Error> {
    let block =
        MultiEraBlock::decode(block_cbor).map_err(|e| Error::InvalidBlock(e.to_string()))?;

    if block.era() != Era::Dijkstra {
        return Err(Error::NotLeiosEra { era: block.era() });
    }

    if block.header().block_body_contains_leios_cert() != Some(true) {
        return Err(Error::NotCertifying { slot: block.slot() });
    }

    if block.tx_count() > 0 {
        return Err(Error::CertifiesAndCarries {
            slot: block.slot(),
            count: block.tx_count(),
        });
    }

    let previous = block.header().previous_hash();
    if previous != Some(certified.header) {
        return Err(Error::NotParent {
            slot: block.slot(),
            previous,
            parent: Some(certified.header),
        });
    }

    body.check_announced(certified)?;

    let txs = body.checked_transactions(wire)?;

    replace_transaction_list(block_cbor, txs)
}

/// Keeps the header bytes, so the header of the result no longer commits to
/// its body.
fn replace_transaction_list<'b>(
    block_cbor: &'b [u8],
    txs: Vec<dijkstra::BlockTransaction<'b>>,
) -> Result<Vec<u8>, Error> {
    let (era, mut block): (u16, dijkstra::Block) =
        minicbor::decode(block_cbor).map_err(|e| Error::InvalidBlock(e.to_string()))?;

    block.block_body.transactions = MaybeIndefArray::Def(txs);

    Ok(minicbor::to_vec((era, block)).expect("write to a vec"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pallas_codec::minicbor::Encoder;

    /// One endorser block of `test_data/dijkstra-fixtures.md`, with the values
    /// recorded for it there.
    struct Fixture {
        name: &'static str,
        body: &'static str,
        txs: &'static str,
        count: usize,
        first_tx_id: &'static str,
        header: &'static str,
        slot: u64,
        header_hash: &'static str,
    }

    const FIXTURES: &[Fixture] = &[
        Fixture {
            name: "dijkstra-eb1",
            body: include_str!("../../test_data/dijkstra-eb1.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb1.ebtxs"),
            count: 1,
            first_tx_id: "a2a3715bac697e28991d003c62a0c280fb91af40944f7bbcee27d972ad0f0a08",
            header: include_str!("../../test_data/dijkstra-eb1.header"),
            slot: 429789,
            header_hash: "abba50f39b31ca7ed67ebe72f588668073a66090a33504a76d9abbc1d6a9d3b5",
        },
        Fixture {
            name: "dijkstra-eb2",
            body: include_str!("../../test_data/dijkstra-eb2.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb2.ebtxs"),
            count: 30,
            first_tx_id: "1839e14ac4327a8a8f6c00d2bfbdb0b4a95bb6dc92d84d3a45d3ff9ada7964c2",
            header: include_str!("../../test_data/dijkstra-eb2.header"),
            slot: 397855,
            header_hash: "779e95c2816db83f41528b1b8260034f68c8f817c4f32edadc05de0fc16f22fb",
        },
        Fixture {
            name: "dijkstra-eb3",
            body: include_str!("../../test_data/dijkstra-eb3.ebbody"),
            txs: include_str!("../../test_data/dijkstra-eb3.ebtxs"),
            count: 425,
            first_tx_id: "1c60a3e90174e349a40a302742de803692d763dc906c51c1c21e253d841d555c",
            header: include_str!("../../test_data/dijkstra-eb3.header"),
            slot: 376369,
            header_hash: "a4c183c4234560ae182fd5e56a021f0f4b13d5019bab155a536db9e8bbebca90",
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

        /// The announcement read out of the fixture's announcing header, so no
        /// test compares a body to a digest of itself.
        fn announcement(&self) -> dijkstra::EbAnnouncement {
            let raw = hex::decode(self.header.trim()).expect("fixture header is hex");
            let header = dijkstra_header(&raw);

            assert_eq!(header.slot(), self.slot, "{} announcing slot", self.name);

            header
                .eb_announcement()
                .unwrap_or_else(|| panic!("{} header announces nothing", self.name))
                .clone()
        }
    }

    pub(super) fn fixture(index: usize) -> (EndorserBlockBody, Vec<Vec<u8>>) {
        let f = &FIXTURES[index];
        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &f.announcement())
            .unwrap_or_else(|e| panic!("{}: {e}", f.name));

        (body, f.wire_txs())
    }

    pub(super) fn encode_entries(entries: &[(Hash<32>, u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = Encoder::new(&mut out);
        e.map(entries.len() as u64).expect("write to a vec");
        for (hash, size) in entries {
            e.bytes(hash.as_ref()).expect("write to a vec");
            e.u32(*size).expect("write to a vec");
        }
        out
    }

    fn hash_of(hex_hash: &str) -> Hash<32> {
        hex_hash.parse().expect("a 32 byte hex hash")
    }

    /// Decodes a body that no fixture header announces, against an
    /// announcement of its own length and hash.
    pub(super) fn announced_as_itself(cbor: &[u8]) -> Result<EndorserBlockBody, Error> {
        EndorserBlockBody::decode_announced(
            cbor,
            &dijkstra::EbAnnouncement {
                eb_hash: Hasher::<256>::hash(cbor),
                eb_size: cbor.len() as u32,
            },
        )
    }

    pub(super) fn enveloped(inner: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        Encoder::new(&mut out).bytes(inner).expect("write to a vec");
        out
    }

    #[test]
    fn each_fixture_header_is_the_ranking_block_the_chain_stored() {
        for f in FIXTURES {
            let raw = hex::decode(f.header.trim()).expect("fixture header is hex");

            assert_eq!(
                Hasher::<256>::hash(&raw).to_string(),
                f.header_hash,
                "{} announcing header is not the block the chain knows",
                f.name
            );

            let announcement = f.announcement();
            let body = f.body_bytes();

            assert_eq!(
                announcement.eb_size as usize,
                body.len(),
                "{} announces a length its body does not have",
                f.name
            );
            assert_eq!(
                announcement.eb_hash,
                Hasher::<256>::hash(&body),
                "{} announces a digest its body does not have",
                f.name
            );
        }
    }

    #[test]
    fn each_fixture_reads_as_its_recorded_count_and_first_transaction() {
        for f in FIXTURES {
            let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &f.announcement())
                .unwrap_or_else(|e| panic!("{}: {e}", f.name));
            assert_eq!(body.len(), f.count, "{} named count", f.name);

            let wire = f.wire_txs();
            let txs = body
                .transactions(&wire)
                .unwrap_or_else(|e| panic!("{}: {e}", f.name));

            assert_eq!(txs.len(), f.count, "{} assembled count", f.name);
            assert_eq!(
                txs[0].hash().to_string(),
                f.first_tx_id,
                "{} first transaction id",
                f.name
            );
            for tx in &txs {
                assert_eq!(tx.era(), Era::Dijkstra, "{} era", f.name);
            }
        }
    }

    #[test]
    fn a_body_with_trailing_bytes_is_refused() {
        let mut raw = FIXTURES[0].body_bytes();
        raw.push(0x00);

        let err = announced_as_itself(&raw).expect_err("trailing bytes must be refused");

        assert!(matches!(err, Error::InvalidBody(_)), "wrong refusal: {err}");
    }

    /// leios-fetch answers a request for a block the peer lacks with an empty
    /// map, which only the announcement tells apart from an endorser block
    /// naming nothing.
    #[test]
    fn an_empty_reply_is_refused_against_the_announcement_that_named_it() {
        let f = &FIXTURES[2];
        let announcement = f.announcement();

        let err = EndorserBlockBody::decode_announced(&[0xa0], &announcement)
            .expect_err("an empty body must not pass as this endorser block");

        match err {
            Error::BodySize { announced, found } => {
                assert_eq!(announced, 15303);
                assert_eq!(found, 1);
            }
            other => panic!("wrong refusal: {other}"),
        }

        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &announcement)
            .expect("the announced body must be accepted");
        assert_eq!(body.len(), 425);
    }

    /// The announcement names this body's own length and digest, so only the
    /// shape is left to refuse.
    #[test]
    fn a_body_of_the_right_length_but_the_wrong_shape_is_refused() {
        let malformed = [0x83, 0x01, 0x02];

        let announcement = dijkstra::EbAnnouncement {
            eb_hash: hash_of("2f5cd8a253b845763f1d3bd7d75dbb43f20507eb4a69af58ef42ce0202082bcc"),
            eb_size: 3,
        };

        let err = EndorserBlockBody::decode_announced(&malformed, &announcement)
            .expect_err("a non-map body must be refused");

        assert!(matches!(err, Error::InvalidBody(_)), "wrong refusal: {err}");
    }

    #[test]
    fn a_body_of_the_announced_length_that_is_a_different_endorser_block_is_refused() {
        let f = &FIXTURES[0];
        let announcement = f.announcement();
        let raw = f.body_bytes();

        // Byte 3 is the first byte of the single entry's key.
        let mut other = raw.clone();
        other[3] ^= 0xff;
        assert_eq!(other.len(), raw.len(), "the mutation changed the length");
        assert_ne!(other, raw, "the mutation changed nothing");

        let err = EndorserBlockBody::decode_announced(&other, &announcement)
            .expect_err("a body that is not the announced endorser block must be refused");

        let said = err.to_string();
        assert!(
            said.contains(&announcement.eb_hash.to_string()),
            "the refusal does not name the announced hash: {said}"
        );
        assert!(
            said.contains(&Hasher::<256>::hash(&other).to_string()),
            "the refusal does not name the hash of the body that arrived: {said}"
        );

        let body = EndorserBlockBody::decode_announced(&raw, &announcement)
            .expect("the announced body must be accepted");
        assert_eq!(body.len(), f.count);
    }

    /// A reordered body has the announced length, so the hash is the only
    /// check that can refuse it.
    #[test]
    fn a_body_with_the_announced_entries_in_another_order_is_refused_on_hash() {
        let f = &FIXTURES[2];
        let announcement = f.announcement();
        let raw = f.body_bytes();

        let body = EndorserBlockBody::decode_announced(&raw, &announcement)
            .expect("the announced body must be accepted");

        let mut sorted = body.entries().to_vec();
        sorted.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        let reordered = encode_entries(&sorted);

        assert_ne!(
            reordered, raw,
            "the wire already had this body in key sorted order"
        );
        assert_eq!(
            reordered.len(),
            announcement.eb_size as usize,
            "a reordering leaves the announced length intact"
        );

        let err = EndorserBlockBody::decode_announced(&reordered, &announcement)
            .expect_err("a reordered body must be refused");

        match err {
            Error::BodyHash { announced, found } => {
                assert_eq!(announced, announcement.eb_hash);
                assert_eq!(found, Hasher::<256>::hash(&reordered));
                assert_ne!(found, announced);
            }
            other => panic!("wrong refusal: {other}"),
        }

        assert_eq!(encode_entries(body.entries()), raw);
    }

    /// The size 229 written in three bytes rather than two decodes to the same
    /// entry, and a hash over its minimal re-encoding would not match.
    #[test]
    fn the_announced_hash_is_over_the_bytes_as_received() {
        let f = &FIXTURES[0];
        let minimal = f.body_bytes();
        assert_eq!(&minimal[35..], &[0x18, 0xe5], "fixture precondition");

        let mut wide = minimal[..35].to_vec();
        wide.extend_from_slice(&[0x19, 0x00, 0xe5]);

        let announcement = dijkstra::EbAnnouncement {
            eb_hash: Hasher::<256>::hash(&wide),
            eb_size: wide.len() as u32,
        };

        let body = EndorserBlockBody::decode_announced(&wide, &announcement)
            .expect("a body must be checked as it arrived");

        let (minimal_body, wire) = fixture(0);
        assert_eq!(body.entries(), minimal_body.entries());
        assert_eq!(body.transactions(&wire).unwrap().len(), 1);

        let re_encoded = encode_entries(body.entries());
        assert_eq!(re_encoded, minimal);
        assert_ne!(Hasher::<256>::hash(&re_encoded), announcement.eb_hash);

        let err = EndorserBlockBody::decode_announced(&wide, &f.announcement())
            .expect_err("the minimal body's announcement must not pass the wide one");
        assert!(
            matches!(err, Error::BodySize { .. }),
            "wrong refusal: {err}"
        );
    }

    /// The body keys a transaction by the blake2b-256 of the whole
    /// transaction, which is not its transaction id.
    #[test]
    fn the_body_key_of_a_transaction_is_not_its_id() {
        let f = &FIXTURES[2];
        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &f.announcement()).unwrap();

        let (key, size) = body.entries()[391];
        assert_eq!(
            key.to_string(),
            "9f3ce353a58a5ee42503cb22c0e3379894e32b7907b67f678894855577c8f7e6"
        );
        assert_eq!(size, 201);

        let wire = f.wire_txs();
        let txs = body.transactions(&wire).unwrap();

        assert_eq!(
            txs[391].hash().to_string(),
            "fffa4361c5251f57f4840c94dcbd05164cdce9b2bcf9bbf75e2fa4baaf30cf87"
        );

        let MultiEraTx::Dijkstra(tx) = &txs[391] else {
            panic!("an endorser block transaction is a Dijkstra transaction");
        };
        assert_eq!(tx.transaction_body.outputs.len(), 1);
        assert_eq!(tx.transaction_body.inputs.len(), 1);
    }

    #[test]
    fn a_permuted_delivery_is_refused() {
        let (body, mut wire) = fixture(1);
        wire.swap(0, 1);

        let err = body
            .transactions(&wire)
            .expect_err("a permuted delivery must be refused");

        assert!(matches!(err, Error::TxHash { index: 0, .. }), "{err}");
    }

    /// Entries 15 and 16 of the largest body differ in size, where every entry
    /// of the 30 transaction body has the same size.
    #[test]
    fn a_delivery_with_two_sizes_swapped_is_refused_on_size() {
        let (body, mut wire) = fixture(2);
        assert_eq!(body.entries()[15].1, 201, "fixture precondition");
        assert_eq!(body.entries()[16].1, 200, "fixture precondition");

        wire.swap(15, 16);

        let err = body
            .transactions(&wire)
            .expect_err("a delivery of the wrong size must be refused");

        match err {
            Error::TxSize {
                index,
                named,
                found,
            } => {
                assert_eq!(index, 15);
                assert_eq!(named, 201);
                assert_eq!(found, 200);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    #[test]
    fn a_short_delivery_is_refused() {
        let (body, mut wire) = fixture(1);
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

    #[test]
    fn a_transaction_delivered_without_its_envelope_is_refused() {
        let (body, wire) = fixture(0);

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

    #[test]
    fn a_block_form_transaction_is_refused() {
        let (_, wire) = fixture(0);
        let inner = unwrap_tx(&wire[0]).expect("the fixture is enveloped");
        assert_eq!(inner[0], 0x83, "fixture precondition");

        let mempool: dijkstra::MempoolTransaction = minicbor::decode(inner).unwrap();
        let block_form = minicbor::to_vec(mempool.to_block_transaction()).unwrap();
        assert_eq!(block_form[0], 0x84, "the block form has four elements");

        let body_cbor =
            encode_entries(&[(Hasher::<256>::hash(&block_form), block_form.len() as u32)]);
        let body = announced_as_itself(&body_cbor).unwrap();

        let err = body
            .transactions(&[enveloped(&block_form)])
            .expect_err("a block form transaction must be refused");

        assert!(matches!(err, Error::TxDecode { index: 0, .. }), "{err}");
    }

    #[test]
    fn a_transaction_with_bytes_after_it_inside_its_envelope_is_refused() {
        let (_, wire) = fixture(0);
        let clean = unwrap_tx(&wire[0])
            .expect("the fixture is enveloped")
            .to_vec();

        let body_of =
            |inner: &[u8]| encode_entries(&[(Hasher::<256>::hash(inner), inner.len() as u32)]);

        let body = announced_as_itself(&body_of(&clean)).unwrap();
        assert_eq!(body.transactions(&[enveloped(&clean)]).unwrap().len(), 1);

        let mut trailing = clean.clone();
        trailing.push(0x00);
        let body = announced_as_itself(&body_of(&trailing)).unwrap();

        let err = body
            .transactions(&[enveloped(&trailing)])
            .expect_err("a transaction with bytes after it must be refused");

        match err {
            Error::TxDecode { index, reason } => {
                assert_eq!(index, 0);
                assert!(reason.contains("1 trailing bytes"), "{reason}");
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// Re-encodes a real block's header with the two Leios fields set, which
    /// leaves the header signature no longer covering the header body.
    pub(super) fn with_leios_header_fields(
        block_str: &str,
        certifies: bool,
        announcement: Option<dijkstra::EbAnnouncement>,
    ) -> Vec<u8> {
        let cbor = hex::decode(block_str.trim()).unwrap();

        let (start, end) = header_span(&cbor);

        let mut header: dijkstra::Header =
            minicbor::decode(&cbor[start..end]).expect("fixture header decodes");

        assert!(
            !header.header_body.block_body_contains_leios_cert,
            "the fixture must not already certify"
        );
        assert!(
            matches!(
                header.header_body.eb_announcement,
                pallas_codec::utils::Nullable::Null
            ),
            "the fixture must not already announce"
        );

        header.header_body.block_body_contains_leios_cert = certifies;
        header.header_body.eb_announcement = match announcement {
            Some(a) => pallas_codec::utils::Nullable::Some(a),
            None => pallas_codec::utils::Nullable::Null,
        };

        let rebuilt = minicbor::to_vec(&header).expect("write to a vec");

        let mut out = Vec::with_capacity(cbor.len() + rebuilt.len());
        out.extend_from_slice(&cbor[..start]);
        out.extend_from_slice(&rebuilt);
        out.extend_from_slice(&cbor[end..]);

        out
    }

    fn header_span(block_cbor: &[u8]) -> (usize, usize) {
        let mut d = Decoder::new(block_cbor);

        d.array().expect("era envelope");
        d.u16().expect("era tag");
        d.array().expect("block array");

        let start = d.position();
        d.skip().expect("header");

        (start, d.position())
    }

    pub(super) fn transaction_list_span(block_cbor: &[u8]) -> (usize, usize) {
        let mut d = Decoder::new(block_cbor);

        d.array().expect("era envelope");
        d.u16().expect("era tag");
        d.array().expect("block array");
        d.skip().expect("header");
        d.array().expect("block body array");

        let start = d.position();
        d.skip().expect("transaction list");

        (start, d.position())
    }

    pub(super) fn header_of(block_cbor: &[u8]) -> Vec<u8> {
        let block = MultiEraBlock::decode(block_cbor).unwrap();
        block.header().cbor().to_vec()
    }

    pub(super) fn dijkstra_header(raw: &[u8]) -> MultiEraHeader<'_> {
        MultiEraHeader::decode(7, None, raw).unwrap()
    }

    fn fixture_header(hex_header: &str) -> Vec<u8> {
        hex::decode(hex_header.trim()).expect("fixture header is hex")
    }

    fn block_header(hex_block: &str) -> Vec<u8> {
        header_of(&hex::decode(hex_block.trim()).expect("fixture block is hex"))
    }

    #[test]
    fn the_synthetic_header_changes_two_fields_and_nothing_else() {
        let plain = hex::decode(include_str!("../../test_data/dijkstra16.block").trim()).unwrap();
        let built = with_leios_header_fields(
            include_str!("../../test_data/dijkstra16.block"),
            true,
            Some(dijkstra::EbAnnouncement {
                eb_hash: Hash::new([7; 32]),
                eb_size: 4096,
            }),
        );

        let common = plain
            .iter()
            .zip(built.iter())
            .take_while(|(a, b)| a == b)
            .count();

        assert_eq!(
            &plain[common..common + 2],
            &[0xf4, 0xf6],
            "the fixture's own flag and null announcement are what changed"
        );
        assert_eq!(built[common], 0xf5, "the built block certifies");

        let (ps, pe) = header_span(&plain);
        let (bs, be) = header_span(&built);
        assert_eq!(ps, bs);
        assert_eq!(&plain[pe..], &built[be..], "the block body is untouched");

        let raw = header_of(&built);
        let header = dijkstra_header(&raw);
        assert_eq!(header.block_body_contains_leios_cert(), Some(true));
        assert_eq!(header.eb_announcement().map(|a| a.eb_size), Some(4096));
        assert_eq!(header.slot(), 311025, "the chain's own slot survives");

        let neither = with_leios_header_fields(
            include_str!("../../test_data/dijkstra16.block"),
            false,
            None,
        );
        assert_eq!(neither, plain, "a no-op build re-encodes byte for byte");
    }

    /// Blocks 17402 to 17406 in chain order, and 17511 and 17512, where each
    /// certifying header names the announcement of the header before it.
    #[test]
    fn each_real_certificate_names_its_parents_announcement() {
        let raw = [
            fixture_header(include_str!("../../test_data/dijkstra-17402.header")),
            block_header(include_str!("../../test_data/dijkstra8.block")),
            fixture_header(include_str!("../../test_data/dijkstra-17404.header")),
            fixture_header(include_str!("../../test_data/dijkstra-17405.header")),
            block_header(include_str!("../../test_data/dijkstra15.block")),
        ];
        let run: Vec<_> = raw.iter().map(|r| dijkstra_header(r.as_slice())).collect();

        let expected = [
            (
                "29694e6d204e586f170a1f4f75da3703cb86289d08e7eaea2def826a6dcd7e90",
                8786,
                371680,
            ),
            (
                "de5f4b812d0e6dc3129510c6663de4ea99bbd6bf019ec2d541853242926fd446",
                39495,
                371723,
            ),
            (
                "df3e00644db8fb9020057eab4d130befab8e7f2eb2a69af008116ccc8c2f0527",
                43312,
                371771,
            ),
            (
                "b0233a1c2608013cec07e933e3d0101f48dc75d785ee9b096bee94cd8713a72d",
                59439,
                371837,
            ),
        ];

        for (pair, (hash, size, slot)) in run.windows(2).zip(expected) {
            let out = certification(Some(&pair[0]), &pair[1])
                .unwrap_or_else(|e| panic!("block {}: {e}", pair[1].number()));

            let certified = out.certified.expect("every header after 17402 certifies");
            assert_eq!(certified.hash, hash_of(hash), "block {}", pair[1].number());
            assert_eq!(certified.size, size, "block {}", pair[1].number());
            assert_eq!(certified.slot, slot, "block {}", pair[1].number());
            assert_eq!(
                certified.header,
                pair[0].hash(),
                "block {}",
                pair[1].number()
            );
            assert_eq!(out.announced, announcement_of(&pair[1]));
        }

        let parent = fixture_header(include_str!("../../test_data/dijkstra-17511.header"));
        let child = block_header(include_str!("../../test_data/dijkstra9.block"));
        let out = certification(Some(&dijkstra_header(&parent)), &dijkstra_header(&child)).unwrap();

        let certified = out.certified.expect("17512 certifies");
        assert_eq!(
            certified.hash,
            hash_of("2abe14e0dd956846cd06b786b36380a608488b265790a55676af90f0ebe86201")
        );
        assert_eq!(certified.size, 24772);
        assert_eq!(certified.slot, 374267);
        assert_eq!(
            certified.header,
            hash_of("9bc3734faa48f8eae7e3a76409c2e60bc12daa37090ec2e0b538b708831008a2")
        );
        assert_eq!(out.announced, None, "17512 announces nothing");
    }

    /// No fixture block certifies after a parent that announced nothing, so
    /// block 14936 is rewritten to certify after the real block 14935.
    #[test]
    fn a_certificate_after_a_parent_that_announced_nothing_is_refused_on_a_rewritten_header() {
        let parent = block_header(include_str!("../../test_data/dijkstra16.block"));
        let parent = dijkstra_header(&parent);
        assert!(parent.eb_announcement().is_none(), "fixture precondition");

        let child = header_of(&with_leios_header_fields(
            include_str!("../../test_data/dijkstra7.block"),
            true,
            None,
        ));
        let child = dijkstra_header(&child);

        let err = certification(Some(&parent), &child)
            .expect_err("a certificate after a parent that announced nothing must be refused");

        match err {
            Error::CertifiesNothing { slot } => assert_eq!(slot, 311104),
            other => panic!("wrong refusal: {other}"),
        }
    }

    #[test]
    fn a_header_given_a_parent_it_does_not_name_is_refused() {
        let other = block_header(include_str!("../../test_data/dijkstra16.block"));
        let other = dijkstra_header(&other);
        let header = block_header(include_str!("../../test_data/dijkstra8.block"));
        let header = dijkstra_header(&header);

        let err = certification(Some(&other), &header).expect_err("a wrong parent must be refused");

        match err {
            Error::NotParent {
                slot,
                previous,
                parent,
            } => {
                assert_eq!(slot, 371723);
                assert_eq!(
                    previous,
                    Some(hash_of(
                        "b0e696b02f5c527b43eabf2149b71c7f9579b4e2db1dbe0c04fe67881cdb8aa9"
                    ))
                );
                assert_eq!(
                    parent,
                    Some(hash_of(
                        "c9d7bca094227279830e2e2110acbb965dc9e90d469ac97594d40bc8e295735c"
                    ))
                );
            }
            other => panic!("wrong refusal: {other}"),
        }

        let err = certification(None, &header).expect_err("genesis is not this header's parent");
        assert!(
            matches!(err, Error::NotParent { parent: None, .. }),
            "{err}"
        );
    }

    #[test]
    fn the_first_dijkstra_header_after_a_conway_parent_certifies_nothing() {
        let conway = hex::decode(include_str!("../../test_data/conway5.block").trim()).unwrap();
        let conway = MultiEraBlock::decode(&conway).unwrap();
        let first = hex::decode(include_str!("../../test_data/dijkstra1.block").trim()).unwrap();
        let first = MultiEraBlock::decode(&first).unwrap();

        let out = certification(Some(&conway.header()), &first.header()).unwrap();

        assert_eq!(out, HeaderOutcome::default());
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::tests::{
        announced_as_itself, dijkstra_header, encode_entries, enveloped, fixture, header_of,
        transaction_list_span, with_leios_header_fields,
    };
    use super::*;

    /// A certifying block of `test_data/dijkstra-fixtures.md`, its parent
    /// header, the endorser block that parent announced, and the ids
    /// `cardano-cli` gives that endorser block's first and last transactions.
    struct Matched {
        name: &'static str,
        block: &'static str,
        parent: &'static str,
        body: &'static str,
        txs: &'static str,
        count: usize,
        first_tx_id: &'static str,
        last_tx_id: &'static str,
    }

    const MATCHED: &[Matched] = &[
        Matched {
            name: "dijkstra8",
            block: include_str!("../../test_data/dijkstra8.block"),
            parent: include_str!("../../test_data/dijkstra-17402.header"),
            body: include_str!("../../test_data/dijkstra-17402.ebbody"),
            txs: include_str!("../../test_data/dijkstra-17402.ebtxs"),
            count: 244,
            first_tx_id: "82bfbdf62f180269e15a72672b51610bc17a043c3d6f5c5a2434403b00a49d98",
            last_tx_id: "7c165b9f5287113a811c8e6276b57ffb89fb0d9daf9bfe028310e68408c7f756",
        },
        Matched {
            name: "dijkstra9",
            block: include_str!("../../test_data/dijkstra9.block"),
            parent: include_str!("../../test_data/dijkstra-17511.header"),
            body: include_str!("../../test_data/dijkstra-17511.ebbody"),
            txs: include_str!("../../test_data/dijkstra-17511.ebtxs"),
            count: 688,
            first_tx_id: "f145a99b6ebf0bcd9b6fcc767089a8d488467a03ec10bdd5b16c479429de2674",
            last_tx_id: "1602655536f5af3f0027b1a1e8720d773636314beac3d40b47581168b3d910db",
        },
        Matched {
            name: "dijkstra15",
            block: include_str!("../../test_data/dijkstra15.block"),
            parent: include_str!("../../test_data/dijkstra-17405.header"),
            body: include_str!("../../test_data/dijkstra-17405.ebbody"),
            txs: include_str!("../../test_data/dijkstra-17405.ebtxs"),
            count: 1651,
            first_tx_id: "c46da84d6b25495f98d7802547e9bb5e0ee6fc877bb7e57e79fab29905cbe832",
            last_tx_id: "ff04b647831e3685c2142766d354aa89dd97bdc5184c37a6230b0ac7682b7d1d",
        },
    ];

    struct Resolving {
        raw: Vec<u8>,
        body_cbor: Vec<u8>,
        certified: AnnouncedEndorserBlock,
        body: EndorserBlockBody,
        wire: Vec<Vec<u8>>,
    }

    fn matched(index: usize) -> Resolving {
        let m = &MATCHED[index];
        let raw = hex::decode(m.block.trim()).unwrap();
        let parent = hex::decode(m.parent.trim()).unwrap();
        let parent = dijkstra_header(&parent);
        let header = header_of(&raw);

        let certified = certification(Some(&parent), &dijkstra_header(&header))
            .unwrap_or_else(|e| panic!("{}: {e}", m.name))
            .certified
            .unwrap_or_else(|| panic!("{} certifies", m.name));

        let body_cbor = hex::decode(m.body.trim()).unwrap();
        let announcement = parent.eb_announcement().expect("the parent announces");
        let body = EndorserBlockBody::decode_announced(&body_cbor, announcement)
            .unwrap_or_else(|e| panic!("{}: {e}", m.name));

        let wire = m
            .txs
            .split_whitespace()
            .map(|l| hex::decode(l).expect("fixture tx is hex"))
            .collect();

        Resolving {
            raw,
            body_cbor,
            certified,
            body,
            wire,
        }
    }

    #[test]
    fn each_real_certifying_block_resolves_with_the_endorser_block_its_parent_announced() {
        for (index, m) in MATCHED.iter().enumerate() {
            let r = matched(index);
            let name = m.name;
            let before = MultiEraBlock::decode(&r.raw).unwrap();
            assert_eq!(before.tx_count(), 0, "{name} precondition");

            let (_, list_end) = transaction_list_span(&r.raw);
            assert_eq!(
                r.raw.len() - list_end,
                64,
                "{name} Leios certificate and nil Peras slot"
            );

            let resolved = resolve_certified_block(&r.raw, &r.certified, &r.body, &r.wire)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let after = MultiEraBlock::decode(&resolved).expect("resolved block must decode");

            assert_eq!(after.tx_count(), m.count, "{name}");
            assert_eq!(after.era(), Era::Dijkstra);
            assert_eq!(after.header().cbor(), before.header().cbor());
            assert_eq!(after.hash(), before.hash());

            let (_, resolved_end) = transaction_list_span(&resolved);
            assert_eq!(
                &resolved[resolved_end..],
                &r.raw[list_end..],
                "{name} certificate slots come through the splice byte for byte"
            );

            let after_txs = after.txs();
            let txs = r.body.transactions(&r.wire).unwrap();
            for (i, (a, b)) in after_txs.iter().zip(txs.iter()).enumerate() {
                assert_eq!(a.hash(), b.hash(), "{name} transaction {i}");
            }
            assert_eq!(after_txs[0].hash().to_string(), m.first_tx_id, "{name}");
            assert_eq!(
                after_txs[m.count - 1].hash().to_string(),
                m.last_tx_id,
                "{name}"
            );
        }
    }

    #[test]
    fn a_body_the_certified_announcement_does_not_name_is_refused() {
        let r = matched(0);
        assert_eq!(r.certified.size, 8786, "fixture precondition");

        let (other, other_wire) = fixture(0);
        let err = resolve_certified_block(&r.raw, &r.certified, &other, &other_wire)
            .expect_err("a body of another length must be refused");
        assert!(
            matches!(
                err,
                Error::BodySize {
                    announced: 8786,
                    found: 37
                }
            ),
            "{err}"
        );

        let mut same_length = r.body_cbor.clone();
        let last = same_length.len() - 1;
        same_length[last] ^= 0x01;
        let other = announced_as_itself(&same_length).expect("still a body");

        let err = resolve_certified_block(&r.raw, &r.certified, &other, &r.wire)
            .expect_err("a body of the announced length and another hash must be refused");
        match err {
            Error::BodyHash { announced, found } => {
                assert_eq!(announced, r.certified.hash);
                assert_eq!(found, Hasher::<256>::hash(&same_length));
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// Each announcement here names the body passed with it, and was made by a
    /// header that is not the block's parent.
    #[test]
    fn an_announcement_the_blocks_parent_did_not_make_is_refused() {
        let r = matched(0);

        let other_header = hex::decode(include_str!("../../test_data/dijkstra-eb1.header").trim())
            .expect("fixture header is hex");
        let other = announcement_of(&dijkstra_header(&other_header)).expect("eb1 is announced");
        let (body, wire) = fixture(0);

        let err = resolve_certified_block(&r.raw, &other, &body, &wire)
            .expect_err("an announcement from another chain must be refused");
        match err {
            Error::NotParent {
                slot,
                previous,
                parent,
            } => {
                assert_eq!(slot, 371723);
                assert_eq!(previous, Some(r.certified.header));
                assert_eq!(parent, Some(other.header));
            }
            other => panic!("wrong refusal: {other}"),
        }

        let sibling = matched(1);
        let err = resolve_certified_block(&r.raw, &sibling.certified, &sibling.body, &sibling.wire)
            .expect_err("another block's certified announcement must be refused");
        assert!(
            matches!(err, Error::NotParent { parent: Some(p), .. } if p == sibling.certified.header),
            "{err}"
        );
    }

    #[test]
    fn a_delivery_the_body_does_not_name_is_refused_before_the_splice() {
        let Resolving {
            raw,
            certified,
            body,
            wire,
            ..
        } = matched(0);
        assert_eq!(
            body.entries()[0].1,
            body.entries()[1].1,
            "fixture precondition"
        );

        let mut permuted = wire.clone();
        permuted.swap(0, 1);
        let err = resolve_certified_block(&raw, &certified, &body, &permuted)
            .expect_err("a permuted delivery must be refused");
        assert!(matches!(err, Error::TxHash { index: 0, .. }), "{err}");

        let short = &wire[..wire.len() - 1];
        let err = resolve_certified_block(&raw, &certified, &body, short)
            .expect_err("a short delivery must be refused");
        assert!(
            matches!(
                err,
                Error::TxCount {
                    named: 244,
                    delivered: 243
                }
            ),
            "{err}"
        );

        let (_, other_wire) = fixture(0);
        let err = resolve_certified_block(&raw, &certified, &body, &other_wire)
            .expect_err("another endorser block's delivery must be refused");
        assert!(
            matches!(
                err,
                Error::TxCount {
                    named: 244,
                    delivered: 1
                }
            ),
            "{err}"
        );
    }

    /// No parent of a certifying fixture block announces a one transaction
    /// body, so the announcement is built from the body and names the block's
    /// real parent.
    #[test]
    fn a_transaction_with_bytes_after_it_inside_its_envelope_is_refused_before_the_splice() {
        let r = matched(0);
        let clean = unwrap_tx(&r.wire[0])
            .expect("the fixture is enveloped")
            .to_vec();

        let resolve_one = |inner: &[u8]| {
            let body_cbor = encode_entries(&[(Hasher::<256>::hash(inner), inner.len() as u32)]);
            let body = announced_as_itself(&body_cbor).unwrap();
            let certified = AnnouncedEndorserBlock {
                hash: Hasher::<256>::hash(&body_cbor),
                size: body_cbor.len() as u32,
                ..r.certified.clone()
            };
            resolve_certified_block(&r.raw, &certified, &body, &[enveloped(inner)])
        };

        let resolved = resolve_one(&clean).expect("the transaction as delivered resolves");
        assert_eq!(MultiEraBlock::decode(&resolved).unwrap().tx_count(), 1);

        let mut trailing = clean.clone();
        trailing.push(0x00);
        let err = resolve_one(&trailing).expect_err("a transaction with bytes after it");
        assert!(matches!(err, Error::TxDecode { index: 0, .. }), "{err}");
    }

    /// The Peras slot of this block is nil, so a splice that wrote a fresh nil
    /// would pass, and the slot is given a value to catch that.
    #[test]
    fn the_splice_keeps_a_peras_certificate_byte_for_byte() {
        let Resolving {
            raw,
            certified,
            body,
            wire,
            ..
        } = matched(0);
        assert_eq!(raw[raw.len() - 1], 0xf6, "fixture precondition");

        let mut with_peras = raw[..raw.len() - 1].to_vec();
        with_peras.extend_from_slice(&[0x44, 0xde, 0xad, 0xbe, 0xef]);
        assert!(
            MultiEraBlock::decode(&with_peras).is_ok(),
            "a block with a Peras certificate must still decode"
        );

        let kept =
            resolve_certified_block(&with_peras, &certified, &body, &wire).expect("must resolve");
        MultiEraBlock::decode(&kept).expect("the spliced block must decode");

        assert_eq!(
            &kept[kept.len() - 5..],
            &[0x44, 0xde, 0xad, 0xbe, 0xef],
            "the Peras certificate comes through the splice byte for byte"
        );
    }

    #[test]
    fn the_spliced_transactions_carry_the_blocks_validity_flag() {
        let Resolving {
            raw,
            certified,
            body,
            wire,
            ..
        } = matched(1);

        let resolved = resolve_certified_block(&raw, &certified, &body, &wire).unwrap();
        let after = MultiEraBlock::decode(&resolved).unwrap();

        let txs = after.txs();
        assert_eq!(txs.len(), 688);
        assert!(
            txs.iter().all(|tx| tx.is_valid()),
            "the mempool form admits no verdict but valid"
        );

        let inner = unwrap_tx(&wire[0]).unwrap();
        assert_eq!(
            inner[0], 0x83,
            "the delivered transaction has three elements"
        );
        let (start, _) = transaction_list_span(&resolved);
        let mut d = Decoder::new(&resolved[start..]);
        d.array().expect("the transaction list");
        assert_eq!(
            resolved[start + d.position()],
            0x84,
            "the block has four, the array header immediately after the list header"
        );
    }

    #[test]
    fn a_block_that_certifies_nothing_is_refused() {
        let raw = hex::decode(include_str!("../../test_data/dijkstra6.block").trim()).unwrap();
        let block = MultiEraBlock::decode(&raw).unwrap();
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(false));

        let Resolving {
            certified,
            body,
            wire,
            ..
        } = matched(0);

        let err = resolve_certified_block(&raw, &certified, &body, &wire)
            .expect_err("a non-certifying block must be refused");

        assert!(
            matches!(err, Error::NotCertifying { slot: 291625 }),
            "{err}"
        );
    }

    /// No fixture block certifies and has transactions of its own, so block
    /// 14278 and its four transactions are rewritten to certify.
    #[test]
    fn a_certifying_block_with_its_own_transactions_is_refused_on_a_rewritten_header() {
        let raw =
            with_leios_header_fields(include_str!("../../test_data/dijkstra6.block"), true, None);
        let header = header_of(&raw);
        assert_eq!(
            dijkstra_header(&header).block_body_contains_leios_cert(),
            Some(true),
            "fixture precondition"
        );

        let Resolving {
            certified,
            body,
            wire,
            ..
        } = matched(0);

        let err = resolve_certified_block(&raw, &certified, &body, &wire)
            .expect_err("certifying and having transactions must be refused");

        match err {
            Error::CertifiesAndCarries { slot, count } => {
                assert_eq!(slot, 291625);
                assert_eq!(count, 4);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    #[test]
    fn a_pre_leios_block_is_refused() {
        let raw = hex::decode(include_str!("../../test_data/conway1.block").trim()).unwrap();
        let Resolving {
            certified,
            body,
            wire,
            ..
        } = matched(0);

        let err = resolve_certified_block(&raw, &certified, &body, &wire)
            .expect_err("a Conway block must not be resolved");

        assert!(matches!(err, Error::NotLeiosEra { .. }), "{err}");
    }

    /// No fixture endorser block is empty, so the announcement is built from
    /// the empty body and names the block's real parent.
    #[test]
    fn an_empty_body_resolves_to_an_empty_block() {
        let r = matched(2);
        let empty = encode_entries(&[]);
        let body = announced_as_itself(&empty).unwrap();
        let certified = AnnouncedEndorserBlock {
            hash: Hasher::<256>::hash(&empty),
            size: empty.len() as u32,
            ..r.certified
        };

        let resolved = resolve_certified_block(&r.raw, &certified, &body, &[]).unwrap();
        let after = MultiEraBlock::decode(&resolved).unwrap();

        assert_eq!(after.tx_count(), 0);
    }
}
