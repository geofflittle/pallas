//! Dijkstra phase one compared with the ledger decoder on the recorded
//! transactions, judged on those the ledger refuses and on the base
//! transaction both must accept.

mod fixture;
mod term;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ledger_oracle_judge::comparison::{
    Allowance, Allowed, Answer, Commit, ComparedLayer, Disagreement, Expected, Key, Layer, Outcome,
    Owner, Records, Reference, Row, WholeInput, cddl_pin, compare, kinds_from_env,
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

/// Phase one over the fixture's ledger state.
struct PhaseOne {
    fixture: fixture::Fixture,
}

impl ComparedLayer for PhaseOne {
    const LAYER: Layer = Layer::Validation;
    const TIME_LIMIT: Duration = Duration::from_secs(60);
    type Input = Vec<u8>;
    type Axis = WholeInput;
    type Value = ();
    type Refusal = Refusal;

    fn input(&self, row: &Row) -> Vec<u8> {
        row.input.clone()
    }

    fn reference(&self, row: &Row) -> Expected<WholeInput, ()> {
        match (&row.reference, row.class.as_str()) {
            (Reference::Refuses(reason), _) => {
                Expected::one(WholeInput, Reference::Refuses(reason.clone()))
            }
            (Reference::Accepts(_), BASE) => Expected::one(WholeInput, Reference::Accepts(())),
            (Reference::Accepts(_), _) => Expected::NotJudged,
        }
    }

    fn run(&self, input: &Vec<u8>) -> Vec<(WholeInput, Result<(), Refusal>)> {
        vec![(WholeInput, self.fixture.validate(input))]
    }

    fn same_reason(&self, _: &str, refusal: &Refusal) -> bool {
        matches!(refusal, Refusal::Malformed(_))
    }

    fn allowance(&self, key: &Key<WholeInput>) -> Option<Allowance<(), Refusal>> {
        (key.class == EXTRA).then_some(Allowance {
            allowed: EXTRA_ELEMENTS,
            shown: |answer| matches!(answer, Answer::Accepts(())),
        })
    }

    fn detail(&self, row: &Row, reference: &Reference<()>, answer: &Answer<(), Refusal>) -> String {
        let variant = row.variant.as_deref().unwrap_or("");
        format!(
            "{} / {variant}: ledger {reference:?}, phase one {answer:?}",
            row.class
        )
    }
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
    let fixture = fixture::Fixture::load(&root().join("test_data/dijkstra-ttl"));
    let run = compare(&Arc::new(PhaseOne { fixture }), &records, &kinds_from_env());
    let mut examples = BTreeMap::new();
    for j in run
        .judgements
        .iter()
        .filter(|j| j.outcome != Outcome::Agree)
    {
        examples.entry(j.key.class.clone()).or_insert(&j.detail);
    }
    println!("records: {}", records.generator());
    println!("{run}");
    examples.values().for_each(|e| println!("{e}"));
    let failures = run.failures();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
