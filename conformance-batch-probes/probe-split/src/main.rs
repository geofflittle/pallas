//! Maps generated Dijkstra blocks and transactions with the v1alpha and v1beta mappers.
//!
//! Reads the generator's JSON lines for one kind on stdin and prints one line
//! per value and mapper: `i`, the mapper, and `ok`, `decode error` or the panic text.
use std::io::BufRead;
use std::panic::{AssertUnwindSafe, catch_unwind};

use pallas_traverse::{Era, MultiEraBlock, MultiEraTx};
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};

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

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "non text panic".to_string()
    }
}

fn outcome(run: impl FnOnce() -> Result<(), String>) -> String {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(())) => "ok".to_string(),
        Ok(Err(e)) => format!("decode error {e}"),
        Err(e) => format!("PANIC {}", panic_text(e)),
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let kind = std::env::args().nth(1).expect("usage: probe-split block|tx");
    let alpha = pallas_utxorpc::v1alpha::Mapper::new(NoLedger);
    let beta = pallas_utxorpc::v1beta::Mapper::new(NoLedger);
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("stdin line");
        if line.trim().is_empty() {
            continue;
        }
        let input: serde_json::Value = serde_json::from_str(&line).expect("json line");
        let bytes = hex::decode(input["hex"].as_str().expect("hex field")).expect("hex");
        let i = &input["i"];
        match kind.as_str() {
            "block" => {
                let mut wrapped = vec![0x82, 0x08];
                wrapped.extend_from_slice(&bytes);
                let a = outcome(|| {
                    let b = MultiEraBlock::decode_dijkstra(&wrapped).map_err(|e| e.to_string())?;
                    alpha.map_block(&b);
                    Ok(())
                });
                let v = outcome(|| {
                    let b = MultiEraBlock::decode_dijkstra(&wrapped).map_err(|e| e.to_string())?;
                    beta.map_block(&b);
                    Ok(())
                });
                println!("{i} v1alpha {a}");
                println!("{i} v1beta {v}");
            }
            "tx" => {
                let a = outcome(|| {
                    let t = MultiEraTx::decode_for_era(Era::Dijkstra, &bytes)
                        .map_err(|e| e.to_string())?;
                    alpha.map_tx(&t);
                    Ok(())
                });
                let v = outcome(|| {
                    let t = MultiEraTx::decode_for_era(Era::Dijkstra, &bytes)
                        .map_err(|e| e.to_string())?;
                    beta.map_tx(&t);
                    Ok(())
                });
                println!("{i} v1alpha {a}");
                println!("{i} v1beta {v}");
            }
            k => panic!("unknown kind {k}"),
        }
    }
}
