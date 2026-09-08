//! Genesis fields written under `extraConfig` instead of at the top level.
//!
//! Current tooling writes a chain's starting funds, pools and delegations
//! under `extraConfig` and leaves the old top level fields empty. A reader
//! that only looks at the top level sees an empty chain and no error. The
//! shapes and the rule for choosing between the two places are the same in
//! the shelley and conway files, so they live here.

use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// The raw keys of one `extraConfig` entry, before it is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
// serde would otherwise require `T: Default` because of the defaulted
// fields, but `Option<T>` defaults to `None` for any `T`.
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct InjectionRaw<T> {
    #[serde(default)]
    data: Option<T>,
    #[serde(default)]
    file: Option<Vec<String>>,
    #[serde(default)]
    hash: Option<String>,
}

/// One `extraConfig` entry.
///
/// It is one of three things: the data written inline, a pointer to a file
/// holding the data, or nothing. Confusing them gives a genesis that parses
/// cleanly and is wrong.
#[derive(Debug, Clone)]
pub enum Injection<T> {
    /// The data is written inline under `data`.
    Embedded(T),

    /// The data is in a separate file, given as path segments and the hash
    /// the file must have.
    FromFile { file: Vec<String>, hash: String },

    /// No `data` and no `file`, so this entry injects nothing.
    Absent,
}

impl<T> TryFrom<InjectionRaw<T>> for Injection<T> {
    type Error = String;

    fn try_from(raw: InjectionRaw<T>) -> Result<Self, Self::Error> {
        match (raw.data, raw.file) {
            (Some(_), Some(_)) => {
                Err("an injection names both a data payload and a file".to_string())
            }
            (Some(data), None) => Ok(Self::Embedded(data)),
            (None, Some(file)) => {
                let hash = raw
                    .hash
                    .ok_or_else(|| "an injection file is named without its hash".to_string())?;

                Ok(Self::FromFile { file, hash })
            }
            (None, None) => Ok(Self::Absent),
        }
    }
}

impl<'de, T> Deserialize<'de> for Injection<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = InjectionRaw::<T>::deserialize(deserializer)?;

        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

/// Pick which of the two places a genesis field was written in.
///
/// If there is no injection, use the top level value. If the injection points
/// at a file, refuse, because the directory that path is relative to is not
/// known here. If the injection is present and the top level holds entries,
/// refuse, because the file names two sources for one field. That matches the
/// ledger, which refuses an empty injection against a populated top level too.
/// Otherwise use the injection.
///
/// `injected_name` is the key under `extraConfig` and `top_level_name` is
/// the field it replaces. Both are only used in error messages.
pub fn resolve<K, V>(
    injected_name: &str,
    top_level_name: &str,
    injection: Option<Injection<HashMap<K, V>>>,
    top_level: Option<HashMap<K, V>>,
) -> Result<Option<HashMap<K, V>>, String> {
    let injected = match injection {
        None | Some(Injection::Absent) => return Ok(top_level),
        Some(Injection::FromFile { file, .. }) => {
            return Err(format!(
                "extraConfig.{injected_name} names an injection file ({}), which cannot be read while parsing",
                file.join("/")
            ));
        }
        Some(Injection::Embedded(injected)) => injected,
    };

    match top_level {
        Some(top_level) if !top_level.is_empty() => Err(format!(
            "extraConfig.{injected_name} and {top_level_name} are both populated, so the genesis names two sources for one field"
        )),
        _ => Ok(Some(injected)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Funds = HashMap<String, u64>;

    fn parse(json: &str) -> Result<Injection<Funds>, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn funds(entries: &[(&str, u64)]) -> Funds {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), *value))
            .collect()
    }

    #[test]
    fn an_embedded_payload_is_read() {
        match parse(r#"{ "data": { "aa": 7 } }"#).expect("the injection must parse") {
            Injection::Embedded(payload) => assert_eq!(payload, funds(&[("aa", 7)])),
            other => panic!("expected an embedded payload, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_object_names_no_injection() {
        match parse("{}").expect("the injection must parse") {
            Injection::Absent => {}
            other => panic!("expected no injection, got {other:?}"),
        }
    }

    #[test]
    fn a_file_arm_keeps_path_and_hash() {
        let json = r#"{ "file": ["genesis", "funds.json"], "hash": "abcd" }"#;

        match parse(json).expect("the injection must parse") {
            Injection::FromFile { file, hash } => {
                assert_eq!(file, vec!["genesis".to_string(), "funds.json".to_string()]);
                assert_eq!(hash, "abcd");
            }
            other => panic!("expected a file injection, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_payload_is_refused() {
        let err = parse(r#"{ "data": { "aa": "not a number" } }"#)
            .expect_err("a payload that does not parse must be refused");

        assert!(err.to_string().contains("invalid type"), "{err}");
    }

    #[test]
    fn both_arms_at_once_is_refused() {
        let json = r#"{ "data": { "aa": 7 }, "file": ["funds.json"], "hash": "abcd" }"#;

        let err = parse(json).expect_err("an injection with two sources must be refused");

        assert!(err.to_string().contains("both a data payload"), "{err}");
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let err = parse(r#"{ "datum": { "aa": 7 } }"#)
            .expect_err("an injection with an unmodelled key must be refused");

        assert!(err.to_string().contains("unknown field"), "{err}");
        assert!(err.to_string().contains("datum"), "{err}");
    }

    #[test]
    fn a_file_arm_needs_its_hash() {
        let err = parse(r#"{ "file": ["funds.json"] }"#)
            .expect_err("a file injection with no hash must be refused");

        assert!(err.to_string().contains("without its hash"), "{err}");
    }

    #[test]
    fn no_injection_reads_the_top_level() {
        let top_level = funds(&[("aa", 7)]);

        let resolved = resolve(
            "initialFunds",
            "initialFunds",
            None,
            Some(top_level.clone()),
        )
        .expect("no injection must resolve");
        assert_eq!(resolved, Some(top_level.clone()));

        let resolved = resolve(
            "initialFunds",
            "initialFunds",
            Some(Injection::Absent),
            Some(top_level.clone()),
        )
        .expect("an absent injection must resolve");
        assert_eq!(resolved, Some(top_level));
    }

    #[test]
    fn an_injection_beats_an_empty_top_level() {
        let injected = funds(&[("bb", 9)]);

        let resolved = resolve(
            "initialFunds",
            "initialFunds",
            Some(Injection::Embedded(injected.clone())),
            Some(Funds::new()),
        )
        .expect("an injection against an empty map must resolve");
        assert_eq!(resolved, Some(injected.clone()));

        let resolved = resolve(
            "initialFunds",
            "initialFunds",
            Some(Injection::Embedded(injected.clone())),
            None,
        )
        .expect("an injection against a missing field must resolve");
        assert_eq!(resolved, Some(injected));
    }

    #[test]
    fn an_empty_payload_against_a_populated_top_level_is_refused() {
        let err = resolve(
            "initialFunds",
            "initialFunds",
            Some(Injection::Embedded(Funds::new())),
            Some(funds(&[("aa", 7)])),
        )
        .expect_err("an empty payload against a populated top level is two sources");

        assert!(err.contains("both populated"), "{err}");
    }

    #[test]
    fn two_empty_sources_stay_empty() {
        let resolved = resolve(
            "stakePools",
            "staking.pools",
            Some(Injection::Embedded(Funds::new())),
            Some(Funds::new()),
        )
        .expect("two empty sources must resolve");

        assert_eq!(resolved, Some(Funds::new()));
    }

    // Disjoint entries on purpose, so the refusal rests on both maps being
    // populated rather than on their keys clashing.
    #[test]
    fn both_sources_populated_is_refused() {
        let err = resolve(
            "initialFunds",
            "initialFunds",
            Some(Injection::Embedded(funds(&[("bb", 9)]))),
            Some(funds(&[("aa", 7)])),
        )
        .expect_err("a field with two sources must be refused");

        assert!(err.contains("initialFunds"), "{err}");
        assert!(err.contains("both populated"), "{err}");
    }

    #[test]
    fn a_file_arm_is_refused() {
        let err = resolve(
            "stakePools",
            "staking.pools",
            Some(Injection::FromFile {
                file: vec!["genesis".to_string(), "pools.json".to_string()],
                hash: "abcd".to_string(),
            }),
            Some(Funds::new()),
        )
        .expect_err("an injection this cannot read must be refused");

        assert!(err.contains("stakePools"), "{err}");
        assert!(err.contains("genesis/pools.json"), "{err}");
    }
}
