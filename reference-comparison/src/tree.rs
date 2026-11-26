//! The value tree an implementation and a reference each write for one value,
//! its normal form and the leaves where two normal forms differ.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::{Map, Value, json};

/// The tree of a value's CBOR encoding, compared by the value it encodes.
pub fn cbor(bytes: &[u8]) -> Value {
    json!({ "cbor": hex::encode(bytes) })
}

/// The tree of a set, whose order and duplicates the value ignores.
pub fn set(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "set": items.into_iter().collect::<Vec<_>>() })
}

/// The tree of an ordered set, whose duplicates the value ignores.
pub fn oset(items: impl IntoIterator<Item = Value>) -> Value {
    json!({ "oset": items.into_iter().collect::<Vec<_>>() })
}

/// The tree of a map, whose order the value ignores and whose later duplicate
/// key wins.
pub fn map(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    json!({ "map": pairs.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>() })
}

/// The tree of an ordered map, whose first duplicate key wins.
pub fn omap(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
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

/// The decimal digits of a big endian unsigned magnitude.
pub fn decimal(be: &[u8]) -> String {
    let mut n: Vec<u8> = be.iter().copied().skip_while(|b| *b == 0).collect();
    if n.is_empty() {
        return "0".into();
    }
    let mut out = Vec::new();
    while !n.is_empty() {
        let mut rem: u32 = 0;
        let mut q = Vec::with_capacity(n.len());
        for &b in &n {
            let cur = rem * 256 + b as u32;
            let digit = cur / 10;
            rem = cur % 10;
            if !(q.is_empty() && digit == 0) {
                q.push(digit as u8);
            }
        }
        out.push(b'0' + rem as u8);
        n = q;
    }
    out.reverse();
    String::from_utf8(out).expect("ascii digits")
}

/// The decimal digits of the negative integer `-1 - n` for a big endian
/// magnitude `n`.
pub fn negative_decimal(be: &[u8]) -> String {
    let mut v = be.to_vec();
    let mut i = v.len();
    loop {
        if i == 0 {
            v.insert(0, 1);
            break;
        }
        i -= 1;
        if v[i] == 255 {
            v[i] = 0;
        } else {
            v[i] += 1;
            break;
        }
    }
    format!("-{}", decimal(&v))
}

/// A value in the form two trees share exactly when they hold the same value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Normal {
    Null,
    Undefined,
    Bool(bool),
    Str(String),
    Int(String),
    Ratio(i128, i128),
    Bytes(Vec<u8>),
    Text(String),
    Float(Vec<u8>),
    Simple(u8),
    Tag(u64, Box<Normal>),
    List(Vec<Normal>),
    Map(BTreeMap<Normal, Normal>),
    Set(BTreeSet<Normal>),
    OSet(Vec<Normal>),
    OMap(Vec<(Normal, Normal)>),
    Record(BTreeMap<String, Normal>),
    Unreadable(String),
}

/// A CBOR reader that ignores the encoding choices that leave the value
/// unchanged.
struct Cbor<'a> {
    b: &'a [u8],
    p: usize,
}

impl Cbor<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let x = *self.b.get(self.p).ok_or("end of input")?;
        self.p += 1;
        Ok(x)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.p.checked_add(n).filter(|e| *e <= self.b.len());
        let end = end.ok_or("end of input")?;
        let out = &self.b[self.p..end];
        self.p = end;
        Ok(out)
    }

    fn arg(&mut self, info: u8) -> Result<u64, String> {
        let n = match info {
            0..=23 => return Ok(info as u64),
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            _ => return Err(format!("additional info {info}")),
        };
        Ok(self
            .take(n)?
            .iter()
            .fold(0u64, |acc, b| (acc << 8) | *b as u64))
    }

    fn at_break(&self) -> bool {
        self.b.get(self.p) == Some(&0xff)
    }

    fn count(&mut self, info: u8) -> Result<Option<u64>, String> {
        if info == 31 {
            Ok(None)
        } else {
            self.arg(info).map(Some)
        }
    }

    fn items<T>(
        &mut self,
        len: Option<u64>,
        mut one: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        let mut out = Vec::new();
        match len {
            Some(n) => {
                for _ in 0..n {
                    out.push(one(self)?);
                }
            }
            None => {
                while !self.at_break() {
                    out.push(one(self)?);
                }
                self.p += 1;
            }
        }
        Ok(out)
    }

    fn chunks(&mut self, major: u8, info: u8) -> Result<Vec<u8>, String> {
        if info != 31 {
            let n = self.arg(info)? as usize;
            return Ok(self.take(n)?.to_vec());
        }
        let mut out = Vec::new();
        while !self.at_break() {
            let ib = self.byte()?;
            if ib >> 5 != major || ib & 31 == 31 {
                return Err("bad string chunk".into());
            }
            let n = self.arg(ib & 31)? as usize;
            out.extend_from_slice(self.take(n)?);
        }
        self.p += 1;
        Ok(out)
    }

    fn item(&mut self) -> Result<Normal, String> {
        let ib = self.byte()?;
        let (major, info) = (ib >> 5, ib & 31);
        Ok(match major {
            0 => Normal::Int(self.arg(info)?.to_string()),
            1 => Normal::Int((-1 - self.arg(info)? as i128).to_string()),
            2 => Normal::Bytes(self.chunks(2, info)?),
            3 => Normal::Text(String::from_utf8_lossy(&self.chunks(3, info)?).into_owned()),
            4 => {
                let len = self.count(info)?;
                Normal::List(self.items(len, |r| r.item())?)
            }
            5 => {
                let len = self.count(info)?;
                Normal::Map(
                    self.items(len, |r| Ok((r.item()?, r.item()?)))?
                        .into_iter()
                        .collect(),
                )
            }
            6 => {
                let tag = self.arg(info)?;
                tagged_value(tag, self.item()?)
            }
            _ => match info {
                20 => Normal::Bool(false),
                21 => Normal::Bool(true),
                22 => Normal::Null,
                23 => Normal::Undefined,
                25 => Normal::Float(self.take(2)?.to_vec()),
                26 => Normal::Float(self.take(4)?.to_vec()),
                27 => Normal::Float(self.take(8)?.to_vec()),
                _ => Normal::Simple(self.arg(info)? as u8),
            },
        })
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The value of a tagged item, with set tags dropped, bignums as integers and
/// rationals reduced.
fn tagged_value(tag: u64, inner: Normal) -> Normal {
    match (tag, inner) {
        (258, inner) => inner,
        (2, Normal::Bytes(b)) => Normal::Int(decimal(&b)),
        (3, Normal::Bytes(b)) => Normal::Int(negative_decimal(&b)),
        (30, Normal::List(xs)) if xs.len() == 2 => {
            let parsed: Vec<Option<i128>> = xs
                .iter()
                .map(|x| match x {
                    Normal::Int(s) => s.parse().ok(),
                    _ => None,
                })
                .collect();
            match parsed[..] {
                [Some(n), Some(d)] if d != 0 => {
                    let g = gcd(n.unsigned_abs(), d.unsigned_abs()).max(1) as i128;
                    let sign = if d < 0 { -1 } else { 1 };
                    Normal::Ratio(sign * n / g, sign * d / g)
                }
                _ => Normal::Tag(30, Box::new(Normal::List(xs))),
            }
        }
        (tag, inner) => Normal::Tag(tag, Box::new(inner)),
    }
}

fn cbor_hex(hexstr: &str) -> Normal {
    let Ok(bytes) = hex::decode(hexstr) else {
        return Normal::Unreadable(hexstr.to_string());
    };
    let mut r = Cbor { b: &bytes, p: 0 };
    match r.item() {
        Ok(v) if r.p == bytes.len() => v,
        _ => Normal::Unreadable(hexstr.to_string()),
    }
}

/// The normal form of `tree`, pushing to `duplicates` the path of each
/// duplicate a set or map removes.
pub fn normal(tree: &Value, duplicates: &mut Vec<String>) -> Normal {
    normal_at(tree, "", duplicates)
}

fn normal_at(v: &Value, path: &str, dups: &mut Vec<String>) -> Normal {
    match v {
        Value::Object(m) if m.len() == 1 && m.contains_key("cbor") => {
            cbor_hex(m["cbor"].as_str().unwrap_or_default())
        }
        Value::Object(m) if m.len() == 1 && m.contains_key("set") => {
            let items = elements(&m["set"], path, dups);
            let n = items.len();
            let out: BTreeSet<Normal> = items.into_iter().collect();
            if out.len() != n {
                dups.push(path.to_string());
            }
            Normal::Set(out)
        }
        Value::Object(m) if m.len() == 1 && m.contains_key("oset") => {
            let mut out: Vec<Normal> = Vec::new();
            for x in elements(&m["oset"], path, dups) {
                if out.contains(&x) {
                    dups.push(path.to_string());
                } else {
                    out.push(x);
                }
            }
            Normal::OSet(out)
        }
        Value::Object(m) if m.len() == 1 && (m.contains_key("map") || m.contains_key("omap")) => {
            let ordered = m.contains_key("omap");
            let pairs = m.values().next().and_then(Value::as_array);
            let mut out: Vec<(Normal, Normal)> = Vec::new();
            for pair in pairs.into_iter().flatten() {
                let k = normal_at(&pair[0], path, dups);
                let v = normal_at(&pair[1], path, dups);
                match out.iter().position(|(x, _)| *x == k) {
                    Some(at) => {
                        dups.push(path.to_string());
                        if !ordered {
                            out[at].1 = v;
                        }
                    }
                    None => out.push((k, v)),
                }
            }
            if ordered {
                Normal::OMap(out)
            } else {
                Normal::Map(out.into_iter().collect())
            }
        }
        Value::Object(m) => Normal::Record(
            m.iter()
                .map(|(k, x)| (k.clone(), normal_at(x, &format!("{path}.{k}"), dups)))
                .collect(),
        ),
        Value::Array(xs) => Normal::List(
            xs.iter()
                .map(|x| normal_at(x, &format!("{path}[]"), dups))
                .collect(),
        ),
        Value::String(s) => Normal::Str(s.clone()),
        Value::Bool(b) => Normal::Bool(*b),
        Value::Null => Normal::Null,
        Value::Number(n) => Normal::Str(n.to_string()),
    }
}

fn elements(v: &Value, path: &str, dups: &mut Vec<String>) -> Vec<Normal> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|x| normal_at(x, &format!("{path}[]"), dups))
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
        Normal::List(xs) | Normal::OSet(xs) => xs.iter().map(leaves).sum(),
        Normal::Set(xs) => xs.iter().map(leaves).sum(),
        Normal::Map(m) => m.iter().map(|(k, x)| leaves(k) + leaves(x)).sum(),
        Normal::OMap(m) => m.iter().map(|(k, x)| leaves(k) + leaves(x)).sum(),
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
        Normal::Int(s) if s.len() < 4 => format!("<{s}>"),
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
        | (Some(Normal::OSet(a)), Some(Normal::OSet(b))) => {
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
        (Some(Normal::OMap(a)), Some(Normal::OMap(b))) => {
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

    #[test]
    fn equal_values_in_other_encodings_agree() {
        let pairs = [
            ("d9010282 0102", "820102"),
            ("a2 0102 0304", "a2 0304 0102"),
            ("d81e82 0204", "d81e82 0102"),
            ("c241 05", "05"),
            ("c341 05", "25"),
            ("5f 4101 4102 ff", "42 0102"),
        ];
        for (a, b) in pairs {
            let (a, b) = (a.replace(' ', ""), b.replace(' ', ""));
            let diff = Diff::between(&cbor_hex(&a), &cbor_hex(&b));
            assert!(diff.differ.is_empty(), "{a} and {b}: {:?}", diff.differ);
        }
        let set_a = json!({ "set": [{ "cbor": "01" }, { "cbor": "02" }] });
        let set_b = json!({ "set": [{ "cbor": "02" }, { "cbor": "01" }] });
        let (a, b) = (normal(&set_a, &mut vec![]), normal(&set_b, &mut vec![]));
        assert!(Diff::between(&a, &b).differ.is_empty());
    }

    #[test]
    fn different_values_differ() {
        let pairs = [
            ("820102", "820201", Difference::Differs),
            ("a10102", "a10103", Difference::Differs),
            ("a2 0102 0304", "a1 0102", Difference::Lacks),
            ("82 01 02", "83 01 02 03", Difference::Extra),
            ("d81e82 0102", "d81e82 0103", Difference::Differs),
            ("c241 05", "06", Difference::Differs),
            ("41 00", "40", Difference::Differs),
        ];
        for (a, b, class) in pairs {
            let (a, b) = (a.replace(' ', ""), b.replace(' ', ""));
            let diff = Diff::between(&cbor_hex(&a), &cbor_hex(&b));
            assert!(
                diff.differ.iter().any(|(c, _, _)| *c == class),
                "{a} and {b}: {:?}",
                diff.differ
            );
        }
        let ordered_a = json!({ "omap": [[{ "cbor": "01" }, { "cbor": "02" }], [{ "cbor": "03" }, { "cbor": "04" }]] });
        let ordered_b = json!({ "omap": [[{ "cbor": "03" }, { "cbor": "04" }], [{ "cbor": "01" }, { "cbor": "02" }]] });
        let (a, b) = (
            normal(&ordered_a, &mut vec![]),
            normal(&ordered_b, &mut vec![]),
        );
        assert_eq!(Diff::between(&a, &b).differ.len(), 1);
        let mut dups = Vec::new();
        normal(
            &json!({ "set": [{ "cbor": "01" }, { "cbor": "01" }] }),
            &mut dups,
        );
        assert_eq!(dups.len(), 1);
    }
}
