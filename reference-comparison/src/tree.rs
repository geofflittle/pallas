//! The value tree an implementation and a reference each write for one value,
//! its normal form and the leaves where two normal forms differ.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value, json};

/// The tree of a set, whose order and duplicates the value ignores.
pub fn set(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "set": items.into_iter().collect::<Vec<_>>() })
}

/// The tree of an ordered set, whose duplicates the value ignores.
pub fn ordered_set(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "oset": items.into_iter().collect::<Vec<_>>() })
}

/// The tree of a map, whose order the value ignores and whose later duplicate
/// key wins.
pub fn map(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    json!({ "map": pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>() })
}

/// The tree of an ordered map, whose first duplicate key wins.
pub fn ordered_map(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    json!({ "omap": pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>() })
}

/// The tree of a variant named `tag` with its named fields.
pub fn tagged(tag: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    m.insert("tag".into(), json!(tag));
    for (k, v) in fields {
        m.insert(k.into(), v);
    }
    Value::Object(m)
}

/// The tree of an object of named fields.
pub fn object(fields: Vec<(&str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// A value in the form two trees share exactly when they hold the same value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Normal {
    /// The absent value.
    Null,
    /// A truth value.
    Boolean(bool),
    /// A string or number of the tree, by its text.
    Scalar(String),
    /// An integer, by its decimal digits.
    Integer(String),
    /// A rational, reduced, with a positive denominator.
    Ratio(i128, i128),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A text string.
    Text(String),
    /// A value under a numbered tag.
    Tag(u64, Box<Normal>),
    /// A sequence whose order the value keeps.
    List(Vec<Normal>),
    /// A map whose order the value ignores.
    Map(BTreeMap<Normal, Normal>),
    /// A set whose order the value ignores.
    Set(BTreeSet<Normal>),
    /// A set whose order the value keeps.
    OrderedSet(Vec<Normal>),
    /// A map whose order the value keeps.
    OrderedMap(Vec<(Normal, Normal)>),
    /// An object of named fields.
    Record(BTreeMap<String, Normal>),
    /// A value of a kind no other variant names, by its producer's text for it.
    Other(String),
    /// A leaf its producer could not read, by its text.
    Unreadable(String),
}

/// The reading of a tree's one key object as a leaf, given its key and value,
/// or none when the object is a record.
pub type Leaf<'a> = &'a dyn Fn(&str, &Value) -> Option<Normal>;

/// The normal form of `tree`, reading each one key object the tree's own forms
/// do not name with `leaf` and pushing to `duplicates` the path of each
/// duplicate a set or map removes.
pub fn normal(tree: &Value, leaf: Leaf, duplicates: &mut Vec<String>) -> Normal {
    normal_at(tree, "", leaf, duplicates)
}

fn normal_at(v: &Value, path: &str, leaf: Leaf, duplicates: &mut Vec<String>) -> Normal {
    match v {
        Value::Object(m) if m.len() == 1 && m.contains_key("set") => {
            let items = elements(&m["set"], path, leaf, duplicates);
            let n = items.len();
            let out: BTreeSet<Normal> = items.into_iter().collect();
            if out.len() != n {
                duplicates.push(path.to_string());
            }
            Normal::Set(out)
        }
        Value::Object(m) if m.len() == 1 && m.contains_key("oset") => {
            let mut out: Vec<Normal> = Vec::new();
            for x in elements(&m["oset"], path, leaf, duplicates) {
                if out.contains(&x) {
                    duplicates.push(path.to_string());
                } else {
                    out.push(x);
                }
            }
            Normal::OrderedSet(out)
        }
        Value::Object(m) if m.len() == 1 && (m.contains_key("map") || m.contains_key("omap")) => {
            let ordered = m.contains_key("omap");
            let pairs = m.values().next().and_then(Value::as_array);
            let mut out: Vec<(Normal, Normal)> = Vec::new();
            for pair in pairs.into_iter().flatten() {
                let k = normal_at(&pair[0], path, leaf, duplicates);
                let v = normal_at(&pair[1], path, leaf, duplicates);
                match out.iter().position(|(x, _)| *x == k) {
                    Some(at) => {
                        duplicates.push(path.to_string());
                        if !ordered {
                            out[at].1 = v;
                        }
                    }
                    None => out.push((k, v)),
                }
            }
            if ordered {
                Normal::OrderedMap(out)
            } else {
                Normal::Map(out.into_iter().collect())
            }
        }
        Value::Object(m) => {
            let read = match m.iter().next() {
                Some((key, value)) if m.len() == 1 => leaf(key, value),
                _ => None,
            };
            read.unwrap_or_else(|| {
                Normal::Record(
                    m.iter()
                        .map(|(k, x)| {
                            let at = format!("{path}.{k}");
                            (k.clone(), normal_at(x, &at, leaf, duplicates))
                        })
                        .collect(),
                )
            })
        }
        Value::Array(xs) => Normal::List(
            xs.iter()
                .map(|x| normal_at(x, &format!("{path}[]"), leaf, duplicates))
                .collect(),
        ),
        Value::String(s) => Normal::Scalar(s.clone()),
        Value::Bool(b) => Normal::Boolean(*b),
        Value::Null => Normal::Null,
        Value::Number(n) => Normal::Scalar(n.to_string()),
    }
}

fn elements(v: &Value, path: &str, leaf: Leaf, duplicates: &mut Vec<String>) -> Vec<Normal> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|x| normal_at(x, &format!("{path}[]"), leaf, duplicates))
        .collect()
}

/// How a leaf of the implementation's tree differs from the reference's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difference {
    Lacks,
    Extra,
    Differs,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Difference::Lacks => "the implementation lacks a leaf",
            Difference::Extra => "the implementation has an extra leaf",
            Difference::Differs => "a leaf differs",
        })
    }
}

/// The leaves of two normal forms that agree, and the paths and values of those
/// that differ.
#[derive(Default)]
pub struct Diff {
    pub agree: usize,
    pub differ: Vec<(Difference, String, String)>,
}

impl Diff {
    /// The leaves where `produced`, the implementation's normal form, agrees
    /// with and differs from `reference`.
    pub fn between(reference: &Normal, produced: &Normal) -> Diff {
        let mut out = Diff::default();
        compare(Some(reference), Some(produced), "", &mut out);
        out
    }
}

fn leaves(v: &Normal) -> usize {
    let sum = match v {
        Normal::List(xs) | Normal::OrderedSet(xs) => xs.iter().map(leaves).sum(),
        Normal::Set(xs) => xs.iter().map(leaves).sum(),
        Normal::Map(m) => m.iter().map(|(k, x)| leaves(k) + leaves(x)).sum(),
        Normal::OrderedMap(m) => m.iter().map(|(k, x)| leaves(k) + leaves(x)).sum(),
        Normal::Record(m) => m.values().map(leaves).sum(),
        Normal::Tag(_, x) => leaves(x),
        _ => 1,
    };
    sum.max(1)
}

fn short(v: &Normal) -> String {
    let s = format!("{v:?}");
    if s.len() > 160 {
        format!("{}...", s.chars().take(160).collect::<String>())
    } else {
        s
    }
}

/// The path segment naming a map key or set element: a small integer by its
/// value, anything else by a short hash.
fn segment(k: &Normal) -> String {
    match k {
        Normal::Integer(s) if s.len() < 4 => format!("<{s}>"),
        _ => {
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut h = DefaultHasher::new();
            format!("{k:?}").hash(&mut h);
            format!("{{{:012x}}}", h.finish() & 0xffff_ffff_ffff)
        }
    }
}

fn compare(reference: Option<&Normal>, produced: Option<&Normal>, path: &str, out: &mut Diff) {
    match (reference, produced) {
        (Some(a), Some(b)) if a == b => out.agree += leaves(a),
        (Some(Normal::Record(a)), Some(Normal::Record(b))) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for k in keys {
                compare(a.get(k), b.get(k), &format!("{path}.{k}"), out);
            }
        }
        (Some(Normal::List(a)), Some(Normal::List(b)))
        | (Some(Normal::OrderedSet(a)), Some(Normal::OrderedSet(b))) => {
            for i in 0..a.len().max(b.len()) {
                compare(a.get(i), b.get(i), &format!("{path}[{i}]"), out);
            }
        }
        (Some(Normal::Set(a)), Some(Normal::Set(b))) => {
            for x in a.union(b) {
                let p = format!("{path}{}", segment(x));
                compare(a.get(x), b.get(x), &p, out);
            }
        }
        (Some(Normal::Map(a)), Some(Normal::Map(b))) => {
            let keys: BTreeSet<&Normal> = a.keys().chain(b.keys()).collect();
            for k in keys {
                compare(a.get(k), b.get(k), &format!("{path}{}", segment(k)), out);
            }
        }
        (Some(Normal::OrderedMap(a)), Some(Normal::OrderedMap(b))) => {
            let order =
                |m: &[(Normal, Normal)]| m.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
            if order(a) != order(b) {
                out.differ
                    .push((Difference::Differs, format!("{path}.order"), String::new()));
            }
            let a: BTreeMap<Normal, Normal> = a.iter().cloned().collect();
            let b: BTreeMap<Normal, Normal> = b.iter().cloned().collect();
            compare(Some(&Normal::Map(a)), Some(&Normal::Map(b)), path, out);
        }
        (Some(Normal::Tag(t, a)), Some(Normal::Tag(u, b))) if t == u => {
            compare(Some(a), Some(b), &format!("{path}#{t}"), out)
        }
        (Some(a), None) => out
            .differ
            .push((Difference::Lacks, path.to_string(), short(a))),
        (None, Some(b)) => out
            .differ
            .push((Difference::Extra, path.to_string(), short(b))),
        (Some(a), Some(b)) => out.differ.push((
            Difference::Differs,
            path.to_string(),
            format!("reference {} implementation {}", short(a), short(b)),
        )),
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The normal form of `tree` read with no leaf of its own.
    fn plain(tree: &Value) -> Normal {
        normal(tree, &|_, _| None, &mut Vec::new())
    }

    #[test]
    fn sets_in_another_order_agree() {
        let a = plain(&json!({ "set": ["x", "y"] }));
        let b = plain(&json!({ "set": ["y", "x"] }));
        assert!(Diff::between(&a, &b).differ.is_empty());
    }

    #[test]
    fn ordered_maps_in_another_order_differ() {
        let a = plain(&json!({ "omap": [["x", 1], ["y", 2]] }));
        let b = plain(&json!({ "omap": [["y", 2], ["x", 1]] }));
        let differ = Diff::between(&a, &b).differ;
        assert_eq!(differ.len(), 1, "{differ:?}");
        assert_eq!(differ[0].1, ".order");
    }

    #[test]
    fn a_duplicate_in_a_set_is_counted_at_its_path() {
        let mut duplicates = Vec::new();
        normal(
            &json!({ "items": { "set": ["x", "x"] } }),
            &|_, _| None,
            &mut duplicates,
        );
        assert_eq!(duplicates, [".items"]);
    }

    #[test]
    fn a_one_key_object_is_a_leaf_only_when_its_reader_says() {
        let tree = json!({ "inner": { "count": "7" } });
        let as_record = plain(&tree);
        assert_eq!(
            as_record,
            Normal::Record(BTreeMap::from([(
                "inner".to_owned(),
                Normal::Record(BTreeMap::from([(
                    "count".to_owned(),
                    Normal::Scalar("7".into())
                )]))
            )]))
        );
        let count = |key: &str, value: &Value| {
            (key == "count").then(|| Normal::Integer(value.as_str().unwrap_or_default().into()))
        };
        let as_leaf = normal(&tree, &count, &mut Vec::new());
        assert_eq!(
            as_leaf,
            Normal::Record(BTreeMap::from([(
                "inner".to_owned(),
                Normal::Integer("7".into())
            )]))
        );
    }
}
