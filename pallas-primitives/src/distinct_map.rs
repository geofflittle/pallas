//! Decoding of maps in which a repeated key is refused.

use std::collections::BTreeMap;

use pallas_codec::minicbor::{Decode, Decoder, data::Type, decode::Error};
use pallas_codec::utils::{PositiveCoin, decode_bytes_sized};

use crate::{AssetName, PolicyId};

/// Decodes a map with no repeated key, reading keys and values with the given decoders.
pub(crate) fn decode_by_key<'b, C, K: Ord, V>(
    d: &mut Decoder<'b>,
    ctx: &mut C,
    mut key: impl FnMut(&mut Decoder<'b>, &mut C) -> Result<K, Error>,
    mut value: impl FnMut(&mut Decoder<'b>, &mut C) -> Result<V, Error>,
) -> Result<BTreeMap<K, V>, Error> {
    let len = d.map()?;
    let mut map = BTreeMap::new();
    let mut read = 0;
    loop {
        match len {
            Some(len) if read == len => break,
            None if d.datatype()? == Type::Break => {
                d.skip()?;
                break;
            }
            _ => {}
        }
        let key_at = d.position();
        let key = key(d, ctx)?;
        let entry = value(d, ctx)?;
        if map.insert(key, entry).is_some() {
            return Err(Error::message("repeated key in a map").at(key_at));
        }
        read += 1;
    }
    Ok(map)
}

/// Decodes a map with no repeated key.
pub(crate) fn decode<'b, C, K, V>(d: &mut Decoder<'b>, ctx: &mut C) -> Result<BTreeMap<K, V>, Error>
where
    K: Decode<'b, C> + Ord,
    V: Decode<'b, C>,
{
    decode_by_key(
        d,
        ctx,
        |d, ctx| d.decode_with(ctx),
        |d, ctx| d.decode_with(ctx),
    )
}

/// Decodes an optional field holding a map with no repeated key.
pub(crate) fn decode_option<'b, C, K, V>(
    d: &mut Decoder<'b>,
    ctx: &mut C,
) -> Result<Option<BTreeMap<K, V>>, Error>
where
    K: Decode<'b, C> + Ord,
    V: Decode<'b, C>,
{
    decode(d, ctx).map(Some)
}

/// Refuses a map with no entry, reporting the position the map was read from.
pub(crate) fn refuse_empty<K, V>(map: &BTreeMap<K, V>, at: usize) -> Result<(), Error> {
    if map.is_empty() {
        return Err(Error::message("empty map where an entry is required").at(at));
    }
    Ok(())
}

/// Decodes a multiasset with no repeated policy or asset, at least one asset per policy and names of at most 32 bytes.
pub(crate) fn decode_multiasset<'b, C, A>(
    d: &mut Decoder<'b>,
    ctx: &mut C,
    mut amount: impl FnMut(&mut Decoder<'b>, &mut C) -> Result<A, Error>,
) -> Result<BTreeMap<PolicyId, BTreeMap<AssetName, A>>, Error> {
    decode_by_key(
        d,
        ctx,
        |d, ctx| d.decode_with(ctx),
        |d, ctx| {
            let at = d.position();
            let assets = decode_by_key(d, ctx, decode_bytes_sized::<0, 32, _>, &mut amount)?;
            refuse_empty(&assets, at)?;
            Ok(assets)
        },
    )
}

/// Decodes an asset amount the ledger reads only above zero.
pub(crate) fn decode_positive_amount<C>(d: &mut Decoder<'_>, ctx: &mut C) -> Result<u64, Error> {
    PositiveCoin::decode(d, ctx).map(u64::from)
}
