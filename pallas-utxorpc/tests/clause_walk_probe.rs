//! Probe: cost of a native script walk built on MultiEraNativeScript::clause().
//!
//! Run: cargo test -p pallas-utxorpc --test clause_walk_probe -- --nocapture
//! Expected verdict line: "clause walk grows quadratically" with the per depth
//! time and peak memory figures printed above it.

use std::time::Instant;

use pallas_primitives::alonzo::NativeScript;
use pallas_traverse::{MultiEraNativeClause, MultiEraNativeScript};

fn chain(depth: usize) -> NativeScript {
    let mut script = NativeScript::ScriptPubkey([0; 28].into());
    for _ in 0..depth {
        script = NativeScript::ScriptAll(vec![script]);
    }
    script
}

fn clause_walk(script: &MultiEraNativeScript) -> usize {
    match script.clause() {
        MultiEraNativeClause::All(xs)
        | MultiEraNativeClause::Any(xs)
        | MultiEraNativeClause::NOfK(_, xs) => 1 + xs.iter().map(clause_walk).sum::<usize>(),
        _ => 1,
    }
}

fn fold_walk(script: &NativeScript) -> usize {
    pallas_codec::tree::fold_tree(script, |_, children: Vec<usize>| {
        1 + children.into_iter().sum::<usize>()
    })
}

fn peak_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}

#[test]
fn clause_walk_cost_by_depth() {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(|| {
            let mut times = vec![];
            for depth in [500usize, 1000, 2000, 4000] {
                let raw = chain(depth);

                let t = Instant::now();
                let folded = fold_walk(&raw);
                let fold_ms = t.elapsed().as_secs_f64() * 1e3;

                let wrapped = MultiEraNativeScript::from_decoded_alonzo_compatible(&raw);
                let before = peak_kib();
                let t = Instant::now();
                let walked = clause_walk(&wrapped);
                let clause_ms = t.elapsed().as_secs_f64() * 1e3;
                let after = peak_kib();

                assert_eq!(folded, walked, "both walks visit every node");
                println!(
                    "depth {depth}: fold {fold_ms:.2} ms, clause {clause_ms:.2} ms, peak rss {before} to {after} KiB"
                );
                times.push(clause_ms);
                std::mem::forget(wrapped);
            }
            let ratio = times[3] / times[2];
            println!("clause time ratio from depth 2000 to 4000: {ratio:.2}");
            if ratio > 3.0 {
                println!("clause walk grows quadratically");
            } else {
                println!("clause walk grows at most linearly");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
