//! Dijkstra phase one compared with the ledger decoder on the recorded
//! transactions, judged on those the ledger refuses and on the base
//! transaction both must accept.

mod fixture;
mod term;

use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use ledger_oracle_judge::comparison::{
    Allowed, Answer, Commit, Disagreement, Layer, Outcome, Owner, Records, Reference, Tally, catch,
    cddl_pin, judge,
};

/// Why phase one refuses a transaction.
#[derive(Debug)]
pub enum Refusal {
    Malformed(String),
    Other(String),
}

/// The class of the unchanged transaction, the one input the ledger accepts
/// that phase one must accept.
const BASE: &str = "base";

/// The class of transactions with an extra element in a definite array.
const EXTRA: &str = "generated extra";

/// The extra array elements upstream main decodes and drops on purpose.
const EXTRA_ELEMENTS: Allowed = Allowed::new(
    "extra array elements",
    Disagreement::Accepts,
    Owner::Upstream {
        commit: Commit::new("a8516e16bb38621eeb86b83bef0e146e6123b648"),
    },
);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn phase_one_refuses_what_the_ledger_refuses() {
    let cddl = read("pallas-primitives/src/dijkstra/defs.cddl");
    let pin = cddl_pin(&cddl).expect("defs.cddl names no ledger commit");
    let records = Records::read(
        &read("test_data/reference-comparison/validation.jsonl"),
        pin,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(records.layer(), Layer::Validation);
    let fixture = fixture::Fixture::load(&root().join("test_data/dijkstra-ttl"));
    let mut tally = Tally::new(Layer::Validation, records.classes());
    let mut examples = BTreeMap::new();
    for row in records.rows() {
        let reference = match (&row.reference, row.class.as_str()) {
            (Reference::Refuses(reason), _) => Reference::Refuses(reason.clone()),
            (Reference::Accepts(_), BASE) => Reference::Accepts(()),
            (Reference::Accepts(_), class) => {
                tally.not_judged(class);
                continue;
            }
        };
        let answer = match catch(AssertUnwindSafe(|| fixture.validate(&row.input))) {
            Ok(Ok(())) => Answer::Accepts(()),
            Ok(Err(refusal)) => Answer::Refuses(refusal),
            Err(message) => Answer::Panics(message),
        };
        let outcome = judge(
            &reference,
            &answer,
            |_, refusal| matches!(refusal, Refusal::Malformed(_)),
            (row.class == EXTRA).then_some(EXTRA_ELEMENTS),
        );
        if outcome != Outcome::Agree {
            examples.entry(row.class.clone()).or_insert_with(|| {
                format!(
                    "{} / {}: ledger {:?}, phase one {:?}",
                    row.class,
                    row.variant.as_deref().unwrap_or(""),
                    reference,
                    answer
                )
            });
        }
        tally.add(&row.class, &outcome);
    }
    println!("records: {}", records.generator());
    println!("{tally}");
    examples.values().for_each(|e| println!("{e}"));
    let failures = tally.failures();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
