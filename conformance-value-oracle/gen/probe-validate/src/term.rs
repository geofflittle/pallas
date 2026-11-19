//! A CBOR data item as a tree, read from any encoding and written with definite lengths.

use pallas_codec::minicbor::data::{Int, Tag, Type};
use pallas_codec::minicbor::{Decoder, Encoder};

/// One CBOR data item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Int(i128),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Term>),
    Map(Vec<(Term, Term)>),
    Tag(u64, Box<Term>),
    Bool(bool),
    Null,
    Undefined,
}

pub fn read(bytes: &[u8]) -> Result<Term, String> {
    let mut d = Decoder::new(bytes);
    let t = item(&mut d)?;
    match d.position() == bytes.len() {
        true => Ok(t),
        false => Err(format!("{} trailing bytes", bytes.len() - d.position())),
    }
}

fn item(d: &mut Decoder<'_>) -> Result<Term, String> {
    let e = |x: pallas_codec::minicbor::decode::Error| x.to_string();
    Ok(match d.datatype().map_err(e)? {
        Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::I8 | Type::I16 | Type::I32
        | Type::I64 | Type::Int => Term::Int(i128::from(d.int().map_err(e)?)),
        Type::Bytes | Type::BytesIndef => {
            let mut out = Vec::new();
            for chunk in d.bytes_iter().map_err(e)? {
                out.extend_from_slice(chunk.map_err(e)?);
            }
            Term::Bytes(out)
        }
        Type::String | Type::StringIndef => {
            let mut out = String::new();
            for chunk in d.str_iter().map_err(e)? {
                out.push_str(chunk.map_err(e)?);
            }
            Term::Text(out)
        }
        Type::Array | Type::ArrayIndef => {
            let mut out = Vec::new();
            match d.array().map_err(e)? {
                Some(n) => {
                    for _ in 0..n {
                        out.push(item(d)?);
                    }
                }
                None => {
                    while d.datatype().map_err(e)? != Type::Break {
                        out.push(item(d)?);
                    }
                    d.skip().map_err(e)?;
                }
            }
            Term::Array(out)
        }
        Type::Map | Type::MapIndef => {
            let mut out = Vec::new();
            match d.map().map_err(e)? {
                Some(n) => {
                    for _ in 0..n {
                        out.push((item(d)?, item(d)?));
                    }
                }
                None => {
                    while d.datatype().map_err(e)? != Type::Break {
                        out.push((item(d)?, item(d)?));
                    }
                    d.skip().map_err(e)?;
                }
            }
            Term::Map(out)
        }
        Type::Tag => {
            let tag = d.tag().map_err(e)?;
            Term::Tag(tag.as_u64(), Box::new(item(d)?))
        }
        Type::Bool => Term::Bool(d.bool().map_err(e)?),
        Type::Null => {
            d.null().map_err(e)?;
            Term::Null
        }
        Type::Undefined => {
            d.undefined().map_err(e)?;
            Term::Undefined
        }
        other => return Err(format!("unsupported CBOR type {other}")),
    })
}

pub fn write(t: &Term) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    put(&mut e, t);
    e.into_writer()
}

fn put(e: &mut Encoder<Vec<u8>>, t: &Term) {
    let r = match t {
        Term::Int(x) => e.int(Int::try_from(*x).expect("an integer CBOR holds")).map(|_| ()),
        Term::Bytes(x) => e.bytes(x).map(|_| ()),
        Term::Text(x) => e.str(x).map(|_| ()),
        Term::Array(xs) => {
            e.array(xs.len() as u64).expect("write to a vec");
            xs.iter().for_each(|x| put(e, x));
            Ok(())
        }
        Term::Map(xs) => {
            e.map(xs.len() as u64).expect("write to a vec");
            for (k, v) in xs {
                put(e, k);
                put(e, v);
            }
            Ok(())
        }
        Term::Tag(tag, x) => {
            e.tag(Tag::new(*tag)).expect("write to a vec");
            put(e, x);
            Ok(())
        }
        Term::Bool(x) => e.bool(*x).map(|_| ()),
        Term::Null => e.null().map(|_| ()),
        Term::Undefined => e.undefined().map(|_| ()),
    };
    r.expect("write to a vec");
}

impl Term {
    /// The value under `key` in a map.
    pub fn get_mut(&mut self, key: i128) -> Option<&mut Term> {
        match self {
            Term::Map(xs) => xs.iter_mut().find(|(k, _)| *k == Term::Int(key)).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Sets `key` in a map, replacing any value it had.
    pub fn set(&mut self, key: i128, value: Term) {
        if let Some(v) = self.get_mut(key) {
            *v = value;
        } else if let Term::Map(xs) = self {
            xs.push((Term::Int(key), value));
        }
    }

    /// The elements of an array, or of an array under a tag.
    pub fn items_mut(&mut self) -> &mut Vec<Term> {
        match self {
            Term::Array(xs) => xs,
            Term::Tag(_, x) => x.items_mut(),
            other => panic!("not an array: {other:?}"),
        }
    }
}

/// A set as the Dijkstra encoding writes it.
pub fn set(items: Vec<Term>) -> Term {
    Term::Tag(258, Box::new(Term::Array(items)))
}

/// A rational as the Dijkstra encoding writes it.
pub fn rational(numerator: i128, denominator: i128) -> Term {
    Term::Tag(30, Box::new(Term::Array(vec![Term::Int(numerator), Term::Int(denominator)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definite_input_writes_back_unchanged() {
        let bytes = hex::decode("83a200d9010281825820000000000000000000000000000000000000000000000000000000000000000000021a00030d40a0f6").unwrap();
        assert_eq!(write(&read(&bytes).unwrap()), bytes);
    }

    #[test]
    fn indefinite_input_writes_definite() {
        assert_eq!(write(&read(&[0x9f, 0x01, 0xff]).unwrap()), vec![0x81, 0x01]);
    }

    #[test]
    fn trailing_bytes_are_refused() {
        assert!(read(&[0x01, 0x00]).is_err());
    }
}
