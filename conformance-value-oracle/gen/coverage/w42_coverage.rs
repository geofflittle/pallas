// Runs the chain oracle's coverage check over the w42 sample blocks and the
// synthetic blocks, one Coverage per input, and prints the locations each
// input held, then every location with its tally and whether the scope
// excludes it.
use pallas_codec::minicbor;
use pallas_traverse as trv;
use pallas_utxorpc::v1::Mapper;
use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use u5c_chain_oracle::coverage::{Coverage, EXCLUDED};

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

fn inner(cbor: &[u8]) -> Vec<u8> {
    let mut d = minicbor::Decoder::new(cbor);
    d.array().unwrap();
    d.u64().unwrap();
    cbor[d.position()..].to_vec()
}

#[test]
fn w42_coverage() {
    let mapper = Mapper::new(Nothing);
    let mut inputs = vec![];
    for var in ["W42_SAMPLES", "SYNTH"] {
        let root = std::path::PathBuf::from(std::env::var(var).unwrap());
        for sub in ["rb", "phase2", "."] {
            let Ok(dir) = std::fs::read_dir(root.join(sub)) else { continue };
            for e in dir {
                let p = e.unwrap().path();
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                if (n.starts_with("rb_") || n.starts_with("blk_") || n.starts_with("block_")) && n.ends_with(".cbor") {
                    inputs.push(p);
                }
            }
        }
    }
    inputs.sort();
    let mut all = Coverage::default();
    for p in inputs {
        let cbor = std::fs::read(&p).unwrap();
        let block = trv::MultiEraBlock::decode(&cbor).unwrap();
        let mapped = mapper.map_block(&block);
        let mut one = Coverage::default();
        one.add(&inner(&cbor), block.number(), &mapped);
        all.add(&inner(&cbor), block.number(), &mapped);
        let failing: Vec<String> = one.failing().iter().map(|(l, t)| format!("{l:?} occ {} dis {} none {}", t.occurrences, t.disagreements, t.without_counterpart)).collect();
        println!("COV {} failing {:?}", p.display(), failing);
        for (l, t) in &one.tallies {
            if t.occurrences > 0 {
                println!("LOC {} {l:?}", p.display());
            }
        }
    }
    for (l, t) in &all.tallies {
        let excluded = EXCLUDED.iter().any(|(e, _)| e == l);
        println!("ALL {l:?} occ {} dis {} none {} excluded {excluded}", t.occurrences, t.disagreements, t.without_counterpart);
    }
    println!("AGREES {}", all.agrees());
    assert!(all.agrees(), "a location u5c does not hold is failing");
}
