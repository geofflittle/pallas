//! Ledger primitives and cbor codec for the Dijkstra era
//!
//! Handcrafted, idiomatic rust artifacts based on the [Dijkstra CDDL](https://github.com/IntersectMBO/cardano-ledger/blob/1587f21a7d1306dc590c2749a5c66232ef66aad0/eras/dijkstra/impl/cddl/data/dijkstra.cddl)
//! file in the IntersectMBO repo, pinned at commit `1587f21a`. That is the
//! ledger revision the node release `prototype-2026w36` integrates, and so the
//! shape the Musashi testnet has served since it was respun on 2026-09-07. A
//! copy of that file sits next to this one as `defs.cddl`, and every rule this
//! module changes relative to Conway cites its line there.
//!
//! Types that Dijkstra leaves untouched are re-exported from [`crate::conway`]
//! rather than copied. A type is only redefined here when the CDDL rule it
//! models changed, or when a rule it references changed underneath it.

use serde::{Deserialize, Serialize};

use pallas_codec::minicbor::{self, Decode, Encode};

pub use pallas_codec::codec_by_datatype;

pub use crate::{
    AddrKeyhash, AssetName, Bytes, Coin, CostModel, DnsName, Epoch, ExUnits, GenesisDelegateHash,
    Genesishash, Hash, IPv4, IPv6, KeepRaw, MaybeIndefArray, Metadata, Metadatum, MetadatumLabel,
    NetworkId, NonEmptySet, NonZeroInt, Nonce, NonceVariant, Nullable, PlutusScript, PolicyId,
    PoolKeyhash, PoolMetadata, PoolMetadataHash, Port, PositiveCoin, PositiveInterval,
    ProtocolVersion, RationalNumber, Relay, RewardAccount, ScriptHash, Set, StakeCredential,
    TransactionInput, UnitInterval, VrfCert, VrfKeyhash, plutus_data::*,
};

use crate::BTreeMap;

use crate::babbage;

// ----- Header

pub use crate::babbage::OperationalCert;

/// `header_body` (`defs.cddl`) carries twelve fields in Dijkstra where
/// Babbage and Conway carry ten: `block_body_contains_leios_cert` and
/// `eb_announcement` are appended.
///
/// This is why the header cannot be re-exported from Conway the way the rest
/// of the era's unchanged types are. Babbage's `HeaderBody` is a ten field
/// definite array, and minicbor's derived array decoder reads its declared
/// field count and skips whatever follows, so a Dijkstra header decodes as a
/// Babbage one, reports the correct slot, block number and hash, and re-encodes
/// forty one bytes short.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct HeaderBody {
    #[n(0)]
    pub block_number: u64,

    #[n(1)]
    pub slot: u64,

    #[n(2)]
    pub prev_hash: Option<Hash<32>>,

    #[n(3)]
    pub issuer_vkey: Bytes,

    #[n(4)]
    pub vrf_vkey: Bytes,

    #[n(5)]
    pub vrf_result: VrfCert,

    #[n(6)]
    pub block_body_size: u64,

    #[n(7)]
    pub block_body_hash: Hash<32>,

    #[n(8)]
    pub operational_cert: OperationalCert,

    #[n(9)]
    pub protocol_version: ProtocolVersion,

    // -- NEW IN DIJKSTRA
    /// Whether this block's body carries a Leios certificate.
    #[n(10)]
    pub block_body_contains_leios_cert: bool,

    /// Optional announcement of an endorser block.
    #[n(11)]
    pub eb_announcement: Nullable<EbAnnouncement>,
}

impl HeaderBody {
    pub fn leader_vrf_output(&self) -> Vec<u8> {
        babbage::derive_tagged_vrf_output(&self.vrf_result.0, babbage::VrfDerivation::Leader)
    }

    pub fn nonce_vrf_output(&self) -> Vec<u8> {
        babbage::derive_tagged_vrf_output(&self.vrf_result.0, babbage::VrfDerivation::Nonce)
    }
}

/// `header = [header_body, body_signature]` (`defs.cddl`).
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct Header {
    #[n(0)]
    pub header_body: HeaderBody,

    #[n(1)]
    pub body_signature: Bytes,
}

// ----- Types Dijkstra shares with Conway, unchanged.

pub use crate::conway::Multiasset;

pub use crate::conway::Mint;

pub use crate::conway::Value;

pub use crate::conway::Withdrawals;

pub use crate::conway::DRep;

pub use crate::conway::DRepCredential;

pub use crate::conway::CommitteeColdCredential;

pub use crate::conway::CommitteeHotCredential;

pub use crate::conway::Vote;

pub use crate::conway::VotingProcedures;

pub use crate::conway::VotingProcedure;

pub use crate::conway::ProposalProcedure;

pub use crate::conway::GovAction;

pub use crate::conway::Constitution;

pub use crate::conway::Voter;

pub use crate::conway::Anchor;

pub use crate::conway::GovActionId;

pub use crate::conway::PoolVotingThresholds;

pub use crate::conway::DRepVotingThresholds;

pub use crate::conway::ExUnitPrices;

pub use crate::conway::VKeyWitness;

/// `bootstrap_witness` narrows `chain_code` to `bytes .size 32` in Dijkstra
/// (`defs.cddl`). The narrowing has no effect on the CBOR shape, so the Conway
/// type decodes and re-encodes Dijkstra bytes unchanged.
pub use crate::conway::BootstrapWitness;

pub use crate::conway::DatumHash;

pub use crate::conway::DatumOption;

pub use crate::conway::LegacyTransactionOutput;

// ----- Leios and Peras additions to the ranking block.

/// `eb_announcement = [eb_hash : hash32, eb_size : uint .size 4]`
/// (`defs.cddl`). An endorser block reference carried in the header body.
///
/// Named `leios_announcement` with fields `announced_eb` and
/// `announced_eb_size` in the ledger revision the blueprint still mirrors. The
/// rename does not move a byte.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct EbAnnouncement {
    /// Hash of the announced endorser block.
    #[n(0)]
    pub eb_hash: Hash<32>,

    /// Size in bytes of the announced endorser block closure.
    #[n(1)]
    pub eb_size: u32,
}

/// `bls_key = [bls_pubkey : bytes .size 96, bls_possession_proof : bytes .size 48]`
/// (`defs.cddl`). Carried at position 3 of `pool_params`.
///
/// Named `leios_key` in the ledger revision the blueprint still mirrors. The
/// rename does not move a byte.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct BlsKey {
    /// BLS public key.
    #[n(0)]
    pub bls_pubkey: Bytes,

    /// Proof of possession for `bls_pubkey`.
    #[n(1)]
    pub bls_possession_proof: Bytes,
}

/// `leios_signature = bytes .size 48` (`defs.cddl`).
pub type LeiosSignature = Bytes;

/// `leios_certificate = [signers : bytes .size (0 .. 8192), signature : leios_signature]`
/// (`defs.cddl`). Present in stock Dijkstra's `block_body`, not only in the
/// Leios fork, so it must be modelled for any Dijkstra block to round-trip.
///
/// The bound on `signers` is a length constraint on a byte string, so it does
/// not change the shape a decoder has to read. It is a validation rule this
/// type does not enforce, in keeping with every other `.size` bound in the
/// era.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct LeiosCertificate {
    /// Bitfield naming the signers, up to 65536 entries.
    #[n(0)]
    pub signers: Bytes,

    /// Aggregated signature over the certified announcement.
    #[n(1)]
    pub signature: LeiosSignature,
}

/// `peras_certificate = bytes` (`defs.cddl`). Reserved in every Dijkstra
/// ranking block body.
pub type PerasCertificate = Bytes;

// ----- Certificates

/// `certificate` (`defs.cddl`) drops Conway's `account_registration_cert` (0)
/// and `account_unregistration_cert` (1), and `pool_params` gains an optional
/// `bls_key` at position 3 of `pool_params` (`defs.cddl`), shifting `pledge`
/// onward.
///
/// The `bls_key` slot is three-state on the wire: absent entirely, present
/// and `nil`, or present and populated. All three re-encode differently, so
/// the field is an `Option<Nullable<_>>` and the codec is hand written rather
/// than derived, because a mid-array optional cannot be expressed positionally.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Certificate {
    StakeDelegation(StakeCredential, PoolKeyhash),
    PoolRegistration {
        operator: PoolKeyhash,
        vrf_keyhash: VrfKeyhash,
        bls_key: Option<Nullable<BlsKey>>,
        pledge: Coin,
        cost: Coin,
        margin: UnitInterval,
        reward_account: RewardAccount,
        pool_owners: Set<AddrKeyhash>,
        relays: Vec<Relay>,
        pool_metadata: Option<PoolMetadata>,
    },
    PoolRetirement(PoolKeyhash, Epoch),
    Reg(StakeCredential, Coin),
    UnReg(StakeCredential, Coin),
    VoteDeleg(StakeCredential, DRep),
    StakeVoteDeleg(StakeCredential, PoolKeyhash, DRep),
    StakeRegDeleg(StakeCredential, PoolKeyhash, Coin),
    VoteRegDeleg(StakeCredential, DRep, Coin),
    StakeVoteRegDeleg(StakeCredential, PoolKeyhash, DRep, Coin),
    AuthCommitteeHot(CommitteeColdCredential, CommitteeHotCredential),
    ResignCommitteeCold(CommitteeColdCredential, Option<Anchor>),
    RegDRepCert(DRepCredential, Coin, Option<Anchor>),
    UnRegDRepCert(DRepCredential, Coin),
    UpdateDRepCert(DRepCredential, Option<Anchor>),
}

impl<'b, C> minicbor::Decode<'b, C> for Certificate {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        d.array()?;
        let variant = d.u16()?;

        match variant {
            2 => Ok(Certificate::StakeDelegation(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            3 => {
                let operator = d.decode_with(ctx)?;
                let vrf_keyhash = d.decode_with(ctx)?;

                // `? bls_key : bls_key/ nil` sits between `vrf_keyhash` and
                // `pledge`. `pledge` is a coin, so a uint here means the slot
                // was omitted, and an array or a null means it was supplied.
                let bls_key = match d.datatype()? {
                    minicbor::data::Type::Array | minicbor::data::Type::ArrayIndef => {
                        Some(Nullable::Some(d.decode_with(ctx)?))
                    }
                    minicbor::data::Type::Null => {
                        d.null()?;
                        Some(Nullable::Null)
                    }
                    _ => None,
                };

                Ok(Certificate::PoolRegistration {
                    operator,
                    vrf_keyhash,
                    bls_key,
                    pledge: d.decode_with(ctx)?,
                    cost: d.decode_with(ctx)?,
                    margin: d.decode_with(ctx)?,
                    reward_account: d.decode_with(ctx)?,
                    pool_owners: d.decode_with(ctx)?,
                    relays: d.decode_with(ctx)?,
                    pool_metadata: d.decode_with(ctx)?,
                })
            }
            4 => Ok(Certificate::PoolRetirement(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            7 => Ok(Certificate::Reg(d.decode_with(ctx)?, d.decode_with(ctx)?)),
            8 => Ok(Certificate::UnReg(d.decode_with(ctx)?, d.decode_with(ctx)?)),
            9 => Ok(Certificate::VoteDeleg(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            10 => Ok(Certificate::StakeVoteDeleg(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            11 => Ok(Certificate::StakeRegDeleg(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            12 => Ok(Certificate::VoteRegDeleg(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            13 => Ok(Certificate::StakeVoteRegDeleg(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            14 => Ok(Certificate::AuthCommitteeHot(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            15 => Ok(Certificate::ResignCommitteeCold(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            16 => Ok(Certificate::RegDRepCert(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            17 => Ok(Certificate::UnRegDRepCert(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            18 => Ok(Certificate::UpdateDRepCert(
                d.decode_with(ctx)?,
                d.decode_with(ctx)?,
            )),
            // Conway's variants 0 and 1 were removed in Dijkstra and are
            // refused rather than silently accepted.
            0 | 1 => Err(minicbor::decode::Error::message(format!(
                "certificate variant {variant} was removed in Dijkstra"
            ))),
            _ => Err(minicbor::decode::Error::message(format!(
                "unknown certificate variant {variant}"
            ))),
        }
    }
}

impl<C> minicbor::Encode<C> for Certificate {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Certificate::StakeDelegation(a, b) => {
                e.array(3)?;
                e.encode_with(2u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::PoolRegistration {
                operator,
                vrf_keyhash,
                bls_key,
                pledge,
                cost,
                margin,
                reward_account,
                pool_owners,
                relays,
                pool_metadata,
            } => {
                e.array(if bls_key.is_some() { 11 } else { 10 })?;
                e.encode_with(3u16, ctx)?;
                e.encode_with(operator, ctx)?;
                e.encode_with(vrf_keyhash, ctx)?;
                match bls_key {
                    Some(Nullable::Some(k)) => {
                        e.encode_with(k, ctx)?;
                    }
                    Some(_) => {
                        e.null()?;
                    }
                    None => {}
                }
                e.encode_with(pledge, ctx)?;
                e.encode_with(cost, ctx)?;
                e.encode_with(margin, ctx)?;
                e.encode_with(reward_account, ctx)?;
                e.encode_with(pool_owners, ctx)?;
                e.encode_with(relays, ctx)?;
                e.encode_with(pool_metadata, ctx)?;
            }
            Certificate::PoolRetirement(a, b) => {
                e.array(3)?;
                e.encode_with(4u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::Reg(a, b) => {
                e.array(3)?;
                e.encode_with(7u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::UnReg(a, b) => {
                e.array(3)?;
                e.encode_with(8u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::VoteDeleg(a, b) => {
                e.array(3)?;
                e.encode_with(9u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::StakeVoteDeleg(a, b, c) => {
                e.array(4)?;
                e.encode_with(10u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;
            }
            Certificate::StakeRegDeleg(a, b, c) => {
                e.array(4)?;
                e.encode_with(11u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;
            }
            Certificate::VoteRegDeleg(a, b, c) => {
                e.array(4)?;
                e.encode_with(12u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;
            }
            Certificate::StakeVoteRegDeleg(a, b, c, dd) => {
                e.array(5)?;
                e.encode_with(13u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;
                e.encode_with(dd, ctx)?;
            }
            Certificate::AuthCommitteeHot(a, b) => {
                e.array(3)?;
                e.encode_with(14u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::ResignCommitteeCold(a, b) => {
                e.array(3)?;
                e.encode_with(15u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::RegDRepCert(a, b, c) => {
                e.array(4)?;
                e.encode_with(16u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
                e.encode_with(c, ctx)?;
            }
            Certificate::UnRegDRepCert(a, b) => {
                e.array(3)?;
                e.encode_with(17u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
            Certificate::UpdateDRepCert(a, b) => {
                e.array(3)?;
                e.encode_with(18u16, ctx)?;
                e.encode_with(a, ctx)?;
                e.encode_with(b, ctx)?;
            }
        }

        Ok(())
    }
}

// ----- Plutus languages and cost models

/// `language = 0 .. 3` in Dijkstra, widened from Conway's `0 .. 2`
/// (`defs.cddl`). PlutusV4 is a new variant rather than a rename.
#[derive(
    Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone, PartialOrd, Ord, Hash,
)]
#[cbor(index_only)]
pub enum Language {
    #[n(0)]
    PlutusV1,

    #[n(1)]
    PlutusV2,

    #[n(2)]
    PlutusV3,

    #[n(3)]
    PlutusV4,
}

/// `cost_models` promotes key 3 from Conway's `3 .. 255` wildcard to a named
/// PlutusV4 key, leaving the wildcard at `4 .. 255` (`defs.cddl`).
#[derive(Serialize, Deserialize, Encode, Debug, PartialEq, Eq, Clone)]
#[cbor(map)]
pub struct CostModels {
    #[n(0)]
    pub plutus_v1: Option<CostModel>,

    #[n(1)]
    pub plutus_v2: Option<CostModel>,

    #[n(2)]
    pub plutus_v3: Option<CostModel>,

    #[n(3)]
    pub plutus_v4: Option<CostModel>,

    #[cbor(skip)]
    pub unknown: BTreeMap<u64, CostModel>,
}

impl<'b, C> minicbor::Decode<'b, C> for CostModels {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let models: BTreeMap<u64, CostModel> = d.decode_with(ctx)?;

        let mut plutus_v1 = None;
        let mut plutus_v2 = None;
        let mut plutus_v3 = None;
        let mut plutus_v4 = None;
        let mut unknown: Vec<(u64, CostModel)> = Vec::new();

        for (k, v) in models.iter() {
            match k {
                0 => plutus_v1 = Some(v.clone()),
                1 => plutus_v2 = Some(v.clone()),
                2 => plutus_v3 = Some(v.clone()),
                3 => plutus_v4 = Some(v.clone()),
                _ => unknown.push((*k, v.clone())),
            }
        }

        Ok(Self {
            plutus_v1,
            plutus_v2,
            plutus_v3,
            plutus_v4,
            unknown: unknown.into_iter().collect(),
        })
    }
}

/// `protocol_param_update` (`defs.cddl`) gains keys 34 through 37, the four
/// reference script parameters, and keys 38 through 48, which carry the pledge
/// and margin limits, the five Leios period and committee parameters, and the
/// four endorser block limits.
///
/// Keys 38 through 48 are what the node reads its Leios configuration from as
/// of `prototype-2026w36`, where the earlier build took them from the Dijkstra
/// genesis file instead. A parameter update that sets one of them decodes as
/// an empty update through a type that stops at 37, which is why the tail is
/// modelled rather than left to a catch-all.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
#[cbor(map)]
pub struct ProtocolParamUpdate {
    #[n(0)]
    pub minfee_a: Option<u64>,
    #[n(1)]
    pub minfee_b: Option<u64>,
    #[n(2)]
    pub max_block_body_size: Option<u64>,
    #[n(3)]
    pub max_transaction_size: Option<u64>,
    #[n(4)]
    pub max_block_header_size: Option<u64>,
    #[n(5)]
    pub key_deposit: Option<Coin>,
    #[n(6)]
    pub pool_deposit: Option<Coin>,
    #[n(7)]
    pub maximum_epoch: Option<Epoch>,
    #[n(8)]
    pub desired_number_of_stake_pools: Option<u64>,
    #[n(9)]
    pub pool_pledge_influence: Option<RationalNumber>,
    #[n(10)]
    pub expansion_rate: Option<UnitInterval>,
    #[n(11)]
    pub treasury_growth_rate: Option<UnitInterval>,

    #[n(16)]
    pub min_pool_cost: Option<Coin>,
    #[n(17)]
    pub ada_per_utxo_byte: Option<Coin>,
    #[n(18)]
    pub cost_models_for_script_languages: Option<CostModels>,
    #[n(19)]
    pub execution_costs: Option<ExUnitPrices>,
    #[n(20)]
    pub max_tx_ex_units: Option<ExUnits>,
    #[n(21)]
    pub max_block_ex_units: Option<ExUnits>,
    #[n(22)]
    pub max_value_size: Option<u64>,
    #[n(23)]
    pub collateral_percentage: Option<u64>,
    #[n(24)]
    pub max_collateral_inputs: Option<u64>,

    #[n(25)]
    pub pool_voting_thresholds: Option<PoolVotingThresholds>,
    #[n(26)]
    pub drep_voting_thresholds: Option<DRepVotingThresholds>,
    #[n(27)]
    pub min_committee_size: Option<u64>,
    #[n(28)]
    pub committee_term_limit: Option<Epoch>,
    #[n(29)]
    pub governance_action_validity_period: Option<Epoch>,
    #[n(30)]
    pub governance_action_deposit: Option<Coin>,
    #[n(31)]
    pub drep_deposit: Option<Coin>,
    #[n(32)]
    pub drep_inactivity_period: Option<Epoch>,
    #[n(33)]
    pub minfee_refscript_cost_per_byte: Option<UnitInterval>,

    // -- NEW IN DIJKSTRA
    #[n(34)]
    pub max_ref_script_size_per_block: Option<u64>,
    #[n(35)]
    pub max_ref_script_size_per_tx: Option<u64>,
    #[n(36)]
    pub ref_script_cost_stride: Option<u64>,
    #[n(37)]
    pub ref_script_cost_multiplier: Option<PositiveInterval>,

    // -- NEW IN THE prototype-2026w36 LEDGER
    /// Key 38. `max_pledge_leverage = nonnegative_interval/ nil`
    /// (`defs.cddl`), so the slot is
    /// three-state and an explicit `nil` is not the same as an absent key.
    #[n(38)]
    pub max_pledge_leverage: Option<Nullable<RationalNumber>>,
    #[n(39)]
    pub min_pool_margin: Option<UnitInterval>,
    #[n(40)]
    pub leios_announcement_period_length: Option<u64>,
    #[n(41)]
    pub leios_vote_period_length: Option<u64>,
    #[n(42)]
    pub leios_diffusion_period_length: Option<u64>,
    #[n(43)]
    pub leios_committee_size: Option<u64>,
    #[n(44)]
    pub leios_quorum_stake_threshold: Option<UnitInterval>,
    #[n(45)]
    pub max_endorser_block_references_size: Option<u64>,
    #[n(46)]
    pub max_endorser_block_txs_size: Option<u64>,
    #[n(47)]
    pub max_endorser_block_execution_units: Option<ExUnits>,
    #[n(48)]
    pub max_ref_script_size_per_endorser_block: Option<u64>,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct Update {
    #[n(0)]
    pub proposed_protocol_parameter_updates: BTreeMap<Genesishash, ProtocolParamUpdate>,

    #[n(1)]
    pub epoch: Epoch,
}

// ----- Transaction body

/// `guards = nonempty_set<addr_keyhash>/ nonempty_oset<credential>`
/// (`defs.cddl`). Replaces Conway's `required_signers` at key 14 and
/// widens it: the second arm carries credentials, which a decoder typed as
/// `NonEmptySet<AddrKeyhash>` cannot read.
///
/// The two arms are told apart by element type: an `addr_keyhash` is a byte
/// string and a `credential` is an array.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Guards {
    AddrKeyhashes(NonEmptySet<AddrKeyhash>),
    Credentials(NonEmptySet<StakeCredential>),
}

impl<'b, C> minicbor::Decode<'b, C> for Guards {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        // Look ahead past the optional set tag and the array header to reach
        // the first element, without consuming anything.
        let mut probe = minicbor::Decoder::new(&d.input()[d.position()..]);
        if probe.datatype()? == minicbor::data::Type::Tag {
            probe.tag()?;
        }
        probe.array()?;

        match probe.datatype()? {
            minicbor::data::Type::Array | minicbor::data::Type::ArrayIndef => {
                Ok(Guards::Credentials(d.decode_with(ctx)?))
            }
            minicbor::data::Type::Bytes | minicbor::data::Type::BytesIndef => {
                Ok(Guards::AddrKeyhashes(d.decode_with(ctx)?))
            }
            other => Err(minicbor::decode::Error::message(format!(
                "invalid guards element type {other} at position {}",
                d.position()
            ))),
        }
    }
}

impl<C> minicbor::Encode<C> for Guards {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Guards::AddrKeyhashes(x) => e.encode_with(x, ctx)?,
            Guards::Credentials(x) => e.encode_with(x, ctx)?,
        };

        Ok(())
    }
}

/// `direct_deposits = {+ reward_account => coin}` (`defs.cddl`).
pub type DirectDeposits = BTreeMap<RewardAccount, Coin>;

/// `required_top_level_guards = {+ credential => plutus_data/ nil}`
/// (`defs.cddl`). Carried by a transaction body at key 24 and by a sub
/// transaction body at the same key.
pub type RequiredTopLevelGuards = BTreeMap<StakeCredential, Nullable<PlutusData>>;

/// `account_balance_intervals = {+ reward_account => account_balance_interval}`
/// (`defs.cddl`).
///
/// Keyed by `credential` in the ledger revision the blueprint still mirrors.
/// A reward account is a byte string and a credential is an array, so the two
/// keyings are not interchangeable on the wire.
pub type AccountBalanceIntervals = BTreeMap<RewardAccount, AccountBalanceInterval>;

/// `starting_account_balance_intervals = {+ reward_account => account_balance_interval}`
/// (`defs.cddl`). Carried by a transaction body at key 27. Same shape as
/// `account_balance_intervals`, a distinct rule and a distinct key.
pub type StartingAccountBalanceIntervals = BTreeMap<RewardAccount, AccountBalanceInterval>;

/// `account_balance_interval` (`defs.cddl`) is either a two element array
/// whose two arms between them permit a missing lower bound or a missing upper
/// bound but never both, or a bare `coin`. The four legal shapes are named
/// here so that the fifth, an array with neither bound, cannot be constructed
/// or decoded.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AccountBalanceInterval {
    /// `[coin, nil]`
    LowerBound(Coin),
    /// `[coin, coin]`
    Bounded(Coin, Coin),
    /// `[nil, coin]`
    UpperBound(Coin),
    /// A bare `coin`, with no enclosing array.
    Exact(Coin),
}

impl<'b, C> minicbor::Decode<'b, C> for AccountBalanceInterval {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        // The bare `coin` arm is a uint and the two bounded arms are arrays, so
        // the shape is decided before anything is consumed.
        match d.datatype()? {
            minicbor::data::Type::Array | minicbor::data::Type::ArrayIndef => {}
            _ => return Ok(AccountBalanceInterval::Exact(d.decode_with(ctx)?)),
        }

        d.array()?;

        let lower: Option<Coin> = match d.datatype()? {
            minicbor::data::Type::Null => {
                d.null()?;
                None
            }
            _ => Some(d.decode_with(ctx)?),
        };

        let upper: Option<Coin> = match d.datatype()? {
            minicbor::data::Type::Null => {
                d.null()?;
                None
            }
            _ => Some(d.decode_with(ctx)?),
        };

        match (lower, upper) {
            (Some(l), None) => Ok(AccountBalanceInterval::LowerBound(l)),
            (Some(l), Some(u)) => Ok(AccountBalanceInterval::Bounded(l, u)),
            (None, Some(u)) => Ok(AccountBalanceInterval::UpperBound(u)),
            (None, None) => Err(minicbor::decode::Error::message(
                "account_balance_interval with neither a lower nor an upper bound",
            )),
        }
    }
}

impl<C> minicbor::Encode<C> for AccountBalanceInterval {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        // The bare arm carries no array header, so the header is written inside
        // the three bounded arms rather than ahead of the match.
        match self {
            AccountBalanceInterval::LowerBound(l) => {
                e.array(2)?;
                e.encode_with(l, ctx)?;
                e.null()?;
            }
            AccountBalanceInterval::Bounded(l, u) => {
                e.array(2)?;
                e.encode_with(l, ctx)?;
                e.encode_with(u, ctx)?;
            }
            AccountBalanceInterval::UpperBound(u) => {
                e.array(2)?;
                e.null()?;
                e.encode_with(u, ctx)?;
            }
            AccountBalanceInterval::Exact(c) => {
                e.encode_with(c, ctx)?;
            }
        }

        Ok(())
    }
}

/// `sub_transaction_body` (`defs.cddl`), the body of a nested transaction.
/// It is Conway's body minus the keys a sub transaction cannot carry, plus
/// `required_top_level_guards` at key 24.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
#[cbor(map)]
pub struct SubTransactionBody<'a> {
    #[n(0)]
    pub inputs: Set<TransactionInput>,

    #[b(1)]
    pub outputs: MaybeIndefArray<TransactionOutput<'a>>,

    #[n(3)]
    pub ttl: Option<u64>,

    #[n(4)]
    pub certificates: Option<NonEmptySet<Certificate>>,

    #[n(5)]
    pub withdrawals: Option<Withdrawals>,

    #[n(7)]
    pub auxiliary_data_hash: Option<Hash<32>>,

    #[n(8)]
    pub validity_interval_start: Option<u64>,

    #[n(9)]
    pub mint: Option<Multiasset<NonZeroInt>>,

    #[n(11)]
    pub script_data_hash: Option<Hash<32>>,

    #[n(14)]
    pub guards: Option<Guards>,

    #[n(15)]
    pub network_id: Option<NetworkId>,

    #[n(18)]
    pub reference_inputs: Option<NonEmptySet<TransactionInput>>,

    #[n(19)]
    pub voting_procedures: Option<VotingProcedures>,

    #[n(20)]
    pub proposal_procedures: Option<NonEmptySet<ProposalProcedure>>,

    #[n(21)]
    pub treasury_value: Option<Coin>,

    #[n(22)]
    pub donation: Option<PositiveCoin>,

    #[n(24)]
    pub required_top_level_guards: Option<RequiredTopLevelGuards>,

    #[n(25)]
    pub direct_deposits: Option<DirectDeposits>,

    #[n(26)]
    pub account_balance_intervals: Option<AccountBalanceIntervals>,
}

/// `sub_transaction = [sub_transaction_body, transaction_witness_set, auxiliary_data/ nil]`
/// (`defs.cddl`). A sub transaction carries no validity flag, in Dijkstra
/// or in the w36 ledger: only a `block_transaction` does.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
pub struct SubTransaction<'b> {
    #[b(0)]
    pub sub_transaction_body: KeepRaw<'b, SubTransactionBody<'b>>,

    #[n(1)]
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,

    #[n(2)]
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,
}

/// `sub_transactions = nonempty_oset<sub_transaction>` (`defs.cddl`).
pub type SubTransactions<'b> = NonEmptySet<SubTransaction<'b>>;

/// `transaction_body` (`defs.cddl`). Relative to Conway: key 14 is `guards`
/// rather than `required_signers`, and keys 23 through 27 are new.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
#[cbor(map)]
pub struct TransactionBody<'a> {
    #[n(0)]
    pub inputs: Set<TransactionInput>,

    /// Emitted as an indefinite length array by at least one transaction in
    /// the measured corpus, so the distinction is preserved here.
    #[b(1)]
    pub outputs: MaybeIndefArray<TransactionOutput<'a>>,

    #[n(2)]
    pub fee: Coin,

    #[n(3)]
    pub ttl: Option<u64>,

    #[n(4)]
    pub certificates: Option<NonEmptySet<Certificate>>,

    #[n(5)]
    pub withdrawals: Option<Withdrawals>,

    #[n(7)]
    pub auxiliary_data_hash: Option<Hash<32>>,

    #[n(8)]
    pub validity_interval_start: Option<u64>,

    #[n(9)]
    pub mint: Option<Multiasset<NonZeroInt>>,

    #[n(11)]
    pub script_data_hash: Option<Hash<32>>,

    #[n(13)]
    pub collateral: Option<NonEmptySet<TransactionInput>>,

    /// Key 14, `required_signers` in Conway, widened to `guards` in Dijkstra.
    #[n(14)]
    pub guards: Option<Guards>,

    #[n(15)]
    pub network_id: Option<NetworkId>,

    #[n(16)]
    pub collateral_return: Option<TransactionOutput<'a>>,

    #[n(17)]
    pub total_collateral: Option<Coin>,

    #[n(18)]
    pub reference_inputs: Option<NonEmptySet<TransactionInput>>,

    #[n(19)]
    pub voting_procedures: Option<VotingProcedures>,

    #[n(20)]
    pub proposal_procedures: Option<NonEmptySet<ProposalProcedure>>,

    #[n(21)]
    pub treasury_value: Option<Coin>,

    #[n(22)]
    pub donation: Option<PositiveCoin>,

    // -- NEW IN DIJKSTRA
    #[b(23)]
    pub sub_transactions: Option<SubTransactions<'a>>,

    /// Key 24. Already reachable through a sub transaction body in the earlier
    /// ledger revision, and promoted to the top level body by the w36 ledger.
    #[n(24)]
    pub required_top_level_guards: Option<RequiredTopLevelGuards>,

    #[n(25)]
    pub direct_deposits: Option<DirectDeposits>,

    #[n(26)]
    pub account_balance_intervals: Option<AccountBalanceIntervals>,

    // -- NEW IN THE prototype-2026w36 LEDGER
    #[n(27)]
    pub starting_account_balance_intervals: Option<StartingAccountBalanceIntervals>,
}

// ----- Outputs

pub type PostAlonzoTransactionOutput<'b> =
    babbage::GenPostAlonzoTransactionOutput<'b, Value, ScriptRef<'b>>;

pub type TransactionOutput<'b> = babbage::GenTransactionOutput<'b, PostAlonzoTransactionOutput<'b>>;

// FIXME: Repeated since macro does not handle type generics yet.
codec_by_datatype! {
    TransactionOutput<'b>,
    Array | ArrayIndef => Legacy,
    Map | MapIndef => PostAlonzo,
    ()
}

// ----- Scripts and witnesses

/// `native_script` gains `script_require_guard = (6, credential)`
/// (`defs.cddl`), so the Conway type cannot be reused even though the
/// witness set rule that reaches it is unchanged.
#[derive(Encode, Decode, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[cbor(flat)]
pub enum NativeScript {
    #[n(0)]
    ScriptPubkey(#[n(0)] AddrKeyhash),
    #[n(1)]
    ScriptAll(#[n(0)] Vec<NativeScript>),
    #[n(2)]
    ScriptAny(#[n(0)] Vec<NativeScript>),
    #[n(3)]
    ScriptNOfK(#[n(0)] u32, #[n(1)] Vec<NativeScript>),
    #[n(4)]
    InvalidBefore(#[n(0)] u64),
    #[n(5)]
    InvalidHereafter(#[n(0)] u64),

    // -- NEW IN DIJKSTRA
    #[n(6)]
    ScriptRequireGuard(#[n(0)] StakeCredential),
}

/// `redeemer_tag` gains tag 6, `guarding` (`defs.cddl`).
#[derive(
    Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord,
)]
#[cbor(index_only)]
pub enum RedeemerTag {
    #[n(0)]
    Spend,
    #[n(1)]
    Mint,
    #[n(2)]
    Cert,
    #[n(3)]
    Reward,
    #[n(4)]
    Vote,
    #[n(5)]
    Propose,

    // -- NEW IN DIJKSTRA
    #[n(6)]
    Guarding,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct Redeemer {
    #[n(0)]
    pub tag: RedeemerTag,

    #[n(1)]
    pub index: u32,

    #[n(2)]
    pub data: PlutusData,

    #[n(3)]
    pub ex_units: ExUnits,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub struct RedeemersKey {
    #[n(0)]
    pub tag: RedeemerTag,
    #[n(1)]
    pub index: u32,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct RedeemersValue {
    #[n(0)]
    pub data: PlutusData,
    #[n(1)]
    pub ex_units: ExUnits,
}

/// `redeemers` is map only in Dijkstra (`defs.cddl`): Conway's array arm and
/// the `redeemer` rule behind it were both deleted, so the array form is not
/// representable here rather than being accepted and re-encoded.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
#[cbor(transparent)]
pub struct Redeemers(pub BTreeMap<RedeemersKey, RedeemersValue>);

impl From<BTreeMap<RedeemersKey, RedeemersValue>> for Redeemers {
    fn from(value: BTreeMap<RedeemersKey, RedeemersValue>) -> Self {
        Redeemers(value)
    }
}

impl std::ops::Deref for Redeemers {
    type Target = BTreeMap<RedeemersKey, RedeemersValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// `transaction_witness_set` (`defs.cddl`). The rule text is identical to
/// Conway's, but two of the rules it reaches are not: `native_script` gained a
/// variant and `redeemers` lost its array arm, so this type is redefined here
/// rather than re-exported.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
#[cbor(map)]
pub struct WitnessSet<'b> {
    #[n(0)]
    pub vkeywitness: Option<NonEmptySet<VKeyWitness>>,

    #[n(1)]
    pub native_script: Option<NonEmptySet<KeepRaw<'b, NativeScript>>>,

    #[n(2)]
    pub bootstrap_witness: Option<NonEmptySet<BootstrapWitness>>,

    #[n(3)]
    pub plutus_v1_script: Option<NonEmptySet<PlutusScript<1>>>,

    #[b(4)]
    pub plutus_data: Option<KeepRaw<'b, NonEmptySet<KeepRaw<'b, PlutusData>>>>,

    #[n(5)]
    pub redeemer: Option<KeepRaw<'b, Redeemers>>,

    #[n(6)]
    pub plutus_v2_script: Option<NonEmptySet<PlutusScript<2>>>,

    #[n(7)]
    pub plutus_v3_script: Option<NonEmptySet<PlutusScript<3>>>,
}

/// `auxiliary_data_map` gains `? 5 : [* plutus_v4_script]` (`defs.cddl`).
///
/// Keys 3 and 4 are also carried here. Conway's CDDL already defines them and
/// the live Conway type in this crate does not, so a Conway auxiliary data
/// value carrying a PlutusV2 or PlutusV3 script loses it on re-encode. This
/// type does not inherit that.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone, Eq)]
#[cbor(map, tag(259))]
pub struct PostAlonzoAuxiliaryData {
    #[n(0)]
    pub metadata: Option<Metadata>,

    #[n(1)]
    pub native_scripts: Option<Vec<NativeScript>>,

    #[n(2)]
    pub plutus_v1_scripts: Option<Vec<PlutusScript<1>>>,

    #[n(3)]
    pub plutus_v2_scripts: Option<Vec<PlutusScript<2>>>,

    #[n(4)]
    pub plutus_v3_scripts: Option<Vec<PlutusScript<3>>>,

    // -- NEW IN DIJKSTRA
    #[n(5)]
    pub plutus_v4_scripts: Option<Vec<PlutusScript<4>>>,
}

pub use crate::alonzo::ShelleyMaAuxiliaryData;

/// `auxiliary_data` (`defs.cddl`). Redefined rather than re-exported from
/// Conway because its post Alonzo arm gained a key.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Eq)]
pub enum AuxiliaryData {
    Shelley(Metadata),
    ShelleyMa(ShelleyMaAuxiliaryData),
    PostAlonzo(PostAlonzoAuxiliaryData),
}

codec_by_datatype! {
    AuxiliaryData,
    Map | MapIndef => Shelley,
    Array | ArrayIndef => ShelleyMa,
    Tag => PostAlonzo,
    ()
}

/// `script` gains `// 4, plutus_v4_script` (`defs.cddl`), reached through
/// `script_ref = #6.24(bytes .cbor script)`.
#[derive(Encode, Decode, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[cbor(flat)]
pub enum ScriptRef<'b> {
    #[n(0)]
    NativeScript(#[b(0)] KeepRaw<'b, NativeScript>),
    #[n(1)]
    PlutusV1Script(#[n(0)] PlutusScript<1>),
    #[n(2)]
    PlutusV2Script(#[n(0)] PlutusScript<2>),
    #[n(3)]
    PlutusV3Script(#[n(0)] PlutusScript<3>),

    // -- NEW IN DIJKSTRA
    #[n(4)]
    PlutusV4Script(#[n(0)] PlutusScript<4>),
}

// ----- Block and transaction

/// `block_body` (`defs.cddl`). Dijkstra replaces Conway's segregated
/// witness layout with complete inline transactions, and reserves a Leios and
/// a Peras certificate slot in every ranking block.
///
/// Three elements, not four. The earlier ledger revision led with an
/// `invalid_transactions` index set and carried the validity of each
/// transaction there. The w36 ledger deletes that element and the
/// `transaction_index` rule behind it, and moves validity into each
/// transaction as a fourth field, so the two shapes cannot both decode.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
pub struct BlockBody<'b> {
    /// The node emits this list as an indefinite length array in 827 of the
    /// 1478 blocks measured off the Musashi immutable database, and as a
    /// definite one in the rest, so the distinction is preserved rather than
    /// normalised.
    #[b(0)]
    pub transactions: MaybeIndefArray<BlockTransaction<'b>>,

    #[n(1)]
    pub leios_certificate: Nullable<LeiosCertificate>,

    #[n(2)]
    pub peras_certificate: Nullable<PerasCertificate>,
}

/// `block = [header, block_body]` (`defs.cddl`).
///
/// This structure allows to retrieve the original CBOR bytes for each
/// structure that might require hashing, so that the resulting hash matches
/// what exists on-chain.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
pub struct Block<'b> {
    #[n(0)]
    pub header: KeepRaw<'b, Header>,

    #[b(1)]
    pub block_body: BlockBody<'b>,
}

/// `block_transaction = [transaction_body, transaction_witness_set, auxiliary_data/ nil, bool]`
/// (`defs.cddl`). Four elements, with the validity flag last. This is what a
/// ranking block body carries, and the only place the flag appears.
///
/// Conway carries the same flag at position 2, ahead of the auxiliary data.
/// Dijkstra moves it behind, so that every field the transaction author
/// supplies comes first and the one field the block producer sets comes last.
/// The field is named `success` to match [`crate::conway::Tx`], since it
/// answers the same question and reaches the same accessor.
///
/// The era's other transaction rule, [`MempoolTransaction`], is three elements
/// and is a distinct type rather than this one with the flag made optional.
/// Both are live on the wire at once, and neither decodes as the other, so
/// which of the two a byte string is has to be a matter of which type read it.
#[derive(Clone, Serialize, Deserialize, Encode, Decode, Debug, PartialEq)]
pub struct BlockTransaction<'b> {
    #[b(0)]
    pub transaction_body: KeepRaw<'b, TransactionBody<'b>>,

    #[n(1)]
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,

    #[n(2)]
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,

    /// Set by the block producer, not by the transaction author. A transaction
    /// only has this field once it is in a block, which is why
    /// [`MempoolTransaction`] does not share this type.
    #[n(3)]
    pub success: bool,
}

impl Eq for BlockTransaction<'_> {}

impl<'b> BlockTransaction<'b> {
    /// The same transaction as its author submitted it, without the flag the
    /// block producer added. Re-encodes as three elements, which is the shape
    /// an endorser block closure and a mempool submission both carry.
    pub fn to_mempool_transaction(&self) -> MempoolTransaction<'b> {
        MempoolTransaction {
            transaction_body: self.transaction_body.clone(),
            transaction_witness_set: self.transaction_witness_set.clone(),
            is_valid_supplied: false,
            auxiliary_data: self.auxiliary_data.clone(),
        }
    }
}

/// `mempool_transaction` (`defs.cddl`) is three elements, and tolerates the
/// Conway four element shape on submission only, with `is_valid` required to be
/// `true`.
///
/// Two paths carry this shape rather than [`BlockTransaction`]: a client
/// submitting a transaction, and an endorser block closure travelling over
/// leios-fetch. Neither has a block producer's verdict to report, so neither
/// carries the flag.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct MempoolTransaction<'b> {
    pub transaction_body: KeepRaw<'b, TransactionBody<'b>>,
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,
    /// `true` when the submitted encoding carried the deprecated `is_valid`
    /// flag, which is the only value the flag is allowed to take.
    pub is_valid_supplied: bool,
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,
}

impl<'b, C> minicbor::Decode<'b, C> for MempoolTransaction<'b> {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d.array()?;

        let transaction_body = d.decode_with(ctx)?;
        let transaction_witness_set = d.decode_with(ctx)?;

        let is_valid_supplied = match d.datatype()? {
            minicbor::data::Type::Bool => {
                if d.bool()? {
                    true
                } else {
                    return Err(minicbor::decode::Error::message(
                        "value `false` not allowed for `is_valid`",
                    ));
                }
            }
            _ => false,
        };

        let expected = if is_valid_supplied { 4 } else { 3 };

        if let Some(len) = len
            && len != expected
        {
            return Err(minicbor::decode::Error::message(format!(
                "expected a {expected} element transaction, found {len}"
            )));
        }

        Ok(MempoolTransaction {
            transaction_body,
            transaction_witness_set,
            is_valid_supplied,
            auxiliary_data: d.decode_with(ctx)?,
        })
    }
}

impl<C> minicbor::Encode<C> for MempoolTransaction<'_> {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.array(if self.is_valid_supplied { 4 } else { 3 })?;
        e.encode_with(&self.transaction_body, ctx)?;
        e.encode_with(&self.transaction_witness_set, ctx)?;
        if self.is_valid_supplied {
            e.bool(true)?;
        }
        e.encode_with(&self.auxiliary_data, ctx)?;

        Ok(())
    }
}

impl Eq for MempoolTransaction<'_> {}

#[cfg(test)]
mod tests {
    use super::{Block, BlockTransaction, Header, MempoolTransaction};
    use pallas_codec::minicbor;
    use pallas_codec::utils::KeepRaw;

    type BlockWrapper<'b> = (u16, Block<'b>);

    /// Every fixture is a real Musashi testnet block, lifted out of the node's
    /// own immutable database and checked against the CRC32 in its secondary
    /// index before being written here.
    const TEST_BLOCKS: &[(&str, &str)] = &[
        (
            "dijkstra1",
            include_str!("../../../test_data/dijkstra1.block"),
        ),
        (
            "dijkstra2",
            include_str!("../../../test_data/dijkstra2.block"),
        ),
        (
            "dijkstra3",
            include_str!("../../../test_data/dijkstra3.block"),
        ),
        (
            "dijkstra4",
            include_str!("../../../test_data/dijkstra4.block"),
        ),
        (
            "dijkstra5",
            include_str!("../../../test_data/dijkstra5.block"),
        ),
        (
            "dijkstra6",
            include_str!("../../../test_data/dijkstra6.block"),
        ),
        (
            "dijkstra7",
            include_str!("../../../test_data/dijkstra7.block"),
        ),
        (
            "dijkstra8",
            include_str!("../../../test_data/dijkstra8.block"),
        ),
        (
            "dijkstra9",
            include_str!("../../../test_data/dijkstra9.block"),
        ),
        (
            "dijkstra10",
            include_str!("../../../test_data/dijkstra10.block"),
        ),
    ];

    #[test]
    fn block_isomorphic_decoding_encoding() {
        for (name, block_str) in TEST_BLOCKS.iter() {
            let bytes = hex::decode(block_str).unwrap_or_else(|_| panic!("bad block file {name}"));

            let block: BlockWrapper = minicbor::decode(&bytes)
                .unwrap_or_else(|e| panic!("error decoding cbor for file {name}: {e:?}"));

            let bytes2 = minicbor::to_vec(block)
                .unwrap_or_else(|e| panic!("error encoding block cbor for file {name}: {e:?}"));

            let first_diff = bytes
                .iter()
                .zip(bytes2.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(bytes.len().min(bytes2.len()));

            assert!(
                bytes.eq(&bytes2),
                "{name}: re-encoded bytes didn't match original, in {} out {}, first difference at byte {first_diff}\n  orig {}\n  ours {}",
                bytes.len(),
                bytes2.len(),
                hex::encode(
                    &bytes[first_diff.saturating_sub(8)..(first_diff + 24).min(bytes.len())]
                ),
                hex::encode(
                    &bytes2[first_diff.saturating_sub(8)..(first_diff + 24).min(bytes2.len())]
                ),
            );
        }
    }

    /// The block level roundtrip above cannot catch a wrong header, because
    /// `Block::header` is a `KeepRaw` and re-emits the bytes it was decoded
    /// from whatever the typed view says. This test re-encodes the header
    /// through its own type, which is the only way the field count is
    /// actually exercised.
    #[test]
    fn header_isomorphic_decoding_encoding() {
        for (name, block_str) in TEST_BLOCKS.iter() {
            let bytes = hex::decode(block_str).unwrap();
            let (_, block): BlockWrapper = minicbor::decode(&bytes).unwrap();

            let raw = block.header.raw_cbor();
            let reencoded = minicbor::to_vec(block.header.clone().unwrap()).unwrap();

            assert_eq!(
                hex::encode(raw),
                hex::encode(&reencoded),
                "{name}: header re-encoded through its own type did not match the wire bytes"
            );
        }
    }

    /// A Dijkstra header body carries twelve fields. Decoding one through
    /// Babbage's ten field type is the silent failure this era module exists
    /// to make impossible, so it is pinned here in both directions: the
    /// Babbage type accepts the bytes and loses the tail, and the Dijkstra
    /// type keeps it.
    #[test]
    fn dijkstra_header_is_not_a_babbage_header() {
        let bytes = hex::decode(TEST_BLOCKS[0].1).unwrap();
        let (_, block): BlockWrapper = minicbor::decode(&bytes).unwrap();
        let raw = block.header.raw_cbor();

        // MUST FIRE: the Babbage type takes these bytes without complaint and
        // gives back a header that is shorter than the one it was handed.
        let as_babbage: crate::babbage::Header =
            minicbor::decode(raw).expect("babbage decoder accepts a dijkstra header");
        let babbage_bytes = minicbor::to_vec(&as_babbage).unwrap();
        assert!(
            babbage_bytes.len() < raw.len(),
            "expected the babbage header type to silently drop the leios fields"
        );

        // MUST NOT FIRE: the Dijkstra type keeps every byte.
        let as_dijkstra: Header = minicbor::decode(raw).unwrap();

        // The tail the Babbage type dropped is exactly the two Leios fields,
        // and no other byte moved. Their width depends on the announcement
        // size, so it is computed from the value rather than hard coded.
        let leios_tail = minicbor::to_vec(as_dijkstra.header_body.block_body_contains_leios_cert)
            .unwrap()
            .len()
            + minicbor::to_vec(&as_dijkstra.header_body.eb_announcement)
                .unwrap()
                .len();
        assert_eq!(
            raw.len() - babbage_bytes.len(),
            leios_tail,
            "the dropped tail should be exactly block_body_contains_leios_cert plus eb_announcement"
        );

        assert_eq!(minicbor::to_vec(&as_dijkstra).unwrap(), raw);
        assert!(as_dijkstra.header_body.block_body_contains_leios_cert);
        assert!(matches!(
            as_dijkstra.header_body.eb_announcement,
            crate::Nullable::Some(_)
        ));
    }

    /// `block_transaction` and `mempool_transaction` are both live on the wire
    /// at once: a ranking block body carries the four element form, and a
    /// mempool submission and an endorser block closure both carry the three
    /// element one. Neither may decode as the other, or a closure would come
    /// back with a validity verdict nobody issued.
    #[test]
    fn a_block_transaction_is_not_a_mempool_transaction() {
        let bytes = hex::decode(TEST_BLOCKS[0].1).unwrap();
        let (_, block): BlockWrapper = minicbor::decode(&bytes).unwrap();

        let tx = block
            .block_body
            .transactions
            .first()
            .expect("the first fixture must carry at least one transaction");

        // MUST NOT FIRE: the four element form round trips as itself.
        let four = minicbor::to_vec(tx).unwrap();
        let back: BlockTransaction = minicbor::decode(&four).unwrap();
        assert_eq!(&back, tx);

        // MUST FIRE: four element bytes are refused by the three element type.
        let as_mempool: Result<MempoolTransaction, _> = minicbor::decode(&four);
        assert!(
            as_mempool.is_err(),
            "a block transaction must not decode as a mempool transaction"
        );

        // MUST NOT FIRE: dropping the flag gives the three element form, and
        // it is three elements shorter on the wire by exactly the flag.
        let three = minicbor::to_vec(tx.to_mempool_transaction()).unwrap();
        let round: MempoolTransaction = minicbor::decode(&three).unwrap();
        assert!(!round.is_valid_supplied);
        assert_eq!(
            four.len() - three.len(),
            minicbor::to_vec(tx.success).unwrap().len(),
            "the two forms should differ by the flag and nothing else"
        );

        // MUST FIRE: three element bytes are refused by the four element type.
        let as_block: Result<BlockTransaction, _> = minicbor::decode(&three);
        assert!(
            as_block.is_err(),
            "a mempool transaction must not decode as a block transaction"
        );
    }

    const DEFS_CDDL: &str = include_str!("defs.cddl");

    /// Every CDDL rule the doc comments in this module name.
    const MODELLED_RULES: &[&str] = &[
        "account_balance_interval",
        "account_balance_intervals",
        "auxiliary_data",
        "auxiliary_data_map",
        "block",
        "block_body",
        "block_transaction",
        "bls_key",
        "bootstrap_witness",
        "certificate",
        "cost_models",
        "direct_deposits",
        "eb_announcement",
        "guards",
        "header",
        "header_body",
        "language",
        "leios_certificate",
        "leios_signature",
        "max_pledge_leverage",
        "mempool_transaction",
        "native_script",
        "nonnegative_interval",
        "peras_certificate",
        "pool_params",
        "protocol_param_update",
        "redeemer_tag",
        "redeemers",
        "required_top_level_guards",
        "script",
        "script_ref",
        "starting_account_balance_intervals",
        "sub_transaction",
        "sub_transaction_body",
        "sub_transactions",
        "transaction_body",
        "transaction_witness_set",
        "vrf_cert",
    ];

    /// Rules the vendored revision does not define, each because the ledger
    /// deleted or renamed it. A type in this module answers to each one, so if
    /// a resync brings one back the model has to change with it.
    const ABSENT_RULES: &[&str] = &[
        // Deleted outright along with the block body's index set.
        "invalid_transactions",
        "transaction_index",
        // Renamed to `block_transaction`.
        "transaction",
        // Renamed to `mempool_transaction`.
        "transaction_mempool",
        // Renamed to `eb_announcement`.
        "leios_announcement",
        // Renamed to `bls_key`.
        "leios_key",
    ];

    /// True when `defs.cddl` defines `name` as a rule of its own, rather than
    /// merely mentioning it or defining something that starts with it.
    fn defines_rule(name: &str) -> bool {
        DEFS_CDDL.lines().any(|line| {
            line.strip_prefix(name).is_some_and(|rest| {
                let rest = rest.trim_start();
                // A plain rule is `name =`, a generic one is `name<a0> =`.
                rest.starts_with('=') || rest.starts_with('<')
            })
        })
    }

    /// A doc comment naming a CDDL rule is only worth writing if it stays true
    /// when the vendored file is resynced. Line numbers went stale on every
    /// insertion and nothing said so, so the citations name rules and this is
    /// what makes the naming load bearing.
    ///
    /// Both directions are checked. A rule this module models must be defined,
    /// and a rule the ledger removed must not be, because a resync that brings
    /// one back is exactly as much of a problem as one that takes another away.
    #[test]
    fn every_cited_rule_matches_the_vendored_cddl() {
        // MUST NOT FIRE: everything the doc comments cite is really in there.
        let missing: Vec<&str> = MODELLED_RULES
            .iter()
            .copied()
            .filter(|name| !defines_rule(name))
            .collect();
        assert!(
            missing.is_empty(),
            "defs.cddl defines no rule for {missing:?}, so a doc comment in this module cites a rule the vendored revision does not have"
        );

        // MUST FIRE if a resync reinstates one: none of these is defined.
        let resurrected: Vec<&str> = ABSENT_RULES
            .iter()
            .copied()
            .filter(|name| defines_rule(name))
            .collect();
        assert!(
            resurrected.is_empty(),
            "defs.cddl defines {resurrected:?}, which this module models as removed or renamed"
        );

        // MUST FIRE: the check can tell a defined rule from an undefined one,
        // so neither list above can pass by the predicate always agreeing.
        assert!(defines_rule("block_body"));
        assert!(!defines_rule("block_bod"));
        assert!(!defines_rule("no_such_rule_exists"));
    }

    /// A Conway (ten field) header body must be refused by the Dijkstra type
    /// rather than accepted with defaults. This is the must-not case that
    /// stops the era detection from being decorative.
    #[test]
    fn conway_header_is_refused_as_dijkstra() {
        let conway = hex::decode(include_str!("../../../test_data/conway1.block")).unwrap();
        let (_, block): (u16, crate::conway::Block) = minicbor::decode(&conway).unwrap();
        let raw = block.header.raw_cbor();

        let decoded: Result<KeepRaw<'_, Header>, _> = minicbor::decode(raw);
        assert!(
            decoded.is_err(),
            "a ten field Conway header must not decode as a Dijkstra header"
        );
    }
}
