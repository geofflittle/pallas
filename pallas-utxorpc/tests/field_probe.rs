// Field coverage probe. For every w42 sample block and transaction and every
// synthetic one, prints three things per input: what the decoded
// pallas primitives carry, as their Debug text (DECODE), the keys of the body, witness set and
// auxiliary data after a minicbor round trip of the decoded structs (REENC, a
// key the derived decoder dropped is missing here), and the JSON paths the v1
// mapper sets (MAP), or the panic message when mapping panics.
#![cfg(feature = "unstable")]

use pallas_codec::minicbor;
use pallas_traverse as trv;
use pallas_utxorpc::v1::Mapper;
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone)]
struct Nothing;

impl LedgerContext for Nothing {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

fn paths(v: &serde_json::Value, p: &str, out: &mut BTreeSet<String>) {
    use serde_json::Value::*;
    match v {
        Object(o) => {
            if o.is_empty() {
                out.insert(format!("{p}{{}}"));
            }
            for (k, x) in o {
                paths(x, &format!("{p}.{k}"), out);
            }
        }
        Array(a) => {
            for x in a {
                paths(x, &format!("{p}[]"), out);
            }
        }
        Null | Bool(false) => {}
        String(s) if s.is_empty() || s == "0" => {}
        Number(n) if n.as_f64() == Some(0.0) => {}
        _ => {
            out.insert(p.to_string());
        }
    }
}

fn set_of(v: &serde_json::Value) -> Vec<String> {
    let mut out = BTreeSet::new();
    paths(v, "", &mut out);
    out.into_iter().collect()
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| e.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non string panic".into())
}

fn hex_of<T: minicbor::Encode<()>>(x: &T) -> String {
    hex::encode(minicbor::to_vec(x).unwrap())
}

fn report_tx(src: &str, mapper: &Mapper<Nothing>, tx: &trv::MultiEraTx) {
    let bt = tx.as_dijkstra().expect("a Dijkstra transaction");
    let decode = format!("{bt:?}");
    let body: &pallas_primitives::dijkstra::TransactionBody = &bt.transaction_body;
    let wits: &pallas_primitives::dijkstra::WitnessSet = &bt.transaction_witness_set;
    let aux = match &bt.auxiliary_data {
        pallas_primitives::Nullable::Some(a) => {
            let a: &pallas_primitives::dijkstra::AuxiliaryData = a;
            Some(hex_of(a))
        }
        _ => None,
    };
    println!(
        "FP {}",
        serde_json::json!({
            "kind": "DECODE", "src": src, "debug": decode,
        })
    );
    println!(
        "FP {}",
        serde_json::json!({
            "kind": "REENC", "src": src, "body": hex_of(body), "wits": hex_of(wits), "aux": aux,
        })
    );
    match catch_unwind(AssertUnwindSafe(|| mapper.map_tx(tx))) {
        Ok(m) => println!(
            "FP {}",
            serde_json::json!({"kind": "MAP", "src": src, "paths": set_of(&serde_json::to_value(&m).unwrap())})
        ),
        Err(e) => println!(
            "FP {}",
            serde_json::json!({"kind": "MAP", "src": src, "panic": panic_text(e)})
        ),
    }
}

fn report_block(src: &str, mapper: &Mapper<Nothing>, cbor: &[u8]) {
    let block = match trv::MultiEraBlock::decode(cbor) {
        Ok(b) => b,
        Err(e) => {
            println!("FP {}", serde_json::json!({"kind": "BLOCKDECODE", "src": src, "error": e.to_string()}));
            return;
        }
    };
    let b = block.as_dijkstra().unwrap();
    let decode = format!("{:?} {:?} {:?}", b.header.header_body, b.block_body.leios_certificate, b.block_body.peras_certificate);
    println!("FP {}", serde_json::json!({"kind": "BLOCKDECODE", "src": src, "debug": decode}));
    match catch_unwind(AssertUnwindSafe(|| mapper.map_block(&block))) {
        Ok(m) => {
            let header = serde_json::to_value(m.header.as_ref().unwrap()).unwrap();
            let mut body = serde_json::to_value(m.body.as_ref().unwrap()).unwrap();
            body.as_object_mut().unwrap().remove("tx");
            println!(
                "FP {}",
                serde_json::json!({"kind": "BLOCKMAP", "src": src, "header": set_of(&header), "body": set_of(&body), "txs": m.body.as_ref().unwrap().tx.len()})
            );
        }
        Err(e) => println!("FP {}", serde_json::json!({"kind": "BLOCKMAP", "src": src, "panic": panic_text(e)})),
    }
    for (i, tx) in block.txs().iter().enumerate() {
        report_tx(&format!("{src}#{i}"), mapper, tx);
    }
}

fn files(dir: &std::path::Path, prefix: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "cbor"))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    v.sort();
    v
}

#[test]
fn field_probe() {
    let samples = std::path::PathBuf::from(std::env::var("W42_SAMPLES").expect("W42_SAMPLES"));
    let synth = std::path::PathBuf::from(std::env::var("SYNTH").expect("SYNTH"));
    let mapper = Mapper::new(Nothing);

    for path in files(&samples.join("rb"), "rb_").into_iter().chain(files(&samples.join("phase2"), "rb_")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        report_block(&name, &mapper, &std::fs::read(&path).unwrap());
    }
    for path in files(&synth, "block_") {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        report_block(&name, &mapper, &std::fs::read(&path).unwrap());
    }
    for path in files(&samples.join("tx"), "").into_iter().chain(files(&synth, "tx_")) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let cbor = std::fs::read(&path).unwrap();
        match trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor) {
            Ok(tx) => report_tx(&name, &mapper, &tx),
            Err(e) => println!("FP {}", serde_json::json!({"kind": "TXDECODE", "src": name, "error": e.to_string()})),
        }
    }
    println!("FP {}", serde_json::json!({"kind": "END"}));
}
