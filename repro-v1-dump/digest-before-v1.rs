// Prints, for every block of a chain dump, the wire length and a digest of
// the block mapped through v1alpha and through v1beta, so two pallas trees
// can be compared on the schemas before v1.

use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};

use base64::Engine;
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use prost::Message;

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }
    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

fn digest(wire: &[u8]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    wire.hash(&mut h);
    format!("{} {:016x}", wire.len(), h.finish())
}

fn main() {
    let path = std::env::args().nth(1).expect("dump path");
    let file = std::fs::File::open(&path).expect("open dump");
    let b64 = base64::engine::general_purpose::STANDARD;
    let alpha = pallas_utxorpc::v1alpha::Mapper::new(NoLedger);
    let beta = pallas_utxorpc::v1beta::Mapper::new(NoLedger);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("line");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cbor = b64.decode(line).expect("base64");
        let a = alpha.map_block_cbor(&cbor);
        let b = beta.map_block_cbor(&cbor);
        let header = a.header.as_ref().expect("header");
        writeln!(
            out,
            "B {} {} alpha {} beta {}",
            header.slot,
            hex::encode(&header.hash),
            digest(&a.encode_to_vec()),
            digest(&b.encode_to_vec())
        )
        .unwrap();
    }
}
