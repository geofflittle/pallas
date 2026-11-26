//! A layer that answers JSON requests, judged against one record row.

use std::sync::Arc;
use std::time::Duration;

use reference_comparison::tree::{self, Normal};
use reference_comparison::{
    Allowance, ComparedLayer, Disagreement, Expected, Key, LayerRun, Outcome, Records, Reference,
    Row, WholeInput, compare,
};
use serde_json::{Value, json};

const PIN: &str = "service-v1";

/// One request for an account and the reference service's answer to it.
const RECORDS: &str = r#"{"reference":"service-v1","generator":"service answers test","layer":"service answers","classes":["account"]}
{"class":"account","input":"7b226163636f756e74223a22616263227d","reference":{"accepts":{"account":"abc","balance":5,"assets":{"set":["gold","silver"]}}}}
"#;

/// A service that answers a request for an account with its balance and the
/// assets it holds, in an order of its own.
struct Service {
    balance: u64,
}

impl Service {
    /// The service's answer to `request`.
    fn answer(&self, request: &Value) -> Value {
        json!({
            "account": request["account"].clone(),
            "balance": self.balance,
            "assets": ["silver", "gold"],
        })
    }
}

/// The value tree of an answer, whose assets are a set.
fn tree_of(answer: &Value) -> Normal {
    let assets = answer["assets"].as_array().into_iter().flatten().cloned();
    let tree = tree::object(vec![
        ("account", answer["account"].clone()),
        ("balance", answer["balance"].clone()),
        ("assets", tree::set(assets)),
    ]);
    tree::normal(&tree, &|_, _| None, &mut Vec::new())
}

impl ComparedLayer for Service {
    const LAYER: &'static str = "service answers";
    const TIME_LIMIT: Duration = Duration::from_secs(5);
    type Input = Value;
    type Axis = WholeInput;
    type Value = Normal;
    type Refusal = String;

    fn input(&self, row: &Row) -> Value {
        serde_json::from_slice(&row.input).expect("a request is JSON")
    }

    fn reference(&self, row: &Row) -> Expected<WholeInput, Normal> {
        let reference = match &row.reference {
            Reference::Accepts(accepted) => {
                Reference::Accepts(tree::normal(&accepted.dump, &|_, _| None, &mut Vec::new()))
            }
            Reference::Refuses(reason) => Reference::Refuses(reason.clone()),
        };
        Expected::one(WholeInput, reference)
    }

    fn run(&self, request: &Value) -> Vec<(WholeInput, Result<Normal, String>)> {
        vec![(WholeInput, Ok(tree_of(&self.answer(request))))]
    }

    fn same_reason(&self, reason: &str, refusal: &String) -> bool {
        reason == refusal
    }

    fn allowance(&self, _: &Row, _: &Key<WholeInput>) -> Option<Allowance<Normal, String>> {
        None
    }
}

/// The run of a service answering with `balance` over `records`.
fn run(balance: u64, records: &Records) -> LayerRun<'_, WholeInput> {
    compare(&Arc::new(Service { balance }), records, &[])
}

#[test]
fn an_equal_answer_in_another_order_agrees() {
    let records = Records::read(RECORDS, PIN).unwrap();
    let run = run(5, &records);
    let outcomes: Vec<_> = run.judgements.iter().map(|j| j.outcome.clone()).collect();
    assert_eq!(outcomes, [Outcome::Agree]);
    assert_eq!(run.failures(), Vec::<String>::new());
}

#[test]
fn an_unequal_answer_disagrees() {
    let records = Records::read(RECORDS, PIN).unwrap();
    let run = run(6, &records);
    let outcomes: Vec<_> = run.judgements.iter().map(|j| j.outcome.clone()).collect();
    assert_eq!(outcomes, [Outcome::Disagree(Disagreement::Values)]);
    assert_eq!(
        run.failures(),
        ["service answers: class \"account\" disagrees (values) 1"]
    );
}

#[test]
fn records_of_another_layer_fail() {
    let text = RECORDS.replace("\"service answers\"", "\"decode\"");
    let records = Records::read(&text, PIN).unwrap();
    let run = run(5, &records);
    assert!(run.judgements.is_empty());
    assert_eq!(
        run.failures(),
        [
            "the records are for decode, not service answers",
            "service answers: no observation judged",
            "service answers: class \"account\" has no judged observation",
        ]
    );
}
