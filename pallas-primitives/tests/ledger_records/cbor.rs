//! The `cbor` leaf of the value tree, read as the value its bytes encode.

use reference_comparison::tree::Normal;
use serde_json::{Value, json};

/// The key of the leaf that holds a value's CBOR encoding.
const KEY: &str = "cbor";

/// The tree of a value's CBOR encoding, compared by the value it encodes.
pub fn leaf(bytes: &[u8]) -> Value {
    json!({ KEY: hex::encode(bytes) })
}

/// The value of a one key object whose key is `cbor`, or none for any other.
pub fn read_leaf(key: &str, value: &Value) -> Option<Normal> {
    (key == KEY).then(|| read_hex(value.as_str().unwrap_or_default()))
}

/// The decimal digits of a big endian unsigned magnitude.
pub fn decimal(magnitude: &[u8]) -> String {
    let mut n: Vec<u8> = magnitude.iter().copied().skip_while(|b| *b == 0).collect();
    if n.is_empty() {
        return "0".into();
    }
    let mut out = Vec::new();
    while !n.is_empty() {
        let mut remainder: u32 = 0;
        let mut quotient = Vec::with_capacity(n.len());
        for &b in &n {
            let current = remainder * 256 + b as u32;
            let digit = current / 10;
            remainder = current % 10;
            if !(quotient.is_empty() && digit == 0) {
                quotient.push(digit as u8);
            }
        }
        out.push(b'0' + remainder as u8);
        n = quotient;
    }
    out.reverse();
    String::from_utf8(out).expect("ascii digits")
}

/// The decimal digits of the negative integer `-1 - n` for a big endian
/// magnitude `n`.
pub fn negative_decimal(magnitude: &[u8]) -> String {
    let mut v = magnitude.to_vec();
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

/// A CBOR reader that ignores the encoding choices that leave the value
/// unchanged.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let x = *self.bytes.get(self.position).ok_or("end of input")?;
        self.position += 1;
        Ok(x)
    }

    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self
            .position
            .checked_add(n)
            .filter(|e| *e <= self.bytes.len());
        let end = end.ok_or("end of input")?;
        let out = &self.bytes[self.position..end];
        self.position = end;
        Ok(out)
    }

    fn argument(&mut self, info: u8) -> Result<u64, String> {
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
        self.bytes.get(self.position) == Some(&0xff)
    }

    fn count(&mut self, info: u8) -> Result<Option<u64>, String> {
        if info == 31 {
            Ok(None)
        } else {
            self.argument(info).map(Some)
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
                self.position += 1;
            }
        }
        Ok(out)
    }

    fn chunks(&mut self, major: u8, info: u8) -> Result<Vec<u8>, String> {
        if info != 31 {
            let n = self.argument(info)? as usize;
            return Ok(self.take(n)?.to_vec());
        }
        let mut out = Vec::new();
        while !self.at_break() {
            let initial = self.byte()?;
            if initial >> 5 != major || initial & 31 == 31 {
                return Err("bad string chunk".into());
            }
            let n = self.argument(initial & 31)? as usize;
            out.extend_from_slice(self.take(n)?);
        }
        self.position += 1;
        Ok(out)
    }

    fn item(&mut self) -> Result<Normal, String> {
        let initial = self.byte()?;
        let (major, info) = (initial >> 5, initial & 31);
        Ok(match major {
            0 => Normal::Integer(self.argument(info)?.to_string()),
            1 => Normal::Integer((-1 - self.argument(info)? as i128).to_string()),
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
                let tag = self.argument(info)?;
                tagged_value(tag, self.item()?)
            }
            _ => match info {
                20 => Normal::Boolean(false),
                21 => Normal::Boolean(true),
                22 => Normal::Null,
                23 => Normal::Other("undefined".into()),
                25 => Normal::Other(format!("float {}", hex::encode(self.take(2)?))),
                26 => Normal::Other(format!("float {}", hex::encode(self.take(4)?))),
                27 => Normal::Other(format!("float {}", hex::encode(self.take(8)?))),
                _ => Normal::Other(format!("simple {}", self.argument(info)? as u8)),
            },
        })
    }
}

fn greatest_common_divisor(a: u128, b: u128) -> u128 {
    if b == 0 {
        a
    } else {
        greatest_common_divisor(b, a % b)
    }
}

/// The value of a tagged item, with set tags dropped, bignums as integers and
/// rationals reduced.
fn tagged_value(tag: u64, inner: Normal) -> Normal {
    match (tag, inner) {
        (258, inner) => inner,
        (2, Normal::Bytes(b)) => Normal::Integer(decimal(&b)),
        (3, Normal::Bytes(b)) => Normal::Integer(negative_decimal(&b)),
        (30, Normal::List(xs)) if xs.len() == 2 => {
            let parsed: Vec<Option<i128>> = xs
                .iter()
                .map(|x| match x {
                    Normal::Integer(s) => s.parse().ok(),
                    _ => None,
                })
                .collect();
            match parsed[..] {
                [Some(n), Some(d)] if d != 0 => {
                    let g =
                        greatest_common_divisor(n.unsigned_abs(), d.unsigned_abs()).max(1) as i128;
                    let sign = if d < 0 { -1 } else { 1 };
                    Normal::Ratio(sign * n / g, sign * d / g)
                }
                _ => Normal::Tag(30, Box::new(Normal::List(xs))),
            }
        }
        (tag, inner) => Normal::Tag(tag, Box::new(inner)),
    }
}

/// The value that the hex digits `digits` encode, or the digits as unreadable
/// when they are not one whole CBOR item.
fn read_hex(digits: &str) -> Normal {
    let Ok(bytes) = hex::decode(digits) else {
        return Normal::Unreadable(digits.to_string());
    };
    let mut reader = Reader {
        bytes: &bytes,
        position: 0,
    };
    match reader.item() {
        Ok(v) if reader.position == bytes.len() => v,
        _ => Normal::Unreadable(digits.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use reference_comparison::tree::{Diff, Difference, normal};

    use super::*;

    /// The normal form of `tree` with its `cbor` leaves read.
    fn normal_of(tree: &Value, duplicates: &mut Vec<String>) -> Normal {
        normal(tree, &read_leaf, duplicates)
    }

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
            let diff = Diff::between(&read_hex(&a), &read_hex(&b));
            assert!(diff.differ.is_empty(), "{a} and {b}: {:?}", diff.differ);
        }
        let set_a = json!({ "set": [{ "cbor": "01" }, { "cbor": "02" }] });
        let set_b = json!({ "set": [{ "cbor": "02" }, { "cbor": "01" }] });
        let (a, b) = (
            normal_of(&set_a, &mut vec![]),
            normal_of(&set_b, &mut vec![]),
        );
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
            let diff = Diff::between(&read_hex(&a), &read_hex(&b));
            assert!(
                diff.differ.iter().any(|(c, _, _)| *c == class),
                "{a} and {b}: {:?}",
                diff.differ
            );
        }
        let ordered_a = json!({ "omap": [[{ "cbor": "01" }, { "cbor": "02" }], [{ "cbor": "03" }, { "cbor": "04" }]] });
        let ordered_b = json!({ "omap": [[{ "cbor": "03" }, { "cbor": "04" }], [{ "cbor": "01" }, { "cbor": "02" }]] });
        let (a, b) = (
            normal_of(&ordered_a, &mut vec![]),
            normal_of(&ordered_b, &mut vec![]),
        );
        assert_eq!(Diff::between(&a, &b).differ.len(), 1);
        let mut duplicates = Vec::new();
        normal_of(
            &json!({ "set": [{ "cbor": "01" }, { "cbor": "01" }] }),
            &mut duplicates,
        );
        assert_eq!(duplicates.len(), 1);
    }
}
