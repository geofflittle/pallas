//! Era-aware Cardano ledger types with their CBOR codecs.
//!
//! This is the data layer that the rest of the Pallas ledger crates sit on:
//! [`pallas-traverse`] gives you a multi-era read API over these types,
//! [`pallas-validate`] applies ledger rules to them, and [`pallas-txbuilder`]
//! builds new ones.
//!
//! If you need raw, era-specific access to a `Tx`, `Block`, or `PlutusData`,
//! you want this crate. If you'd rather work over many eras through one
//! interface, reach for [`pallas-traverse`].
//!
//! [`pallas-traverse`]: https://crates.io/crates/pallas-traverse
//! [`pallas-validate`]: https://crates.io/crates/pallas-validate
//! [`pallas-txbuilder`]: https://crates.io/crates/pallas-txbuilder
//!
//! # Usage
//!
//! ```no_run
//! use pallas_codec::minicbor;
//! use pallas_primitives::conway;
//!
//! # let cbor_bytes: Vec<u8> = vec![];
//! let tx: conway::Tx = minicbor::decode(&cbor_bytes)?;
//!
//! for input in tx.transaction_body.inputs.iter() {
//!     println!("{:?}#{}", input.transaction_id, input.index);
//! }
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```
//!
//! # Overview
//!
//! - [`byron`], [`alonzo`], [`babbage`], [`conway`] — one module per era,
//!   each exposing the era's `Block`, `Tx`, `TransactionInput`,
//!   `TransactionOutput`, `Value`, `Certificate`, `Metadata`, witness sets,
//!   and so on. A fifth module, `dijkstra`, covers the Dijkstra era behind the `unstable` feature.
//! - `plutus_data` — re-exported [`PlutusData`], [`BigInt`], and helpers
//!   shared across eras.
//! - `framework` — common type aliases and codec primitives
//!   ([`AddrKeyhash`], [`Coin`], [`PolicyId`], [`RationalNumber`],
//!   [`StakeCredential`], [`TransactionInput`], [`ExUnits`],
//!   [`PlutusScript`], …).
//! - Re-exports from [`pallas-codec`] ([`Bytes`], [`KeepRaw`],
//!   [`KeyValuePairs`], [`NonEmptySet`], [`Set`], [`Nullable`], …) and
//!   [`pallas-crypto`] ([`struct@Hash`]).
//!
//! [`pallas-codec`]: https://crates.io/crates/pallas-codec
//! [`pallas-crypto`]: https://crates.io/crates/pallas-crypto
//!
//! # Feature flags
//!
//! - `relaxed` — relax some validation invariants applied during decoding;
//!   useful for round-tripping non-canonical historical artifacts.
//! - `unstable` — adds the `dijkstra` module. The era's schema is still moving, so its types are outside this crate's semver promise.
//!
//! # Usage as part of `pallas`
//!
//! When depending on the umbrella [`pallas`] crate, this crate is re-exported
//! as `pallas::ledger::primitives`.
//!
//! [`pallas`]: https://crates.io/crates/pallas

#[cfg(feature = "unstable")]
mod array_record;
#[cfg(feature = "unstable")]
mod distinct_map;
#[cfg(feature = "unstable")]
mod flat_enum;
mod framework;
#[cfg(feature = "unstable")]
mod map_record;
mod metadatum;
mod native_script;
mod plutus_data;

/// Ledger primitives for the Alonzo era (smart contracts).
pub mod alonzo;
/// Ledger primitives for the Babbage era (reference inputs / inline datums).
pub mod babbage;
/// Ledger primitives for the Byron era.
pub mod byron;
/// Ledger primitives for the Conway era (governance).
pub mod conway;
/// Ledger primitives for the Dijkstra era (inline transactions, Leios).
#[cfg(feature = "unstable")]
pub mod dijkstra;
pub use plutus_data::*;

pub use framework::*;

pub use pallas_codec::codec_by_datatype;

pub use pallas_codec::utils::{
    Bytes, Int, KeepRaw, KeyValuePairs, MaybeIndefArray, NonEmptySet, NonZeroInt, Nullable,
    PositiveCoin, Set,
};
pub use pallas_crypto::hash::Hash;

use pallas_codec::minicbor::{self, Decode, Encode, data::Tag};
#[cfg(not(feature = "unstable"))]
use pallas_codec::utils::decode_text_def_or_indef;
use pallas_codec::utils::end_array;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

// ----- Common type definitions

/// Hash of a Cardano address verification key (Blake2b-224).
pub type AddrKeyhash = Hash<28>;

/// Token name within a multi-asset bundle (raw bytes, up to 32 long).
pub type AssetName = Bytes;

/// Quantity in lovelace.
pub type Coin = u64;

/// Plutus cost model: ordered list of per-primitive cost coefficients.
pub type CostModel = Vec<i64>;

/// Hash of a Plutus datum (Blake2b-256).
pub type DatumHash = Hash<32>;

/// DNS name (A or SRV record) used in relay declarations.
pub type DnsName = String;

/// Epoch number on the Cardano chain.
pub type Epoch = u64;

/// Plutus script execution budget: memory and step units.
#[derive(Serialize, Deserialize, Encode, Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(not(feature = "unstable"), derive(Decode))]
pub struct ExUnits {
    /// Memory units consumed.
    #[n(0)]
    pub mem: u64,
    /// CPU step units consumed.
    #[n(1)]
    pub steps: u64,
}

#[cfg(feature = "unstable")]
array_record::decode_array_record!(ExUnits {
    mem: decode_nonnegative_int64,
    steps: decode_nonnegative_int64
});

/// Decodes an unsigned integer of at most `i64::MAX`.
#[cfg(feature = "unstable")]
fn decode_nonnegative_int64<Ctx>(
    d: &mut minicbor::Decoder<'_>,
    _ctx: &mut Ctx,
) -> Result<u64, minicbor::decode::Error> {
    u64::try_from(d.i64()?)
        .map_err(|_| minicbor::decode::Error::message("negative execution units"))
}

/// Per-unit prices used to convert [`ExUnits`] into fee lovelace.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct ExUnitPrices {
    /// Price per memory unit.
    #[n(0)]
    pub mem_price: PositiveInterval,

    /// Price per CPU step.
    #[n(1)]
    pub step_price: PositiveInterval,
}

/// Hash identifying a genesis configuration.
pub type Genesishash = Bytes;

/// Hash of a genesis delegate certificate.
pub type GenesisDelegateHash = Bytes;

/// IPv4 address bytes (4 bytes, big-endian).
pub type IPv4 = Bytes;

/// IPv6 address bytes (16 bytes, big-endian).
pub type IPv6 = Bytes;

/// Transaction metadata map, keyed by label.
pub type Metadata = BTreeMap<MetadatumLabel, Metadatum>;

/// Single metadata value of any supported CBOR shape.
///
/// The CBOR codec, `Clone` and `Ord` never recurse per nesting level, so
/// chain-deep metadata decodes, encodes, copies and compares on any stack,
/// and [`pallas_codec::tree`] can traverse it through `IndexedNode`.
/// `Debug`, `Drop` and Serde still recurse.
#[derive(Serialize, Deserialize, Debug)]
pub enum Metadatum {
    /// Integer (signed or unsigned, up to 64 bits).
    Int(Int),
    /// Raw byte string.
    Bytes(Bytes),
    /// UTF-8 text string.
    Text(String),
    /// Ordered list of metadata values.
    Array(Vec<Metadatum>),
    /// Map of metadata values keyed by metadata values.
    Map(KeyValuePairs<Metadatum, Metadatum>),
}

/// Top-level metadata label (CIP-10 / CIP-25 / etc.).
pub type MetadatumLabel = u64;

/// The network this artifact targets (encoded as a small CBOR enum).
#[derive(
    Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy,
)]
#[cbor(index_only)]
pub enum NetworkId {
    /// The Cardano testnet.
    #[n(0)]
    Testnet,
    /// The Cardano mainnet.
    #[n(1)]
    Mainnet,
}

impl From<NetworkId> for u8 {
    fn from(network_id: NetworkId) -> u8 {
        match network_id {
            NetworkId::Testnet => 0,
            NetworkId::Mainnet => 1,
        }
    }
}

impl TryFrom<u8> for NetworkId {
    type Error = ();

    fn try_from(i: u8) -> Result<Self, Self::Error> {
        match i {
            0 => Ok(Self::Testnet),
            1 => Ok(Self::Mainnet),
            _ => Err(()),
        }
    }
}

/// Praos nonce used as input to the leader-selection schedule.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct Nonce {
    /// Discriminator selecting how the nonce was produced.
    #[n(0)]
    pub variant: NonceVariant,

    /// Hash payload, present when `variant` is [`NonceVariant::Nonce`].
    #[n(1)]
    pub hash: Option<Hash<32>>,
}

/// Discriminator for [`Nonce`]: neutral (genesis) or hashed.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
#[cbor(index_only)]
pub enum NonceVariant {
    /// Initial neutral nonce, with no hash payload.
    #[n(0)]
    NeutralNonce,

    /// A hashed nonce; the payload lives in [`Nonce::hash`].
    #[n(1)]
    Nonce,
}

/// Raw bytes of a Plutus script of language version `VERSION` (1, 2, or 3).
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
#[cbor(transparent)]
pub struct PlutusScript<const VERSION: usize>(pub Bytes);

impl<const VERSION: usize> AsRef<[u8]> for PlutusScript<VERSION> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// Hash of a minting policy (Blake2b-224).
pub type PolicyId = Hash<28>;

/// Hash of a stake pool's cold key (Blake2b-224).
pub type PoolKeyhash = Hash<28>;

/// Stake pool metadata reference: URL plus the hash of the pointed-to JSON.
#[derive(Serialize, Deserialize, Encode, Debug, PartialEq, Eq, Clone)]
#[cfg_attr(not(feature = "unstable"), derive(Decode))]
pub struct PoolMetadata {
    /// URL serving the pool metadata JSON.
    #[n(0)]
    #[cbor(decode_with = "decode_text_def_or_indef")]
    pub url: String,

    /// Hash of the JSON document served at `url`.
    #[n(1)]
    pub hash: PoolMetadataHash,
}

#[cfg(feature = "unstable")]
array_record::decode_array_record!(PoolMetadata {
    url: pallas_codec::utils::decode_text_sized::<0, 128, _>,
    hash
});

/// Hash of stake pool metadata (Blake2b-256).
pub type PoolMetadataHash = Bytes;

/// TCP/UDP port number.
pub type Port = u32;

/// Rational number guaranteed to be strictly positive.
pub type PositiveInterval = RationalNumber;

/// Protocol version: `(major, minor)`.
pub type ProtocolVersion = (u64, u64);

/// Decoders for a [`ProtocolVersion`] field written as an array of either length form.
#[cfg(feature = "unstable")]
pub(crate) mod protocol_version {
    use super::ProtocolVersion;
    use pallas_codec::minicbor::{self, data::Type, decode::Error};
    use pallas_codec::utils::decode_array;

    /// The highest major protocol version the ledger reads.
    const MAX_MAJOR: u32 = 13;

    /// Decodes a [`ProtocolVersion`].
    pub fn decode<Ctx>(
        d: &mut minicbor::Decoder<'_>,
        ctx: &mut Ctx,
    ) -> Result<ProtocolVersion, Error> {
        decode_array(d, |a| {
            let major: u32 = a.decode_with(ctx)?;
            if major > MAX_MAJOR {
                return Err(Error::message(format!(
                    "protocol version major {major} above {MAX_MAJOR}"
                )));
            }
            let minor: u32 = a.decode_with(ctx)?;
            Ok((major.into(), minor.into()))
        })
    }

    /// Decodes an optional [`ProtocolVersion`], reading a null as `None`.
    pub fn decode_option<Ctx>(
        d: &mut minicbor::Decoder<'_>,
        ctx: &mut Ctx,
    ) -> Result<Option<ProtocolVersion>, Error> {
        if d.datatype()? == Type::Null {
            d.skip()?;
            return Ok(None);
        }
        decode(d, ctx).map(Some)
    }
}

/// On-chain rational number, encoded as a CBOR tag-30 array of `[num, den]`.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct RationalNumber {
    /// Numerator of the rational.
    pub numerator: u64,
    /// Denominator of the rational.
    pub denominator: u64,
}

impl<'b, C> minicbor::decode::Decode<'b, C> for RationalNumber {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        // TODO: Enforce tag == 30 & array of size 2
        d.tag()?;
        let len = d.array()?;
        let rational = RationalNumber {
            numerator: d.decode_with(ctx)?,
            denominator: d.decode_with(ctx)?,
        };
        if cfg!(feature = "unstable") {
            end_array(d, len, 2)?;
            if rational.denominator == 0 {
                return Err(minicbor::decode::Error::message(
                    "rational with a zero denominator",
                ));
            }
        }
        Ok(rational)
    }
}

impl<C> minicbor::encode::Encode<C> for RationalNumber {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(Tag::new(30))?;
        e.array(2)?;
        e.encode_with(self.numerator, ctx)?;
        e.encode_with(self.denominator, ctx)?;
        Ok(())
    }
}

/// Network endpoint declared by a stake pool's relay.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Relay {
    /// IP-based relay with optional port and either or both IPv4/IPv6.
    SingleHostAddr(Option<Port>, Option<IPv4>, Option<IPv6>),
    /// DNS A-record relay with optional port and a hostname.
    SingleHostName(Option<Port>, DnsName),
    /// DNS SRV-record relay (port and host both come from the SRV record).
    MultiHostName(DnsName),
}

impl<'b, C> minicbor::decode::Decode<'b, C> for Relay {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d.array()?;
        let variant = d.u16()?;

        let (relay, read) = match variant {
            0 => (
                Relay::SingleHostAddr(
                    decode_port(d, ctx)?,
                    decode_ip::<4, _>(d, ctx)?,
                    decode_ip::<16, _>(d, ctx)?,
                ),
                4,
            ),
            1 => (
                Relay::SingleHostName(decode_port(d, ctx)?, decode_dns_name(d, ctx)?),
                3,
            ),
            2 => (Relay::MultiHostName(decode_dns_name(d, ctx)?), 2),
            _ => {
                return Err(minicbor::decode::Error::message(
                    "invalid variant id for Relay",
                ));
            }
        };

        if cfg!(feature = "unstable") {
            end_array(d, len, read)?;
        }
        Ok(relay)
    }
}

/// Decode a relay's port of at most 65535, or the null that stands for none.
#[cfg(feature = "unstable")]
fn decode_port<C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<Option<Port>, minicbor::decode::Error> {
    d.decode_with::<_, Option<u16>>(ctx)
        .map(|port| port.map(Port::from))
}

/// Decode a relay's port, or the null that stands for none.
#[cfg(not(feature = "unstable"))]
fn decode_port<C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<Option<Port>, minicbor::decode::Error> {
    d.decode_with(ctx)
}

/// Decode a relay's IP address of `SIZE` bytes, or the null that stands for none.
#[cfg(feature = "unstable")]
fn decode_ip<const SIZE: usize, C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<Option<Bytes>, minicbor::decode::Error> {
    if d.datatype()? == minicbor::data::Type::Null {
        d.null()?;
        return Ok(None);
    }
    pallas_codec::utils::decode_bytes_sized::<SIZE, SIZE, _>(d, ctx).map(Some)
}

/// Decode a relay's IP address, or the null that stands for none.
#[cfg(not(feature = "unstable"))]
fn decode_ip<const SIZE: usize, C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<Option<Bytes>, minicbor::decode::Error> {
    d.decode_with(ctx)
}

/// Decode a relay's DNS name of at most 128 bytes.
#[cfg(feature = "unstable")]
fn decode_dns_name<C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<DnsName, minicbor::decode::Error> {
    pallas_codec::utils::decode_text_sized::<0, 128, _>(d, ctx)
}

/// Decode a relay's DNS name.
#[cfg(not(feature = "unstable"))]
fn decode_dns_name<C>(
    d: &mut minicbor::Decoder<'_>,
    ctx: &mut C,
) -> Result<DnsName, minicbor::decode::Error> {
    decode_text_def_or_indef(d, ctx)
}

impl<C> minicbor::encode::Encode<C> for Relay {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Relay::SingleHostAddr(a, b, c) => {
                e.array(4)?;
                e.encode_with(0, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;

                Ok(())
            }
            Relay::SingleHostName(a, b) => {
                e.array(3)?;
                e.encode_with(1, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;

                Ok(())
            }
            Relay::MultiHostName(a) => {
                e.array(2)?;
                e.encode_with(2, ctx)?;
                e.encode_with(a, ctx)?;

                Ok(())
            }
        }
    }
}

/// Reward-account bytes (network header byte plus stake-credential hash).
pub type RewardAccount = Bytes;

/// Hash of a script (Blake2b-224).
pub type ScriptHash = Hash<28>;

#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd, Eq, Ord, Clone, Hash, Encode)]
#[cfg_attr(not(feature = "unstable"), derive(Decode))]
// !! NOTE / IMPORTANT !!
// It is tempting to swap the order of the two constructors so that AddrKeyHash
// comes first. This indeed nicely maps the binary representation which
// associates 0 to AddrKeyHash and 1 to ScriptHash.
//
// However, for historical reasons, the ScriptHash variant comes first in the
// Haskell reference codebase. From this ordering is derived the `PartialOrd`
// and `Ord` instances; which impacts how Maps/Dictionnaries indexed by
// StakeCredential will be ordered. So, it is crucial to preserve this quirks to
// avoid hard to troubleshoot issues down the line.
#[cbor(flat)]
/// On-chain credential controlling a stake address: a script or a key hash.
pub enum StakeCredential {
    /// Stake credential backed by a script hash.
    #[n(1)]
    ScriptHash(#[n(0)] ScriptHash),
    /// Stake credential backed by a verification-key hash.
    #[n(0)]
    AddrKeyhash(#[n(0)] AddrKeyhash),
}

#[cfg(feature = "unstable")]
flat_enum::decode_flat_enum!(StakeCredential,
    0 => AddrKeyhash(hash),
    1 => ScriptHash(hash),
);

/// Index of a transaction within its containing block.
pub type TransactionIndex = u32;

/// Reference to a transaction output: `(tx_hash, output_index)`.
#[derive(
    Serialize, Deserialize, Encode, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, std::hash::Hash,
)]
#[cfg_attr(not(feature = "unstable"), derive(Decode))]
pub struct TransactionInput {
    /// Hash of the transaction that produced the output.
    #[n(0)]
    pub transaction_id: Hash<32>,

    /// Index of the output within that transaction.
    #[n(1)]
    pub index: u64,
}

#[cfg(feature = "unstable")]
array_record::decode_array_record!(TransactionInput {
    transaction_id,
    index: decode_word16
});

/// Decodes an unsigned integer of at most 16 bits into a wider field.
#[cfg(feature = "unstable")]
pub(crate) fn decode_word16<Ctx, T: From<u16>>(
    d: &mut minicbor::Decoder<'_>,
    _ctx: &mut Ctx,
) -> Result<T, minicbor::decode::Error> {
    d.u16().map(T::from)
}

/// Rational number constrained to the closed interval [0, 1].
pub type UnitInterval = RationalNumber;

/// VRF certificate: the output bytes followed by the proof bytes.
#[derive(Serialize, Deserialize, Encode, Debug, PartialEq, Eq, Clone)]
#[cfg_attr(not(feature = "unstable"), derive(Decode))]
pub struct VrfCert(
    /// VRF output bytes.
    #[n(0)]
    pub Bytes,
    /// VRF proof bytes.
    #[n(1)]
    pub Bytes,
);

#[cfg(feature = "unstable")]
array_record::decode_array_record!(
    VrfCert(
        output: pallas_codec::utils::decode_bytes_sized::<64, 64, _>,
        proof: pallas_codec::utils::decode_bytes_sized::<80, 80, _>,
    )
);

/// Hash of a VRF verification key (Blake2b-256).
pub type VrfKeyhash = Hash<32>;

#[cfg(test)]
mod tests {
    use super::{PoolMetadata, Relay};
    use pallas_codec::minicbor;

    const ABC: [&str; 2] = ["63616263", "7f6161626263ff"];

    #[test]
    fn pool_metadata_url_decodes_in_either_length_form() {
        for url in ABC {
            let raw = hex::decode(format!("82{url}43010203")).unwrap();
            let metadata: PoolMetadata = minicbor::decode(&raw).unwrap();
            assert_eq!(metadata.url, "abc", "{url}");
            assert_eq!(*metadata.hash, vec![1, 2, 3], "{url}");
        }
    }

    #[test]
    fn relay_strings_decode_in_either_length_form() {
        for name in ABC {
            let raw = hex::decode(format!("8301f6{name}")).unwrap();
            let relay: Relay = minicbor::decode(&raw).unwrap();
            assert_eq!(relay, Relay::SingleHostName(None, "abc".into()), "{name}");

            let raw = hex::decode(format!("8202{name}")).unwrap();
            let relay: Relay = minicbor::decode(&raw).unwrap();
            assert_eq!(relay, Relay::MultiHostName("abc".into()), "{name}");
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_version_of_two_numbers_decodes_in_either_length_form() {
        for version in ["82031a8a26c280", "9f031a8a26c280ff"] {
            let raw = hex::decode(version).unwrap();
            let mut d = minicbor::Decoder::new(&raw);
            let decoded = super::protocol_version::decode(&mut d, &mut ()).unwrap();
            assert_eq!(decoded, (3, 2317795968), "{version}");
            assert_eq!(d.position(), raw.len(), "{version}");
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_version_of_three_numbers_is_refused_in_either_length_form() {
        for version in ["83031a8a26c28000", "9f031a8a26c28000ff"] {
            let raw = hex::decode(version).unwrap();
            let mut d = minicbor::Decoder::new(&raw);
            let error = super::protocol_version::decode(&mut d, &mut ()).unwrap_err();
            assert!(
                error.to_string().contains("more elements than"),
                "{version}: {error}"
            );
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_version_reads_a_major_up_to_13_and_a_minor_of_32_bits() {
        let decode = |version: &str| {
            let raw = hex::decode(version).unwrap();
            super::protocol_version::decode(&mut minicbor::Decoder::new(&raw), &mut ())
        };
        assert_eq!(decode("820d1affffffff").unwrap(), (13, u32::MAX as u64));
        assert_eq!(decode("820000").unwrap(), (0, 0));

        for version in ["820e00", "821a0001000000", "820d1b0000000100000000"] {
            let error = decode(version).unwrap_err().to_string();
            assert!(
                error.contains("protocol version") || error.contains("overflow"),
                "{version}: {error}"
            );
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn an_output_or_action_index_is_read_in_16_bits() {
        use crate::TransactionInput;
        use crate::conway::GovActionId;

        let id = "5820".to_string() + &"00".repeat(32);
        let input: TransactionInput =
            minicbor::decode(&hex::decode(format!("82{id}19ffff")).unwrap()).unwrap();
        assert_eq!(input.index, 65535);
        let action: GovActionId =
            minicbor::decode(&hex::decode(format!("82{id}19ffff")).unwrap()).unwrap();
        assert_eq!(action.action_index, 65535);

        let over = hex::decode(format!("82{id}1a00010000")).unwrap();
        for error in [
            minicbor::decode::<TransactionInput>(&over).unwrap_err(),
            minicbor::decode::<GovActionId>(&over).unwrap_err(),
        ] {
            assert!(error.to_string().contains("overflow"), "{error}");
        }
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_rational_with_a_zero_denominator_is_refused() {
        use super::RationalNumber;

        let read: RationalNumber = minicbor::decode(&hex::decode("d81e820001").unwrap()).unwrap();
        assert_eq!((read.numerator, read.denominator), (0, 1));

        let error = minicbor::decode::<RationalNumber>(&hex::decode("d81e820100").unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("zero denominator"), "{error}");
    }
}
