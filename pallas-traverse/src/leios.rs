//! Leios endorser blocks as a follower receives them over leios-fetch.
//!
//! Over node to node block-fetch, a ranking block that certifies an endorser
//! block contains none of its transactions, and over node to client ChainSync
//! it already contains them. When a certifying block is validated, its endorser
//! block's transactions apply to its parent's ledger state, before the tick to
//! the certifying block's slot.

use std::collections::HashMap;

use pallas_codec::minicbor::{self, bytes::ByteSlice, Decoder};
use pallas_codec::utils::AnyCbor;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_primitives::dijkstra;

use crate::{Era, MultiEraHeader, MultiEraTx};

/// Why an endorser block, its transactions, or a certificate was refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The body is not a definite length CBOR map of hash to size.
    #[error("invalid endorser block body: {0}")]
    InvalidBody(String),

    /// The body hash differs from the announced hash.
    #[error("endorser block hash {found}, announced {announced}")]
    BodyHash {
        announced: Hash<32>,
        found: Hash<32>,
    },

    /// The body names one transaction hash at two entries.
    #[error("endorser block names {hash} at entries {first} and {second}")]
    DuplicateTx {
        hash: Hash<32>,
        first: usize,
        second: usize,
    },

    /// The number of delivered transactions differs from the number the body names.
    #[error("{delivered} transactions delivered, endorser block names {named}")]
    TxCount { named: usize, delivered: usize },

    /// A delivered transaction's hash differs from the hash its entry names.
    #[error("transaction {index} hash {found}, endorser block names {named}")]
    TxHash {
        index: usize,
        named: Hash<32>,
        found: Hash<32>,
    },

    /// A delivered transaction is not a CBOR byte string holding a Dijkstra
    /// transaction.
    #[error("transaction {index} is not a Dijkstra transaction: {reason}")]
    TxDecode { index: usize, reason: String },

    /// The header given as the parent is not the one the header names.
    #[error("header at slot {slot} has parent {previous:?}, given {parent:?}")]
    NotParent {
        slot: u64,
        previous: Option<Hash<32>>,
        parent: Option<Hash<32>>,
    },

    /// A header certifies an endorser block and its parent announced none.
    #[error("header at slot {slot} certifies, its parent announced none")]
    CertifiesNothing { slot: u64 },
}

/// An endorser block body as leios-fetch delivers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlockBody {
    block: dijkstra::EndorserBlock,
}

impl EndorserBlockBody {
    /// Decodes a body, refusing one whose minimal re-encoding does not hash to
    /// the announced blake2b-256, or that names one transaction twice.
    pub fn decode_announced(
        cbor: &[u8],
        announcement: &dijkstra::EbAnnouncement,
    ) -> Result<Self, Error> {
        let mut d = Decoder::new(cbor);

        let block: dijkstra::EndorserBlock =
            d.decode().map_err(|e| Error::InvalidBody(e.to_string()))?;

        if d.position() != cbor.len() {
            return Err(Error::InvalidBody(format!(
                "{} trailing bytes after the map",
                cbor.len() - d.position()
            )));
        }

        let found = Hasher::<256>::hash_cbor(&block);
        if found != announcement.eb_hash {
            return Err(Error::BodyHash {
                announced: announcement.eb_hash,
                found,
            });
        }

        let mut seen = HashMap::with_capacity(block.len());
        for (second, (hash, _)) in block.iter().enumerate() {
            if let Some(first) = seen.insert(*hash, second) {
                return Err(Error::DuplicateTx {
                    hash: *hash,
                    first,
                    second,
                });
            }
        }

        Ok(Self { block })
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

    /// Checks every transaction the body names, in body order, against its
    /// entry's hash and decodes it. `wire` holds the byte strings leios-fetch
    /// delivers, one per entry. The `encode()` of a returned transaction is the
    /// four element block form, not the delivered bytes, and its `hash()` is
    /// the blake2b-256 of its body bytes as delivered.
    pub fn transactions<'b>(&self, wire: &'b [AnyCbor]) -> Result<Vec<MultiEraTx<'b>>, Error> {
        if wire.len() != self.block.len() {
            return Err(Error::TxCount {
                named: self.block.len(),
                delivered: wire.len(),
            });
        }

        let mut out = Vec::with_capacity(wire.len());

        for (index, ((named, _), delivered)) in self.block.iter().zip(wire).enumerate() {
            let tx_decode = |e: minicbor::decode::Error| Error::TxDecode {
                index,
                reason: e.to_string(),
            };

            let inner: &ByteSlice = minicbor::decode(delivered.raw_bytes()).map_err(tx_decode)?;

            let found = Hasher::<256>::hash(inner);
            if found != *named {
                return Err(Error::TxHash {
                    index,
                    named: *named,
                    found,
                });
            }

            out.push(MultiEraTx::decode_for_era(Era::Dijkstra, inner).map_err(tx_decode)?);
        }

        Ok(out)
    }
}

/// The announcement a ranking header certifies, which is its parent's and
/// never its own. `parent` is `None` for a header that follows genesis.
pub fn certification<'a>(
    parent: Option<&'a MultiEraHeader>,
    header: &MultiEraHeader,
) -> Result<Option<&'a dijkstra::EbAnnouncement>, Error> {
    let parent_hash = parent.map(|p| p.hash());

    if header.previous_hash() != parent_hash {
        return Err(Error::NotParent {
            slot: header.slot(),
            previous: header.previous_hash(),
            parent: parent_hash,
        });
    }

    if header.block_body_contains_leios_cert() != Some(true) {
        return Ok(None);
    }

    parent
        .and_then(|p| p.eb_announcement())
        .map(Some)
        .ok_or(Error::CertifiesNothing {
            slot: header.slot(),
        })
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
            name: "dijkstra-17402",
            body: include_str!("../../test_data/dijkstra-17402.ebbody"),
            txs: include_str!("../../test_data/dijkstra-17402.ebtxs"),
            count: 244,
            first_tx_id: "82bfbdf62f180269e15a72672b51610bc17a043c3d6f5c5a2434403b00a49d98",
            header: include_str!("../../test_data/dijkstra-17402.header"),
            slot: 371680,
            header_hash: "b0e696b02f5c527b43eabf2149b71c7f9579b4e2db1dbe0c04fe67881cdb8aa9",
        },
    ];

    impl Fixture {
        fn body_bytes(&self) -> Vec<u8> {
            hex::decode(self.body.trim()).expect("fixture body is hex")
        }

        fn wire_txs(&self) -> Vec<AnyCbor> {
            wire_txs(self.txs)
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

    fn wire_txs(hex_txs: &str) -> Vec<AnyCbor> {
        hex_txs
            .split_whitespace()
            .map(|l| AnyCbor::from(hex::decode(l).expect("fixture tx is hex")))
            .collect()
    }

    fn fixture(index: usize) -> (EndorserBlockBody, Vec<AnyCbor>) {
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

    fn hash_of(hex_hash: &str) -> Hash<32> {
        hex_hash.parse().expect("a 32 byte hex hash")
    }

    /// Decodes a body that no fixture header announces, against an
    /// announcement of its own length and hash.
    fn announced_as_itself(cbor: &[u8]) -> Result<EndorserBlockBody, Error> {
        EndorserBlockBody::decode_announced(
            cbor,
            &dijkstra::EbAnnouncement {
                eb_hash: Hasher::<256>::hash(cbor),
                eb_size: cbor.len() as u32,
            },
        )
    }

    fn enveloped(inner: &[u8]) -> AnyCbor {
        let mut out = Vec::new();
        Encoder::new(&mut out).bytes(inner).expect("write to a vec");
        AnyCbor::from(out)
    }

    fn unwrapped(tx: &AnyCbor) -> &[u8] {
        minicbor::decode::<&ByteSlice>(tx.raw_bytes()).expect("the fixture is enveloped")
    }

    /// A body naming each transaction by its own hash and length, announced as
    /// itself.
    fn body_naming(txs: &[&[u8]]) -> EndorserBlockBody {
        let entries: Vec<_> = txs
            .iter()
            .map(|tx| (Hasher::<256>::hash(tx), tx.len() as u32))
            .collect();

        announced_as_itself(&encode_entries(&entries)).expect("a body of distinct entries")
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
            Error::BodyHash { announced, found } => {
                assert_eq!(announced, announcement.eb_hash);
                assert_eq!(found, Hasher::<256>::hash(&[0xa0]));
            }
            other => panic!("wrong refusal: {other}"),
        }

        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &announcement)
            .expect("the announced body must be accepted");
        assert_eq!(body.len(), 244);
    }

    /// The node does not compare a body with the announced size.
    #[test]
    fn a_body_is_accepted_whatever_size_the_announcement_names() {
        let f = &FIXTURES[2];
        let announced = f.announcement();
        assert_eq!(announced.eb_size, 8786, "fixture precondition");

        for eb_size in [0, 8785, 8787, u32::MAX] {
            let announcement = dijkstra::EbAnnouncement {
                eb_size,
                ..announced.clone()
            };

            let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &announcement)
                .unwrap_or_else(|e| panic!("announced size {eb_size}: {e}"));
            assert_eq!(body.len(), 244);
        }
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
    /// entry, and the node hashes that entry's minimal encoding.
    #[test]
    fn the_announced_hash_is_over_the_minimal_re_encoding() {
        let f = &FIXTURES[0];
        let minimal = f.body_bytes();
        assert_eq!(&minimal[35..], &[0x18, 0xe5], "fixture precondition");

        let mut wide = minimal[..35].to_vec();
        wide.extend_from_slice(&[0x19, 0x00, 0xe5]);

        let of_minimal = dijkstra::EbAnnouncement {
            eb_hash: f.announcement().eb_hash,
            eb_size: wide.len() as u32,
        };
        assert_eq!(of_minimal.eb_hash, Hasher::<256>::hash(&minimal));

        let body = EndorserBlockBody::decode_announced(&wide, &of_minimal)
            .expect("the wide body re-encodes to the announced one");

        let (minimal_body, wire) = fixture(0);
        assert_eq!(body.entries(), minimal_body.entries());
        assert_eq!(body.transactions(&wire).unwrap().len(), 1);

        let of_wide = dijkstra::EbAnnouncement {
            eb_hash: Hasher::<256>::hash(&wide),
            eb_size: wide.len() as u32,
        };

        let err = EndorserBlockBody::decode_announced(&wide, &of_wide)
            .expect_err("a hash over the wide bytes names no body the node accepts");

        match err {
            Error::BodyHash { announced, found } => {
                assert_eq!(announced, of_wide.eb_hash);
                assert_eq!(found, of_minimal.eb_hash);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// The body keys a transaction by the blake2b-256 of the whole
    /// transaction, which is not its transaction id.
    #[test]
    fn the_body_key_of_a_transaction_is_not_its_id() {
        let f = &FIXTURES[2];
        let body = EndorserBlockBody::decode_announced(&f.body_bytes(), &f.announcement()).unwrap();

        let (key, size) = body.entries()[0];
        assert_eq!(
            key.to_string(),
            "2bc50f5b4942ca304e39e7cd7f1c4261d85437ea5464b2d1a623f47790254a3d"
        );
        assert_eq!(size, 201);

        let wire = f.wire_txs();
        let txs = body.transactions(&wire).unwrap();

        assert_eq!(
            txs[0].hash().to_string(),
            "82bfbdf62f180269e15a72672b51610bc17a043c3d6f5c5a2434403b00a49d98"
        );

        let MultiEraTx::Dijkstra(tx) = &txs[0] else {
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

    /// The node does not compare a transaction with the size its entry names.
    #[test]
    fn a_transaction_is_accepted_whatever_size_its_entry_names() {
        let (_, wire) = fixture(0);
        let inner = unwrapped(&wire[0]);
        let id = Hasher::<256>::hash(inner);

        for size in [0, inner.len() as u32 - 1, inner.len() as u32 + 1, u32::MAX] {
            let body = announced_as_itself(&encode_entries(&[(id, size)]))
                .expect("a body of one entry");

            let txs = body
                .transactions(&wire)
                .unwrap_or_else(|e| panic!("named size {size}: {e}"));
            assert_eq!(txs[0].hash().to_string(), FIXTURES[0].first_tx_id);
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
        assert_eq!(body.transactions(&wire).unwrap().len(), 1);

        let inner = unwrapped(&wire[0]);
        assert_eq!(inner.len(), 229);
        assert_ne!(inner.len(), wire[0].len(), "the envelope is real");

        let err = body
            .transactions(&[AnyCbor::from(inner.to_vec())])
            .expect_err("an unwrapped transaction must be refused");

        assert!(matches!(err, Error::TxDecode { index: 0, .. }), "{err}");
    }

    /// The node reads an indefinite length transaction as it reads the
    /// definite one, with or without the `true` third element.
    #[test]
    fn an_indefinite_length_transaction_reads_as_the_definite_one() {
        let (_, wire) = fixture(0);
        let three = unwrapped(&wire[0]);
        assert_eq!(three[0], 0x83, "fixture precondition");

        let mempool: dijkstra::MempoolTransaction = minicbor::decode(three).unwrap();
        let four = minicbor::to_vec(dijkstra::MempoolTransaction {
            is_valid_supplied: true,
            ..mempool
        })
        .unwrap();
        assert_eq!(four[0], 0x84, "the four element form");

        for definite in [three, four.as_slice()] {
            let mut indefinite = vec![0x9f];
            indefinite.extend_from_slice(&definite[1..]);
            indefinite.push(0xff);

            let wire = [enveloped(&indefinite)];
            let txs = body_naming(&[&indefinite])
                .transactions(&wire)
                .unwrap_or_else(|e| panic!("{:02x} form: {e}", definite[0]));

            assert_eq!(txs[0].hash().to_string(), FIXTURES[0].first_tx_id);
        }
    }

    /// The Dijkstra era decoder also reads the four element block form.
    #[test]
    fn a_block_form_transaction_is_accepted() {
        let (_, wire) = fixture(0);
        let inner = unwrapped(&wire[0]);
        assert_eq!(inner[0], 0x83, "fixture precondition");

        let mempool: dijkstra::MempoolTransaction = minicbor::decode(inner).unwrap();
        let block_form = minicbor::to_vec(dijkstra::BlockTransaction::from(&mempool)).unwrap();
        assert_eq!(block_form[0], 0x84, "the block form has four elements");

        let wire = [enveloped(&block_form)];
        let txs = body_naming(&[&block_form])
            .transactions(&wire)
            .expect("a block form transaction");

        assert_eq!(txs[0].hash().to_string(), FIXTURES[0].first_tx_id);
    }

    /// The Dijkstra era decoder stops after one transaction.
    #[test]
    fn a_transaction_with_bytes_after_it_inside_its_envelope_is_accepted() {
        let (_, wire) = fixture(0);
        let mut trailing = unwrapped(&wire[0]).to_vec();
        trailing.push(0x00);

        let wire = [enveloped(&trailing)];
        let txs = body_naming(&[&trailing])
            .transactions(&wire)
            .expect("the transaction before the trailing byte");

        assert_eq!(txs[0].hash().to_string(), FIXTURES[0].first_tx_id);
    }

    #[test]
    fn a_body_naming_one_transaction_twice_is_refused() {
        let (real, _) = fixture(2);
        let first = real.entries()[0];
        let second = real.entries()[1];
        assert_ne!(first.0, second.0, "fixture precondition");

        let distinct = announced_as_itself(&encode_entries(&[first, second]))
            .expect("two distinct entries must be accepted");
        assert_eq!(distinct.len(), 2);

        let err = announced_as_itself(&encode_entries(&[first, second, first]))
            .expect_err("a body naming one transaction twice must be refused");

        match err {
            Error::DuplicateTx {
                hash,
                first: at,
                second: again,
            } => {
                assert_eq!(hash, first.0);
                assert_eq!(at, 0);
                assert_eq!(again, 2);
            }
            other => panic!("wrong refusal: {other}"),
        }
    }

    /// Re-encodes a real block's header with the two Leios fields set, which
    /// leaves the header signature no longer covering the header body.
    fn with_leios_header_fields(
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

    fn header_of(block_cbor: &[u8]) -> Vec<u8> {
        let block = MultiEraBlock::decode(block_cbor).unwrap();
        block.header().cbor().to_vec()
    }

    fn dijkstra_header(raw: &[u8]) -> MultiEraHeader<'_> {
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
            ),
            (
                "de5f4b812d0e6dc3129510c6663de4ea99bbd6bf019ec2d541853242926fd446",
                39495,
            ),
            (
                "df3e00644db8fb9020057eab4d130befab8e7f2eb2a69af008116ccc8c2f0527",
                43312,
            ),
            (
                "b0233a1c2608013cec07e933e3d0101f48dc75d785ee9b096bee94cd8713a72d",
                59439,
            ),
        ];

        for (pair, (hash, size)) in run.windows(2).zip(expected) {
            let certified = certification(Some(&pair[0]), &pair[1])
                .unwrap_or_else(|e| panic!("block {}: {e}", pair[1].number()))
                .expect("every header after 17402 certifies");

            assert_eq!(
                certified.eb_hash,
                hash_of(hash),
                "block {}",
                pair[1].number()
            );
            assert_eq!(certified.eb_size, size, "block {}", pair[1].number());
        }

        let parent = fixture_header(include_str!("../../test_data/dijkstra-17511.header"));
        let parent = dijkstra_header(&parent);
        let child = block_header(include_str!("../../test_data/dijkstra9.block"));
        let certified = certification(Some(&parent), &dijkstra_header(&child))
            .unwrap()
            .expect("17512 certifies");

        assert_eq!(
            certified.eb_hash,
            hash_of("2abe14e0dd956846cd06b786b36380a608488b265790a55676af90f0ebe86201")
        );
        assert_eq!(certified.eb_size, 24772);
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
    fn a_header_that_does_not_certify_after_a_parent_that_announced_certifies_nothing() {
        let parent = fixture_header(include_str!("../../test_data/dijkstra-17513.header"));
        let parent = dijkstra_header(&parent);
        assert_eq!(
            parent.eb_announcement().map(|a| a.eb_size),
            Some(29487),
            "fixture precondition"
        );

        let child = fixture_header(include_str!("../../test_data/dijkstra-17514.header"));
        let child = dijkstra_header(&child);
        assert_eq!(
            child.block_body_contains_leios_cert(),
            Some(false),
            "fixture precondition"
        );

        let out = certification(Some(&parent), &child).unwrap();

        assert_eq!(out, None);
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
    fn a_header_that_does_not_certify_given_a_parent_it_does_not_name_is_refused() {
        let other = block_header(include_str!("../../test_data/dijkstra16.block"));
        let other = dijkstra_header(&other);
        let header = fixture_header(include_str!("../../test_data/dijkstra-17514.header"));
        let header = dijkstra_header(&header);
        assert_eq!(
            header.block_body_contains_leios_cert(),
            Some(false),
            "fixture precondition"
        );

        let err = certification(Some(&other), &header).expect_err("a wrong parent must be refused");

        match err {
            Error::NotParent {
                slot,
                previous,
                parent,
            } => {
                assert_eq!(slot, 374322);
                assert_eq!(
                    previous,
                    Some(hash_of(
                        "68d831715019aa837971400882b6a435a4cafbf61c560821eb8b06230bda9660"
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

        let conway = conway.header();
        let out = certification(Some(&conway), &first.header()).unwrap();

        assert_eq!(out, None);
    }

    /// A certifying block of `test_data/dijkstra-fixtures.md` as node to node
    /// block-fetch delivers it, its parent header, the endorser block that
    /// parent announced, and the ids
    /// `cardano-cli` gives that endorser block's first and last transactions.
    struct Certified {
        name: &'static str,
        block: &'static str,
        parent: &'static str,
        body: &'static str,
        txs: &'static str,
        count: usize,
        first_tx_id: &'static str,
        last_tx_id: &'static str,
    }

    const CERTIFIED: &[Certified] = &[Certified {
        name: "dijkstra8",
        block: include_str!("../../test_data/dijkstra8.block"),
        parent: include_str!("../../test_data/dijkstra-17402.header"),
        body: include_str!("../../test_data/dijkstra-17402.ebbody"),
        txs: include_str!("../../test_data/dijkstra-17402.ebtxs"),
        count: 244,
        first_tx_id: "82bfbdf62f180269e15a72672b51610bc17a043c3d6f5c5a2434403b00a49d98",
        last_tx_id: "7c165b9f5287113a811c8e6276b57ffb89fb0d9daf9bfe028310e68408c7f756",
    }];

    #[test]
    fn each_real_certifying_block_names_the_endorser_block_whose_transactions_it_lacks() {
        for c in CERTIFIED {
            let name = c.name;
            let raw = hex::decode(c.block.trim()).unwrap();
            let block = MultiEraBlock::decode(&raw).unwrap();
            assert_eq!(block.tx_count(), 0, "{name} carries no transactions");

            let parent = hex::decode(c.parent.trim()).unwrap();
            let parent = dijkstra_header(&parent);
            let announcement = parent.eb_announcement().expect("the parent announces");

            let certified = certification(Some(&parent), &block.header())
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .unwrap_or_else(|| panic!("{name} certifies"));
            assert_eq!(certified, announcement, "{name}");

            let body_cbor = hex::decode(c.body.trim()).unwrap();
            let body = EndorserBlockBody::decode_announced(&body_cbor, certified)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let wire = wire_txs(c.txs);

            let txs = body
                .transactions(&wire)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(txs.len(), c.count, "{name}");
            assert_eq!(txs[0].hash().to_string(), c.first_tx_id, "{name}");
            assert_eq!(txs[c.count - 1].hash().to_string(), c.last_tx_id, "{name}");
        }
    }
}
