//! The Leios endorser block, defined by the node to node `leios-fetch` CDDL
//! rather than by the ledger CDDL in `defs.cddl`.

use pallas_codec::utils::KeyValuePairs;
use pallas_crypto::hash::Hash;

/// `endorser_block = { * hash => word32 }`: the blake2b-256 and byte length of
/// each transaction the block names. Pairs rather than a sorted map, because
/// the announced hash covers the wire order.

#[cfg(test)]
mod tests {
    use super::*;
    use pallas_codec::minicbor;

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

    #[cfg(not(feature = "relaxed"))]
    #[test]
    fn a_key_that_is_not_32_bytes_is_refused() {
        let mut short = vec![0xa1, 0x58, 31];
        short.extend_from_slice(&[0x11; 31]);
        short.push(0x01);

        assert!(minicbor::decode::<EndorserBlock>(&short).is_err());
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
    fn an_indefinite_body_re_encodes_indefinite() {
        let definite = body(FIXTURES[0].1);
        assert_eq!(definite[0], 0xa1, "fixture precondition");

        let mut indefinite = vec![0xbf];
        indefinite.extend_from_slice(&definite[1..]);
        indefinite.push(0xff);

        let block: EndorserBlock = minicbor::decode(&indefinite).unwrap();
        assert!(matches!(block, KeyValuePairs::Indef(_)));
        assert_eq!(minicbor::to_vec(&block).unwrap(), indefinite);
    }
}
