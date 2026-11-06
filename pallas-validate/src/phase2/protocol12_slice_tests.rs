//! Raw programs at protocol 12, against the results the Haskell ledger's script evaluation gave them.
use super::error::{Error, Unsupported};
use super::native_evaluator::eval_native;
use pallas_codec::minicbor;
use pallas_primitives::conway::{ExUnits, PlutusData};
use pallas_primitives::dijkstra::Language;

/// The programs the native evaluation refuses, each with the cause it names.
const REFUSED: [(&str, &str); 2] = [
    (
        "v3-unaudited-builtin",
        "V3 builtin outside audited protocol-12 subset",
    ),
    (
        "v3-integer-above-bound",
        "integer operand outside audited registration range",
    ),
];

/// The verdict on one program.
#[derive(Debug, PartialEq)]
enum Verdict {
    Agree,
    Refused(&'static str),
    Disagree(String),
}

/// The programs of the slice, one JSON object per line.
fn rows() -> Vec<serde_json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_data/protocol-12-uplc/slice.jsonl");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The bytes of a base 16 JSON string.
fn bytes(value: &serde_json::Value) -> Vec<u8> {
    hex::decode(value.as_str().unwrap()).unwrap()
}

/// The pair of a JSON array of two unsigned integers.
fn pair(value: &serde_json::Value) -> (u64, u64) {
    (value[0].as_u64().unwrap(), value[1].as_u64().unwrap())
}

/// Evaluates a program natively and compares the result with the ledger's.
fn judge(row: &serde_json::Value) -> Verdict {
    let language = match row["language"].as_str().unwrap() {
        "PlutusV1" => Language::PlutusV1,
        "PlutusV2" => Language::PlutusV2,
        "PlutusV3" => Language::PlutusV3,
        other => panic!("language {other}"),
    };
    let args: Vec<PlutusData> = row["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| minicbor::decode(&bytes(arg)).unwrap())
        .collect();
    let costs: Vec<i64> = serde_json::from_value(row["cost_model"].clone()).unwrap();
    let (mem, steps) = pair(&row["budget"]);
    let got = eval_native(
        language,
        &bytes(&row["script"]),
        &args.iter().collect::<Vec<_>>(),
        &costs,
        ExUnits { mem, steps },
    );
    let want = &row["result"];
    match (got, want.get("ran")) {
        (Err(Error::DijkstraUnsupported(Unsupported(cause))), Some(_)) => Verdict::Refused(cause),
        (Ok((result, _)), Some(ran))
            if result.success && (result.units.mem, result.units.steps) == pair(ran) =>
        {
            Verdict::Agree
        }
        (Ok((result, _)), None) if want.get("failed").is_some() && !result.success => {
            Verdict::Agree
        }
        (got, _) => Verdict::Disagree(format!("{got:?} against {want}")),
    }
}

#[test]
fn protocol_12_slice_agrees_with_the_ledger() {
    let rows = rows();
    let mut agree = 0;
    let mut refused = Vec::new();
    let mut disagree = Vec::new();
    for row in &rows {
        let name = row["name"].as_str().unwrap();
        match judge(row) {
            Verdict::Agree => agree += 1,
            Verdict::Refused(cause) => refused.push((name, cause)),
            Verdict::Disagree(why) => disagree.push(format!("{name}: {why}")),
        }
    }
    println!(
        "UPLC-ORACLE compared={} agree={agree} refused={} disagree={}",
        rows.len(),
        refused.len(),
        disagree.len()
    );
    assert!(disagree.is_empty(), "{disagree:#?}");
    assert_eq!(refused, REFUSED.to_vec());
}

#[test]
fn a_changed_ledger_result_disagrees() {
    let mut row = rows()
        .into_iter()
        .find(|row| row["name"] == "v3-add")
        .unwrap();
    assert_eq!(judge(&row), Verdict::Agree);
    row["result"]["ran"][1] = (pair(&row["result"]["ran"]).1 + 1).into();
    assert!(matches!(judge(&row), Verdict::Disagree(_)));
}
