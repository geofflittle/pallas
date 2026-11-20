//! Compares the ledger's decode verdict with pallas-validate Dijkstra phase one
//! on transactions the ledger refuses.
//!
//! usage: probe-validate plant FIXTURE | base FIXTURE N | sign FIXTURE |
//! compare FIXTURE CASES LEDGER

mod fixture;
mod outcome;
mod plant;
mod term;

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use fixture::Fixture;
use outcome::{Ledger, Outcome, Table, Validator, classify};
use serde_json::{Value, json};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let fx = |i: usize| Fixture::load(Path::new(&args[i]));
    match args.get(1).map(String::as_str) {
        Some("plant") => {
            plant::lines(&fx(2)).iter().for_each(|l| println!("{l}"));
            ExitCode::SUCCESS
        }
        Some("base") => {
            let hex = hex::encode(term::write(&fx(2).base()));
            for i in 0..args[3].parse::<usize>().unwrap() {
                println!("{}", json!({ "i": i, "hex": hex }));
            }
            ExitCode::SUCCESS
        }
        Some("sign") => {
            let fx = fx(2);
            let base = term::write(&fx.base());
            let mut out = std::io::stdout().lock();
            for line in std::io::stdin().lock().lines() {
                let g: Value = serde_json::from_str(&line.unwrap()).unwrap();
                let mutation = g["mutation"].as_str().unwrap();
                let bytes = hex::decode(g["hex"].as_str().unwrap()).unwrap();
                let hex = hex::encode(fx.resign(&base, &bytes));
                let kind = format!("generated {mutation}");
                writeln!(
                    out,
                    "{}",
                    json!({ "kind": kind, "variant": mutation, "hex": hex })
                )
                .unwrap();
            }
            ExitCode::SUCCESS
        }
        Some("compare") => compare(&fx(2), Path::new(&args[3]), Path::new(&args[4])),
        _ => {
            eprintln!(
                "usage: probe-validate plant FIXTURE | base FIXTURE N | sign FIXTURE | compare FIXTURE CASES LEDGER"
            );
            ExitCode::from(2)
        }
    }
}

fn ledger(v: &Value) -> Option<Ledger> {
    match (v.get("dump"), v.get("err")) {
        (Some(_), None) => Some(Ledger::Accepts),
        (None, Some(Value::String(e))) => Some(Ledger::Refuses(e.clone())),
        _ => None,
    }
}

fn short(s: &str) -> String {
    s.chars().take(160).collect()
}

fn compare(fx: &Fixture, cases: &Path, verdicts: &Path) -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let read = |p: &Path| -> Vec<Value> {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let (cases, verdicts) = (read(cases), read(verdicts));
    if cases.len() != verdicts.len() {
        eprintln!(
            "{} cases but {} ledger verdicts",
            cases.len(),
            verdicts.len()
        );
        return ExitCode::from(2);
    }
    let mut table = Table::default();
    let mut variants: BTreeMap<(String, String), BTreeMap<String, usize>> = BTreeMap::new();
    let mut accepted: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    let mut base_agrees = None;
    for (case, verdict) in cases.iter().zip(&verdicts) {
        let kind = case["kind"].as_str().unwrap();
        let variant = case["variant"].as_str().unwrap();
        let hex = case["hex"].as_str().unwrap();
        let Some(l) = ledger(verdict) else {
            eprintln!("ledger line is neither a dump nor an err: {verdict}");
            return ExitCode::from(2);
        };
        let v = fx.validate(&hex::decode(hex).unwrap());
        let outcome = classify(&l, &v);
        if kind == "base" {
            base_agrees = Some(outcome == Outcome::LedgerAccepts(Validator::Accepts));
        }
        let cell = match &l {
            Ledger::Accepts => format!("ledger accepts | validator {}", validator(&v)),
            Ledger::Refuses(r) => format!("ledger {} | validator {}", short(r), validator(&v)),
        };
        *variants
            .entry((kind.into(), variant.into()))
            .or_default()
            .entry(cell)
            .or_default() += 1;
        if let (Outcome::ValidatorAccepts, Ledger::Refuses(r)) = (&outcome, &l) {
            accepted
                .entry(kind.into())
                .or_insert((variant.into(), r.clone(), hex.into()));
        }
        table.add(kind, &outcome);
    }
    println!(
        "{:<28} {:>8} {:>10} {:>10} {:>9} {:>8}",
        "kind", "ledger", "validator", "validator", "validator", "ledger"
    );
    println!(
        "{:<28} {:>8} {:>10} {:>10} {:>9} {:>8}",
        "", "refuses", "malformed", "other", "accepts", "accepts"
    );
    for (kind, c) in &table.kinds {
        println!(
            "{:<28} {:>8} {:>10} {:>10} {:>9} {:>8}",
            kind,
            c.refused_by_ledger,
            c.refused_malformed,
            c.refused_other,
            c.accepted,
            c.ledger_accepts
        );
    }
    println!();
    for ((kind, variant), cells) in &variants {
        for (cell, n) in cells {
            println!("{kind} / {variant}: {n} x {cell}");
        }
    }
    for (kind, (variant, reason, hex)) in &accepted {
        println!();
        println!("accepted example, {kind} / {variant}");
        println!("  ledger: {reason}");
        println!("  tx: {hex}");
    }
    match (base_agrees, table.failed()) {
        (Some(true), true) => {
            println!(
                "\nFAIL: the validator accepts {} kinds the ledger refuses",
                accepted.len()
            );
            ExitCode::FAILURE
        }
        (Some(true), false) => {
            println!("\nPASS: the validator refuses every transaction the ledger refuses");
            ExitCode::SUCCESS
        }
        (Some(false), _) | (None, _) => {
            println!("\nUNUSABLE: the base transaction is not accepted by both");
            ExitCode::from(2)
        }
    }
}

fn validator(v: &Validator) -> String {
    match v {
        Validator::Accepts => "accepts".into(),
        Validator::RefusesMalformed(e) => format!("refuses malformed {}", short(e)),
        Validator::RefusesOther(e) => format!("refuses other {}", short(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> Fixture {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conf/test_data/dijkstra-ttl");
        Fixture::load(&root)
    }

    #[test]
    fn a_ledger_dump_is_an_accept() {
        assert_eq!(
            ledger(&json!({ "dump": { "body": {} } })),
            Some(Ledger::Accepts)
        );
    }

    #[test]
    fn a_ledger_err_is_a_refusal() {
        assert_eq!(
            ledger(&json!({ "err": "bad", "ok": false })),
            Some(Ledger::Refuses("bad".into()))
        );
    }

    #[test]
    fn a_ledger_line_with_neither_or_both_is_no_verdict() {
        assert_eq!(ledger(&json!({ "ok": true })), None);
        assert_eq!(ledger(&json!({ "dump": {}, "err": "bad" })), None);
    }

    #[test]
    fn the_base_transaction_passes_phase_one() {
        let fx = fixture();
        assert_eq!(fx.validate(&term::write(&fx.base())), Validator::Accepts);
    }

    #[test]
    fn a_body_edited_as_bytes_passes_once_resigned() {
        let fx = fixture();
        let base = term::write(&fx.base());
        let mut tx = fx.base();
        let body = &mut tx.items_mut()[0];
        fixture::pay(body, -1);
        body.set(2, term::Term::Int(fixture::FEE + 1));
        let unsigned = term::write(&tx);
        assert!(
            matches!(fx.validate(&unsigned), Validator::RefusesOther(e) if e.contains("Signature") || e.contains("Witness"))
        );
        assert_eq!(
            fx.validate(&fx.resign(&base, &unsigned)),
            Validator::Accepts
        );
    }

    #[test]
    fn every_plant_but_the_base_differs_from_the_base() {
        let fx = fixture();
        let base = term::write(&fx.base());
        let plants = plant::plants(&fx);
        assert_eq!(plants[0].kind, "base");
        assert_eq!(term::write(&plants[0].tx), base);
        assert!(plants[1..].iter().all(|p| term::write(&p.tx) != base));
    }

    #[test]
    fn a_zero_pool_margin_denominator_is_refused_as_malformed() {
        let fx = fixture();
        let p = plant::plants(&fx)
            .into_iter()
            .find(|p| p.variant == "pool margin 1/0")
            .unwrap();
        assert!(matches!(
            fx.validate(&term::write(&p.tx)),
            Validator::RefusesMalformed(_)
        ));
    }
}
