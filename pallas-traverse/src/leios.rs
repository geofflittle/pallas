//! Leios endorser blocks, read from a follower's side of the wire.
//!
//! A ranking block that certifies an endorser block carries none of its
//! transactions, so a follower fetches the endorser block over leios-fetch and
//! splices its transactions into the certifying block.

use pallas_codec::minicbor::Decoder;
use pallas_codec::utils::MaybeIndefArray;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_primitives::dijkstra;

use crate::{Era, MultiEraHeader, MultiEraTx};

/// Why an endorser block body, one of its transactions, or a certifying block
/// was refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("endorser block body is not a cbor map of hash to size: {0}")]
    InvalidBody(String),

    #[error(
        "endorser block body is {found} bytes and the announcement committed to {announced}, \
         so it is not the announced endorser block"
    )]
    BodySize { announced: u32, found: usize },

    #[error("endorser block body hashes to {found} and the announcement named {announced}")]
    BodyHash {
        announced: Hash<32>,
        found: Hash<32>,
    },

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
        "the header at slot {slot} certifies an endorser block and the walk resumed without \
         knowing whether an announcement was pending"
    )]
    CertifiesUnknown { slot: u64 },

    #[error(
        "the block at slot {slot} certifies an endorser block and carries {count} transactions \
         of its own"
    )]
    CertifiesAndCarries { slot: u64, count: usize },

    #[error("the block at slot {slot} certifies no endorser block")]
    NotCertifying { slot: u64 },

    #[error("a {era} block cannot certify an endorser block")]
    NotLeiosEra { era: Era },

    #[error("the block cbor is not a ranking block: {0}")]
    InvalidBlock(String),
}

/// An endorser block body, with the checks a follower applies to what
/// leios-fetch delivers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlockBody {
    block: dijkstra::EndorserBlock,
}

impl EndorserBlockBody {
    /// Decodes a body with no announcement to check it against, so the empty
    /// body a peer sends for a block it lacks reads as a block naming nothing.
    pub fn decode(cbor: &[u8]) -> Result<Self, Error> {
        let mut d = Decoder::new(cbor);

        let block = d
            .decode()
            .map_err(|e| Error::InvalidBody(e.to_string()))?;

        if d.position() != cbor.len() {
            return Err(Error::InvalidBody(format!(
                "{} trailing bytes after the map",
                cbor.len() - d.position()
            )));
        }

        Ok(Self { block })
    }

    /// Decodes a body, refusing one whose length or blake2b-256 differs from
    /// what the announcement commits to.
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

        Self::decode(cbor)
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

    /// Checks each delivered transaction, still in its leios-fetch envelope,
    /// against the entry at its position, and decodes it.
    pub fn transactions<'b>(&self, wire: &'b [Vec<u8>]) -> Result<Vec<MultiEraTx<'b>>, Error> {
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

fn decode_closure_transaction(index: usize, inner: &[u8]) -> Result<MultiEraTx<'_>, Error> {
    let mempool: dijkstra::MempoolTransaction =
        pallas_codec::minicbor::decode(inner).map_err(|e| Error::TxDecode {
            index,
            reason: e.to_string(),
        })?;

    Ok(MultiEraTx::Dijkstra(Box::new(std::borrow::Cow::Owned(
        mempool.to_block_transaction(),
    ))))
}

/// An announced endorser block, with the point a leios-fetch request needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncedEndorserBlock {
    /// Slot of the announcing header, which is the slot a leios-fetch point
    /// carries.
    pub slot: u64,
    /// The body hash the announcement commits to.
    pub hash: Hash<32>,
    /// The body length the announcement commits to.
    pub size: u32,
}

/// What one ranking block header certifies and announces, which can be both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderOutcome {
    /// The pending announcement this header certifies.
    pub certified: Option<AnnouncedEndorserBlock>,
    /// The announcement this header makes.
    pub announced: Option<AnnouncedEndorserBlock>,
}

/// What a certification walk knows about an announcement awaiting its
/// certificate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PendingAnnouncement {
    /// No announcement is pending.
    #[default]
    Nothing,

    /// This announcement is pending.
    Waiting(AnnouncedEndorserBlock),

    /// The walk resumed from a stored position and has read no announcement
    /// since, so it cannot say what a certificate names.
    Unknown,
}

impl PendingAnnouncement {
    /// The pending announcement, when the walk knows of one.
    pub fn waiting(&self) -> Option<&AnnouncedEndorserBlock> {
        match self {
            Self::Waiting(eb) => Some(eb),
            _ => None,
        }
    }
}

/// A walk over ranking chain headers that decides which endorser block each
/// certificate names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CertificationTracker {
    pending: PendingAnnouncement,
}

impl CertificationTracker {
    /// Starts a walk from a stored state rather than from origin.
    pub fn resume_from(pending: PendingAnnouncement) -> Self {
        Self { pending }
    }

    /// What the walk knows about a pending announcement.
    pub fn pending(&self) -> &PendingAnnouncement {
        &self.pending
    }

    /// Reads the next header, settling its certificate before recording its
    /// own announcement, since a certificate names an earlier announcement.
    pub fn observe(&mut self, header: &MultiEraHeader) -> Result<HeaderOutcome, Error> {
        let mut outcome = HeaderOutcome::default();

        if header.block_body_contains_leios_cert() == Some(true) {
            // A refusal leaves the walk unchanged.
            match &self.pending {
                PendingAnnouncement::Nothing => {
                    return Err(Error::CertifiesNothing {
                        slot: header.slot(),
                    });
                }
                PendingAnnouncement::Unknown => {
                    return Err(Error::CertifiesUnknown {
                        slot: header.slot(),
                    });
                }
                PendingAnnouncement::Waiting(_) => {
                    let taken = std::mem::take(&mut self.pending);

                    let PendingAnnouncement::Waiting(pending) = taken else {
                        unreachable!("just matched as waiting")
                    };

                    outcome.certified = Some(pending);
                }
            }
        }

        if let Some(announcement) = header.eb_announcement() {
            let announced = AnnouncedEndorserBlock {
                slot: header.slot(),
                hash: announcement.eb_hash,
                size: announcement.eb_size,
            };

            self.pending = PendingAnnouncement::Waiting(announced.clone());
            outcome.announced = Some(announced);
        }

        Ok(outcome)
    }
}

/// Rewrites a certifying ranking block so its transactions are those of the
/// endorser block it certifies, given unwrapped and in body order.
pub fn resolve_certified_block(block_cbor: &[u8], txs: &[&[u8]]) -> Result<Vec<u8>, Error> {
    let block =
        crate::MultiEraBlock::decode(block_cbor).map_err(|e| Error::InvalidBlock(e.to_string()))?;

    if block.era() != Era::Dijkstra {
        return Err(Error::NotLeiosEra { era: block.era() });
    }

    if block.header().block_body_contains_leios_cert() != Some(true) {
        return Err(Error::NotCertifying { slot: block.slot() });
    }

    refuse_certifying_block_with_own_txs(block.slot(), true, block.tx_count())?;

    replace_transaction_list(block_cbor, txs)
}

/// Replaces a Dijkstra block's transactions with the given mempool
/// transactions and keeps the header bytes, so the header no longer commits to
/// the body.
pub fn replace_transaction_list(block_cbor: &[u8], txs: &[&[u8]]) -> Result<Vec<u8>, Error> {
    let (era, mut block): (u16, dijkstra::Block) = pallas_codec::minicbor::decode(block_cbor)
        .map_err(|e| Error::InvalidBlock(e.to_string()))?;

    let mut replacement = Vec::with_capacity(txs.len());
    for (index, tx) in txs.iter().enumerate() {
        let mempool: dijkstra::MempoolTransaction =
            pallas_codec::minicbor::decode(tx).map_err(|e| Error::TxDecode {
                index,
                reason: e.to_string(),
            })?;

        replacement.push(mempool.to_block_transaction());
    }

    block.block_body.transactions = MaybeIndefArray::Def(replacement);

    Ok(pallas_codec::minicbor::to_vec((era, block)).expect("write to a vec"))
}

/// Refuses a block that both certifies an endorser block and carries its own
/// transactions.
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

    fn encode_entries(entries: &[(Hash<32>, u32)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = Encoder::new(&mut out);
        e.map(entries.len() as u64).expect("write to a vec");
        for (hash, size) in entries {
            e.bytes(hash.as_ref()).expect("write to a vec");
            e.u32(*size).expect("write to a vec");
        }
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

        let err = EndorserBlockBody::decode(&raw).expect_err("trailing bytes must be refused");

        assert!(matches!(err, Error::InvalidBody(_)), "wrong refusal: {err}");
    }

    /// leios-fetch answers a request for a block the peer lacks with an empty
    /// body, and only the announced length tells the two apart.
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
            eb_hash: Hash::from(
                <[u8; 32]>::try_from(
                    hex::decode("2f5cd8a253b845763f1d3bd7d75dbb43f20507eb4a69af58ef42ce0202082bcc")
                        .expect("hex")
                        .as_slice(),
                )
                .expect("32 bytes"),
            ),
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
            "the wire already carried this body in key sorted order"
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
            pallas_codec::minicbor::decode(&cbor[start..end]).expect("fixture header decodes");

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

        let rebuilt = pallas_codec::minicbor::to_vec(&header).expect("write to a vec");

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

    pub(super) fn announcement_of(hash: [u8; 32], size: u32) -> dijkstra::EbAnnouncement {
        dijkstra::EbAnnouncement {
            eb_hash: Hash::new(hash),
            eb_size: size,
        }
    }

    /// Block 14935 at slot 311025, which carries no transactions, made to
    /// certify.
    pub(super) fn certifying_block() -> Vec<u8> {
        with_leios_header_fields(include_str!("../../test_data/dijkstra16.block"), true, None)
    }

    fn header_of(block_cbor: &[u8]) -> Vec<u8> {
        let block = MultiEraBlock::decode(block_cbor).unwrap();
        block.header().cbor().to_vec()
    }

    fn dijkstra_header(raw: &[u8]) -> MultiEraHeader<'_> {
        MultiEraHeader::decode(7, None, raw).unwrap()
    }

    #[test]
    fn the_synthetic_header_changes_two_fields_and_nothing_else() {
        let plain = hex::decode(include_str!("../../test_data/dijkstra16.block").trim()).unwrap();
        let built = with_leios_header_fields(
            include_str!("../../test_data/dijkstra16.block"),
            true,
            Some(announcement_of([7; 32], 4096)),
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

    #[test]
    fn a_header_that_certifies_nothing_pending_is_refused() {
        let block = certifying_block();
        let raw = header_of(&block);
        let header = dijkstra_header(&raw);
        assert_eq!(
            header.block_body_contains_leios_cert(),
            Some(true),
            "fixture precondition"
        );

        let mut tracker = CertificationTracker::default();
        let err = tracker
            .observe(&header)
            .expect_err("certifying with nothing pending must be refused");

        match err {
            Error::CertifiesNothing { slot } => assert_eq!(slot, 311025),
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// Four real blocks in chain order, with the Leios fields set on three of
    /// them, since no block on the chain they come from sets either field.
    #[test]
    fn certification_walks_the_headers_and_abandons_a_superseded_announcement() {
        let earlier = AnnouncedEndorserBlock {
            slot: 86000,
            hash: Hash::new([9; 32]),
            size: 1234,
        };

        let mut tracker =
            CertificationTracker::resume_from(PendingAnnouncement::Waiting(earlier.clone()));

        // Block 4255 certifies the carried announcement and makes its own.
        let block2 = with_leios_header_fields(
            include_str!("../../test_data/dijkstra1.block"),
            true,
            Some(announcement_of([0x2b; 32], 28519)),
        );
        let raw2 = header_of(&block2);
        let out = tracker.observe(&dijkstra_header(&raw2)).unwrap();
        assert_eq!(out.certified.as_ref(), Some(&earlier));
        let announced2 = out.announced.expect("4255 announces");
        assert_eq!(announced2.slot, 86463);
        assert_eq!(announced2.hash, Hash::new([0x2b; 32]));
        assert_eq!(announced2.size, 28519);
        assert_eq!(tracker.pending().waiting(), Some(&announced2));

        // Block 14212, unmodified, neither certifies nor announces.
        let raw5 = header_of(
            &hex::decode(include_str!("../../test_data/dijkstra4.block").trim()).unwrap(),
        );
        let out = tracker.observe(&dijkstra_header(&raw5)).unwrap();
        assert_eq!(out, HeaderOutcome::default());
        assert_eq!(tracker.pending().waiting(), Some(&announced2));

        // Block 14278 announces without certifying, abandoning 4255's.
        let block7 = with_leios_header_fields(
            include_str!("../../test_data/dijkstra6.block"),
            false,
            Some(announcement_of([0x70; 32], 512)),
        );
        let raw7 = header_of(&block7);
        let out = tracker.observe(&dijkstra_header(&raw7)).unwrap();
        assert_eq!(out.certified, None, "14278 certifies nothing");
        let announced7 = out.announced.expect("14278 announces");
        assert_eq!(announced7.hash, Hash::new([0x70; 32]));
        assert_eq!(tracker.pending().waiting(), Some(&announced7));

        // Block 14936 announces again, abandoning 14278's.
        let block9 = with_leios_header_fields(
            include_str!("../../test_data/dijkstra7.block"),
            false,
            Some(announcement_of([0x17; 32], 74668)),
        );
        let raw9 = header_of(&block9);
        let out = tracker.observe(&dijkstra_header(&raw9)).unwrap();
        assert_eq!(out.certified, None);
        let announced9 = out.announced.expect("14936 announces");
        assert_eq!(announced9.slot, 311104);
        assert_eq!(announced9.size, 74668);
        assert_eq!(tracker.pending().waiting(), Some(&announced9));
    }

    #[test]
    fn a_pre_leios_header_certifies_nothing() {
        let cbor = hex::decode(include_str!("../../test_data/conway1.block").trim()).unwrap();
        let block = MultiEraBlock::decode(&cbor).unwrap();
        let raw = block.header().cbor().to_vec();
        let header = MultiEraHeader::decode(6, None, &raw).unwrap();
        assert_eq!(
            header.block_body_contains_leios_cert(),
            None,
            "fixture precondition"
        );

        let mut tracker = CertificationTracker::resume_from(PendingAnnouncement::Waiting(
            AnnouncedEndorserBlock {
                slot: 1,
                hash: Hash::new([1; 32]),
                size: 5,
            },
        ));

        let out = tracker.observe(&header).unwrap();
        assert_eq!(out, HeaderOutcome::default());
        assert!(
            tracker.pending().waiting().is_some(),
            "pending is left alone"
        );
    }

    #[test]
    fn a_walk_that_cannot_tell_refuses_a_certificate_as_its_own_ignorance() {
        let block = certifying_block();
        let raw = header_of(&block);
        let header = dijkstra_header(&raw);
        assert_eq!(
            header.block_body_contains_leios_cert(),
            Some(true),
            "fixture precondition"
        );

        let mut unknown = CertificationTracker::resume_from(PendingAnnouncement::Unknown);
        let err = unknown
            .observe(&header)
            .expect_err("a walk that cannot tell must refuse");

        match err {
            Error::CertifiesUnknown { slot } => assert_eq!(slot, 311025),
            other => panic!("wrong refusal: {other}"),
        }

        let mut nothing = CertificationTracker::resume_from(PendingAnnouncement::Nothing);
        let err = nothing
            .observe(&header)
            .expect_err("a walk that knows nothing is waiting must refuse");

        assert!(
            matches!(err, Error::CertifiesNothing { .. }),
            "knowing nothing is waiting is a different refusal: {err}"
        );
    }

    #[test]
    fn an_announcement_settles_a_walk_that_could_not_tell() {
        let mut tracker = CertificationTracker::resume_from(PendingAnnouncement::Unknown);

        let block7 = with_leios_header_fields(
            include_str!("../../test_data/dijkstra6.block"),
            false,
            Some(announcement_of([0x70; 32], 512)),
        );
        let raw7 = header_of(&block7);
        let out = tracker.observe(&dijkstra_header(&raw7)).unwrap();
        let announced = out.announced.expect("14278 announces");
        assert_eq!(
            tracker.pending(),
            &PendingAnnouncement::Waiting(announced.clone()),
            "the announcement replaces not knowing"
        );

        let block8 = certifying_block();
        let raw8 = header_of(&block8);
        let out = tracker.observe(&dijkstra_header(&raw8)).unwrap();
        assert_eq!(out.certified, Some(announced));
    }

    #[test]
    fn a_refused_certificate_does_not_change_the_walk() {
        let block = certifying_block();
        let raw = header_of(&block);
        let header = dijkstra_header(&raw);

        let mut tracker = CertificationTracker::resume_from(PendingAnnouncement::Unknown);

        assert!(tracker.observe(&header).is_err());
        assert_eq!(tracker.pending(), &PendingAnnouncement::Unknown);

        let err = tracker
            .observe(&header)
            .expect_err("the second look must refuse the same way");

        assert!(matches!(err, Error::CertifiesUnknown { .. }), "{err}");
    }

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

#[cfg(test)]
mod resolve_tests {
    use super::tests::{certifying_block, fixture, transaction_list_span};
    use super::*;
    use crate::MultiEraBlock;

    #[test]
    fn a_certifying_block_resolves_to_the_endorser_blocks_transactions() {
        let raw = certifying_block();
        let before = MultiEraBlock::decode(&raw).unwrap();
        assert_eq!(before.header().block_body_contains_leios_cert(), Some(true));
        assert_eq!(before.tx_count(), 0, "fixture precondition");

        let (body, wire) = fixture(2);
        let txs = body.transactions(&wire).unwrap();
        let inner: Vec<&[u8]> = wire.iter().map(|w| unwrap_tx(w).unwrap()).collect();

        let resolved_cbor = resolve_certified_block(&raw, &inner).expect("must resolve");
        let after = MultiEraBlock::decode(&resolved_cbor).expect("resolved block must decode");

        assert_eq!(after.tx_count(), 425);
        assert_eq!(after.era(), Era::Dijkstra);

        assert_eq!(after.header().cbor(), before.header().cbor());
        assert_eq!(after.hash(), before.hash());
        assert_eq!(after.slot(), before.slot());

        let after_txs = after.txs();
        assert_eq!(after_txs.len(), txs.len());
        for (i, (a, b)) in after_txs.iter().zip(txs.iter()).enumerate() {
            assert_eq!(a.hash(), b.hash(), "transaction {i}");
        }
        assert_eq!(
            after_txs[391].hash().to_string(),
            "fffa4361c5251f57f4840c94dcbd05164cdce9b2bcf9bbf75e2fa4baaf30cf87"
        );
    }

    #[test]
    fn a_single_transaction_endorser_block_resolves() {
        let raw = certifying_block();
        let (_, wire) = fixture(0);
        let inner: Vec<&[u8]> = wire.iter().map(|w| unwrap_tx(w).unwrap()).collect();

        let resolved_cbor = resolve_certified_block(&raw, &inner).unwrap();
        let after = MultiEraBlock::decode(&resolved_cbor).unwrap();

        assert_eq!(after.tx_count(), 1);
        let txs = after.txs();
        assert_eq!(
            txs[0].hash().to_string(),
            "a2a3715bac697e28991d003c62a0c280fb91af40944f7bbcee27d972ad0f0a08"
        );
        let MultiEraTx::Dijkstra(tx) = &txs[0] else {
            panic!("a resolved Dijkstra block carries Dijkstra transactions");
        };
        assert_eq!(tx.transaction_body.inputs.len(), 1);
    }

    #[test]
    fn a_block_that_certifies_nothing_is_refused() {
        let raw = hex::decode(include_str!("../../test_data/dijkstra6.block").trim()).unwrap();
        let block = MultiEraBlock::decode(&raw).unwrap();
        assert_eq!(block.header().block_body_contains_leios_cert(), Some(false));
        assert_eq!(block.tx_count(), 4, "fixture precondition");

        let (_, wire) = fixture(0);
        let inner: Vec<&[u8]> = wire.iter().map(|w| unwrap_tx(w).unwrap()).collect();

        let err = resolve_certified_block(&raw, &inner)
            .expect_err("a non-certifying block must be refused");

        assert!(matches!(err, Error::NotCertifying { .. }), "{err}");
    }

    /// Every block on the chain has nil in both certificate slots, so a splice
    /// that wrote fresh nils would pass the first check, and the Peras slot is
    /// given a value to catch that.
    #[test]
    fn the_splice_replaces_the_transaction_list_and_not_a_certificate_slot() {
        let raw = hex::decode(include_str!("../../test_data/dijkstra6.block").trim()).unwrap();
        let before = MultiEraBlock::decode(&raw).unwrap();
        assert_eq!(before.tx_count(), 4, "fixture precondition");
        assert_eq!(&raw[raw.len() - 2..], &[0xf6, 0xf6], "fixture precondition");

        let spliced = replace_transaction_list(&raw, &[]).expect("the splice must succeed");
        let after = MultiEraBlock::decode(&spliced).expect("the spliced block must decode");

        assert_eq!(after.tx_count(), 0);
        assert_eq!(after.era(), Era::Dijkstra);
        assert_eq!(
            after.header().cbor(),
            before.header().cbor(),
            "the header is untouched"
        );
        assert_eq!(
            &spliced[spliced.len() - 2..],
            &[0xf6, 0xf6],
            "both certificate slots survive the splice"
        );

        let mut with_peras = raw[..raw.len() - 1].to_vec();
        with_peras.extend_from_slice(&[0x44, 0xde, 0xad, 0xbe, 0xef]);
        assert!(
            MultiEraBlock::decode(&with_peras).is_ok(),
            "a block carrying a Peras certificate must still decode"
        );

        let kept = replace_transaction_list(&with_peras, &[]).expect("the splice must succeed");
        MultiEraBlock::decode(&kept).expect("the spliced block must decode");
        assert_eq!(
            &kept[kept.len() - 5..],
            &[0x44, 0xde, 0xad, 0xbe, 0xef],
            "the Peras certificate comes through the splice byte for byte"
        );

        let (start, _) = transaction_list_span(&raw);
        assert_eq!(&spliced[..start], &raw[..start]);
        assert_eq!(
            spliced.len(),
            start + 3,
            "an empty list plus the two nil certificate slots is three bytes"
        );
    }

    #[test]
    fn the_spliced_transactions_carry_the_blocks_validity_flag() {
        let raw = certifying_block();
        let (body, wire) = fixture(0);
        let inner: Vec<&[u8]> = wire.iter().map(|w| unwrap_tx(w).unwrap()).collect();

        let resolved = resolve_certified_block(&raw, &inner).unwrap();
        let after = MultiEraBlock::decode(&resolved).unwrap();

        let txs = after.txs();
        assert_eq!(txs.len(), 1);
        assert!(txs[0].is_valid(), "the mempool form admits no verdict but valid");
        assert_eq!(txs[0].hash(), body.transactions(&wire).unwrap()[0].hash());

        assert_eq!(inner[0][0], 0x83, "the closure carries three elements");
        let (start, _) = transaction_list_span(&resolved);
        assert_eq!(
            resolved[start + 1],
            0x84,
            "the block carries four, the array header immediately after the list header"
        );
    }

    #[test]
    fn a_pre_leios_block_is_refused() {
        let raw = hex::decode(include_str!("../../test_data/conway1.block").trim()).unwrap();

        let err =
            resolve_certified_block(&raw, &[]).expect_err("a Conway block must not be resolved");

        assert!(matches!(err, Error::NotLeiosEra { .. }), "{err}");
    }

    /// An endorser block may name no transactions, and a missing one is refused
    /// earlier, on its announced length.
    #[test]
    fn resolving_with_no_transactions_gives_an_empty_block() {
        let raw = certifying_block();
        let resolved_cbor = resolve_certified_block(&raw, &[]).unwrap();
        let after = MultiEraBlock::decode(&resolved_cbor).unwrap();
        assert_eq!(after.tx_count(), 0);
    }
}
