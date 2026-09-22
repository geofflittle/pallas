//! The Leios endorser block, defined by the node to node `leios-fetch` CDDL
//! rather than by the ledger CDDL in `defs.cddl`.

use std::ops::Deref;

use pallas_codec::minicbor::{self, Decode, Decoder, Encode, Encoder};
use pallas_crypto::hash::Hash;
use serde::{Deserialize, Serialize};

/// `endorser_block = { * base.hash => base.word32 }`: the blake2b-256 and byte
/// length of each transaction the block names. Pairs rather than a sorted map,
/// because the announced hash covers the wire order, and a definite length map
/// only, the one form the node decodes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EndorserBlock(Vec<(Hash<32>, u32)>);

impl Deref for EndorserBlock {
    type Target = [(Hash<32>, u32)];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'b, C> Decode<'b, C> for EndorserBlock {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d.map()?.ok_or_else(|| {
            minicbor::decode::Error::message("an endorser block is a definite length map")
        })?;

        // No capacity from `len`, which the sending peer chooses.
        let mut entries = Vec::new();
        for _ in 0..len {
            entries.push((d.decode_with(ctx)?, d.decode_with(ctx)?));
        }

        Ok(Self(entries))
    }
}

impl<C> Encode<C> for EndorserBlock {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.map(self.0.len() as u64)?;

        for (hash, size) in &self.0 {
            e.encode_with(hash, ctx)?;
            e.encode_with(size, ctx)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Name, body, and the count, first key and first size recorded for it in
    /// `test_data/dijkstra-fixtures.md`.
    const FIXTURES: &[(&str, &str, usize, &str, u32)] = &[
        (
            "dijkstra-eb1",
            include_str!("../../../test_data/dijkstra-eb1.ebbody"),
            1,
            "a69f9fc581e5914a101a3e619f5ce64b6bce76721fd5eed0cbad0e0f6d411cc5",
            229,
        ),
        (
            "dijkstra-eb2",
            include_str!("../../../test_data/dijkstra-eb2.ebbody"),
            30,
            "455a00b521f35f2c0a6ff0a59296c3316de6219af206c13d2e95870f66541fec",
            200,
        ),
        (
            "dijkstra-eb3",
            include_str!("../../../test_data/dijkstra-eb3.ebbody"),
            425,
            "b6813cef46bc2f5b0ce9761e7177138f93edf6d4b9c95ff28b071a4ce0978d6b",
            201,
        ),
    ];

    fn body(hex_body: &str) -> Vec<u8> {
        hex::decode(hex_body.trim()).expect("fixture body is hex")
    }

    #[test]
    fn a_body_that_is_not_a_map_is_refused() {
        let array = [0x83, 0x01, 0x02, 0x03];

        assert!(minicbor::decode::<EndorserBlock>(&array).is_err());
    }

    fn one_entry_with_key_of(len: u8) -> Vec<u8> {
        let mut body = vec![0xa1, 0x58, len];
        body.extend_from_slice(&vec![0x11; len as usize]);
        body.push(0x01);
        body
    }

    #[test]
    fn a_key_that_is_not_32_bytes_is_refused() {
        let block = minicbor::decode::<EndorserBlock>(&one_entry_with_key_of(32))
            .expect("a 32 byte key must be accepted");
        assert_eq!(block[0], (Hash::new([0x11; 32]), 1));

        for len in [31, 33] {
            let err = minicbor::decode::<EndorserBlock>(&one_entry_with_key_of(len))
                .expect_err("a key that is not 32 bytes must be refused");
            assert!(
                err.to_string().contains(&format!("{len} bytes")),
                "wrong refusal for {len}: {err}"
            );
        }
    }

    #[test]
    fn each_body_decodes_to_its_recorded_count_and_first_entry() {
        for (name, hex_body, count, first_key, first_size) in FIXTURES {
            let block: EndorserBlock =
                minicbor::decode(&body(hex_body)).unwrap_or_else(|e| panic!("{name}: {e}"));

            assert_eq!(block.len(), *count, "{name} transaction count");

            let (key, size) = &block[0];
            assert_eq!(key.to_string(), *first_key, "{name} first key");
            assert_eq!(size, first_size, "{name} first size");
        }
    }

    #[test]
    fn each_body_re_encodes_byte_for_byte() {
        for (name, hex_body, ..) in FIXTURES {
            let raw = body(hex_body);
            let block: EndorserBlock = minicbor::decode(&raw).unwrap();

            assert_eq!(minicbor::to_vec(&block).unwrap(), raw, "{name}");
        }
    }

    #[test]
    fn an_indefinite_body_is_refused() {
        let definite = body(FIXTURES[0].1);
        assert_eq!(definite[0], 0xa1, "fixture precondition");
        assert!(minicbor::decode::<EndorserBlock>(&definite).is_ok());

        let mut indefinite = vec![0xbf];
        indefinite.extend_from_slice(&definite[1..]);
        indefinite.push(0xff);

        let err = minicbor::decode::<EndorserBlock>(&indefinite)
            .expect_err("an indefinite length body must be refused");
        assert!(err.to_string().contains("definite"), "wrong refusal: {err}");
    }
}
