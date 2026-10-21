// Prints, for every block of a chain dump mapped through the v1 schema, a
// line with the slot, the hash and a digest of the whole mapped block, then
// one JSON row per transaction and one per sub transaction, so the output
// of two pallas trees can be diffed with cmprows.py.

use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};

use base64::Engine;
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use prost::Message;
use serde_json::{Value, json};

use pallas_utxorpc::v1 as ver;

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

fn row(common: Value) -> Value {
    json!({
        "hash": common["hash"],
        "outputs": common["outputs"],
        "mint": common["mint"],
        "withdrawals": common["withdrawals"].as_array().map(|ws| ws.iter().map(|w| w["rewardAccount"].clone()).collect::<Vec<_>>()),
        "input_refs": common["inputs"].as_array().map(|is| is.iter().map(|i| json!([i["txHash"], i["outputIndex"]])).collect::<Vec<_>>()),
    })
}

fn main() {
    let path = std::env::args().nth(1).expect("dump path");
    let file = std::fs::File::open(&path).expect("open dump");
    let b64 = base64::engine::general_purpose::STANDARD;
    let mapper = ver::Mapper::new(NoLedger);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in std::io::BufReader::new(file).lines() {
        let line = line.expect("line");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cbor = b64.decode(line).expect("base64");
        let block = mapper.map_block_cbor(&cbor);
        // With FULL_SLOT set, print only the whole mapped block at that slot as JSON.
        if let Ok(slot) = std::env::var("FULL_SLOT") {
            if block.header.as_ref().map(|h| h.slot.to_string()) == Some(slot) {
                writeln!(out, "{}", serde_json::to_string_pretty(&block).unwrap()).unwrap();
            }
            continue;
        }
        let wire = block.encode_to_vec();
        let mut digest = std::collections::hash_map::DefaultHasher::new();
        wire.hash(&mut digest);
        let header = block.header.as_ref().expect("header");
        writeln!(
            out,
            "B {} {} {} {:016x}",
            header.slot,
            hex::encode(&header.hash),
            wire.len(),
            digest.finish()
        )
        .unwrap();
        for tx in block.body.map(|b| b.tx).unwrap_or_default() {
            let collateral_return = tx
                .collateral
                .as_ref()
                .and_then(|c| c.collateral_return.as_ref())
                .map(|o| serde_json::to_value(o).unwrap())
                .unwrap_or(Value::Null);
            let mut r = row(serde_json::to_value(tx.common.as_ref().expect("common")).unwrap());
            r["successful"] = json!(tx.successful);
            r["collateral_return"] = collateral_return;
            r["sub_transactions"] = json!(tx.sub_transactions.len());
            writeln!(out, "{}", r).unwrap();
            for sub in &tx.sub_transactions {
                let s = json!({ "sub": row(serde_json::to_value(sub).unwrap()) });
                writeln!(out, "{}", s).unwrap();
            }
        }
    }
}
