//! Lightweight inspection of block data without full CBOR decoding

use pallas_codec::minicbor::{data::Token, decode::Tokenizer};

#[cfg(feature = "unstable")]
use pallas_codec::minicbor::{self, data::Type};

use crate::Era;

#[derive(Debug)]
pub enum Outcome {
    Matched(Era),
    EpochBoundary,
    Inconclusive,
}

/// Which transaction rule a CBOR value's shape matches.
///
/// Every era from Shelley through Conway writes the validity flag third and
/// the auxiliary data fourth. Dijkstra writes them the other way round, and
/// adds a three element rule with no validity flag at all. The position of
/// the flag is therefore the whole signal, and it is read here rather than
/// inferred from which decoder was tried first.
#[cfg(feature = "unstable")]
#[derive(Debug, PartialEq, Eq)]
pub enum TxShape {
    /// `[body, witness_set, auxiliary_data/ nil, bool]`, Dijkstra's
    /// `block_transaction`.
    DijkstraBlock,
    /// `[body, witness_set, auxiliary_data/ nil]`, Dijkstra's
    /// `mempool_transaction`.
    DijkstraMempool,
    /// `[body, witness_set, bool, auxiliary_data/ nil]`, which is every era
    /// from Shelley through Conway.
    ValidityThird,
    /// No transaction rule this crate models has this shape. Byron's payload
    /// lands here, and so does anything malformed.
    Other,
}

/// Read a transaction's shape without decoding its body or witness set.
#[cfg(feature = "unstable")]
pub fn tx_shape(cbor: &[u8]) -> TxShape {
    fn read(cbor: &[u8]) -> Result<TxShape, minicbor::decode::Error> {
        let mut d = minicbor::Decoder::new(cbor);

        let len = match d.array()? {
            Some(len) => len,
            // An indefinite length array is a shape no node writes for a
            // transaction, and skipping to a position in one says nothing.
            None => return Ok(TxShape::Other),
        };

        if len != 3 && len != 4 {
            return Ok(TxShape::Other);
        }

        d.skip()?;
        d.skip()?;

        let third_is_bool = d.datatype()? == Type::Bool;

        if len == 3 {
            return Ok(if third_is_bool {
                TxShape::Other
            } else {
                TxShape::DijkstraMempool
            });
        }

        if third_is_bool {
            return Ok(TxShape::ValidityThird);
        }

        d.skip()?;

        Ok(if d.datatype()? == Type::Bool {
            TxShape::DijkstraBlock
        } else {
            TxShape::Other
        })
    }

    read(cbor).unwrap_or(TxShape::Other)
}

// Executes a very lightweight inspection of the initial tokens of the CBOR
// block payload to extract the tag of the block wrapper which defines the era
// of the contained bytes.
pub fn block_era(cbor: &[u8]) -> Outcome {
    let mut tokenizer = Tokenizer::new(cbor);

    if !matches!(tokenizer.next(), Some(Ok(Token::Array(2)))) {
        return Outcome::Inconclusive;
    }

    match tokenizer.next() {
        Some(Ok(Token::U8(variant))) => match variant {
            0 => Outcome::EpochBoundary,
            1 => Outcome::Matched(Era::Byron),
            2 => Outcome::Matched(Era::Shelley),
            3 => Outcome::Matched(Era::Allegra),
            4 => Outcome::Matched(Era::Mary),
            5 => Outcome::Matched(Era::Alonzo),
            6 => Outcome::Matched(Era::Babbage),
            7 => Outcome::Matched(Era::Conway),
            #[cfg(feature = "unstable")]
            8 => Outcome::Matched(Era::Dijkstra),
            _ => Outcome::Inconclusive,
        },
        _ => Outcome::Inconclusive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_block_detected() {
        let block_str = include_str!("../../test_data/genesis.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::EpochBoundary));
    }

    #[test]
    fn byron_block_detected() {
        let block_str = include_str!("../../test_data/byron1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Byron)));
    }

    #[test]
    fn shelley_block_detected() {
        let block_str = include_str!("../../test_data/shelley1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Shelley)));
    }

    #[test]
    fn allegra_block_detected() {
        let block_str = include_str!("../../test_data/allegra1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Allegra)));
    }

    #[test]
    fn mary_block_detected() {
        let block_str = include_str!("../../test_data/mary1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Mary)));
    }

    #[test]
    fn alonzo_block_detected() {
        let block_str = include_str!("../../test_data/alonzo1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Alonzo)));
    }

    #[test]
    fn babbage_block_detected() {
        let block_str = include_str!("../../test_data/babbage1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Babbage)));
    }

    #[test]
    fn conway_block_detected() {
        let block_str = include_str!("../../test_data/conway1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Conway)));
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_block_detected() {
        let block_str = include_str!("../../test_data/dijkstra1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Matched(Era::Dijkstra)));
    }

    /// The same fixture, without the era compiled in. Tag 8 is then a tag this
    /// crate does not know, and saying so is what keeps the gate from leaving
    /// an era number behind with no types to decode into.
    #[cfg(not(feature = "unstable"))]
    #[test]
    fn dijkstra_block_is_inconclusive_without_the_feature() {
        let block_str = include_str!("../../test_data/dijkstra1.block");
        let bytes = hex::decode(block_str).unwrap();

        let inference = block_era(bytes.as_slice());

        assert!(matches!(inference, Outcome::Inconclusive));
    }

    #[cfg(feature = "unstable")]
    fn first_tx_bytes(block_str: &str) -> Vec<u8> {
        let cbor = hex::decode(block_str).unwrap();
        let block = crate::MultiEraBlock::decode(&cbor).unwrap();
        block.txs().first().unwrap().encode()
    }

    /// A Dijkstra block transaction reads as one, and a Conway transaction of
    /// the same length reads as the shape every era before Dijkstra writes.
    ///
    /// This is what era detection on the transaction path rests on, so the
    /// two are asserted against each other rather than one at a time.
    #[cfg(feature = "unstable")]
    #[test]
    fn the_validity_flag_position_tells_the_two_four_element_shapes_apart() {
        let dijkstra = first_tx_bytes(include_str!("../../test_data/dijkstra3.block"));
        assert_eq!(tx_shape(&dijkstra), TxShape::DijkstraBlock);

        let conway = first_tx_bytes(include_str!("../../test_data/conway1.block"));
        assert_eq!(tx_shape(&conway), TxShape::ValidityThird);

        let babbage = first_tx_bytes(include_str!("../../test_data/babbage6.block"));
        assert_eq!(tx_shape(&babbage), TxShape::ValidityThird);
    }

    /// A three element transaction is the mempool rule, which no era before
    /// Dijkstra has.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_three_element_transaction_is_the_mempool_shape() {
        let dijkstra = first_tx_bytes(include_str!("../../test_data/dijkstra3.block"));
        let block_tx: pallas_primitives::dijkstra::BlockTransaction =
            minicbor::decode(&dijkstra).unwrap();

        let mempool = minicbor::to_vec(block_tx.to_mempool_transaction()).unwrap();
        assert_eq!(tx_shape(&mempool), TxShape::DijkstraMempool);
    }

    /// Shapes no transaction rule this crate models describes are named as
    /// such rather than guessed at, so the decoder falls through to the era
    /// chain instead of being told the wrong era.
    #[cfg(feature = "unstable")]
    #[test]
    fn a_shape_no_rule_describes_is_reported_as_other() {
        for bytes in [
            // an empty array
            vec![0x80],
            // a two element array
            vec![0x82, 0x01, 0x02],
            // a five element array
            vec![0x85, 0x01, 0x02, 0x03, 0x04, 0x05],
            // a map rather than an array
            vec![0xa0],
            // a four element array with no bool at either position
            vec![0x84, 0x01, 0x02, 0x03, 0x04],
            // truncated after the array head
            vec![0x84],
        ] {
            assert_eq!(tx_shape(&bytes), TxShape::Other, "{}", hex::encode(&bytes));
        }
    }
}
