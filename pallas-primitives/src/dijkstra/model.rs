//! Ledger primitives and cbor codec for the Dijkstra era
//!
//! Handcrafted, idiomatic rust artifacts based on the [Dijkstra CDDL](https://github.com/IntersectMBO/cardano-ledger/blob/f3104f00f9819ba94de119c38bc3e0109982821f/eras/dijkstra/impl/cddl/data/dijkstra.cddl)
//! file in the IntersectMBO repo, pinned at commit `f3104f0`. A copy of that
//! file sits next to this one as `defs.cddl`, and every rule this module
//! changes relative to Conway cites its line there.
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
    TransactionIndex, TransactionInput, UnitInterval, VrfCert, VrfKeyhash, plutus_data::*,
};

use crate::BTreeMap;

use crate::babbage;

// ----- Header

pub use crate::babbage::OperationalCert;

/// `header_body` (`defs.cddl`) carries twelve fields in Dijkstra where Babbage
/// and Conway carry ten: `leios_certified` and `leios_announcement` are
/// appended.
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
    /// Whether this block certifies the previously announced endorser block.
    #[n(10)]
    pub leios_certified: bool,

    /// Optional announcement of an endorser block.
    #[n(11)]
    pub leios_announcement: Nullable<LeiosAnnouncement>,
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

/// `leios_announcement = [announced_eb : hash32, announced_eb_size : uint .size 4]`
/// (`defs.cddl:99`). An endorser block reference carried in the header body.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct LeiosAnnouncement {
    /// Hash of the announced endorser block.
    #[n(0)]
    pub announced_eb: Hash<32>,

    /// Size in bytes of the announced endorser block.
    #[n(1)]
    pub announced_eb_size: u32,
}

/// `leios_key = [leios_pubkey : bytes .size 96, leios_possessionproof : bytes .size 48]`
/// (`defs.cddl:480`). Carried at position 3 of `pool_params`.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct LeiosKey {
    /// BLS public key.
    #[n(0)]
    pub leios_pubkey: Bytes,

    /// Proof of possession for `leios_pubkey`.
    #[n(1)]
    pub leios_possessionproof: Bytes,
}

/// `leios_signature = bytes .size 48` (`defs.cddl:907`).
pub type LeiosSignature = Bytes;

/// `leios_certificate = [signers : bytes, aggregated_signature : leios_signature]`
/// (`defs.cddl:902`). Present in stock Dijkstra's `block_body`, not only in the
/// Leios fork, so it must be modelled for any Dijkstra block to round-trip.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Eq, Clone)]
pub struct LeiosCertificate {
    /// Bitfield naming the signers.
    #[n(0)]
    pub signers: Bytes,

    /// Aggregated signature over the certified announcement.
    #[n(1)]
    pub aggregated_signature: LeiosSignature,
}

/// `peras_certificate = bytes` (`defs.cddl:909`). Reserved in every Dijkstra
/// ranking block body.
pub type PerasCertificate = Bytes;

// ----- Certificates

/// `certificate` (`defs.cddl`) drops Conway's `account_registration_cert` (0)
/// and `account_unregistration_cert` (1), and `pool_params` gains an optional
/// `leios_key` at position 3 (`defs.cddl:464`), shifting `pledge` onward.
///
/// The `leios_key` slot is three-state on the wire: absent entirely, present
/// and `nil`, or present and populated. All three re-encode differently, so
/// the field is an `Option<Nullable<_>>` and the codec is hand written rather
/// than derived, because a mid-array optional cannot be expressed positionally.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Certificate {
    StakeDelegation(StakeCredential, PoolKeyhash),
    PoolRegistration {
        operator: PoolKeyhash,
        vrf_keyhash: VrfKeyhash,
        leios_key: Option<Nullable<LeiosKey>>,
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

                // `? leios_key : leios_key/ nil` sits between `vrf_keyhash` and
                // `pledge`. `pledge` is a coin, so a uint here means the slot
                // was omitted, and an array or a null means it was supplied.
                let leios_key = match d.datatype()? {
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
                    leios_key,
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
                leios_key,
                pledge,
                cost,
                margin,
                reward_account,
                pool_owners,
                relays,
                pool_metadata,
            } => {
                e.array(if leios_key.is_some() { 11 } else { 10 })?;
                e.encode_with(3u16, ctx)?;
                e.encode_with(operator, ctx)?;
                e.encode_with(vrf_keyhash, ctx)?;
                match leios_key {
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

/// `protocol_param_update` gains keys 34 through 37 (`defs.cddl:709`), the four
/// reference script parameters.
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
/// (`defs.cddl:640`). Replaces Conway's `required_signers` at key 14 and
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

/// `direct_deposits = {+ reward_account => coin}` (`defs.cddl:822`).
pub type DirectDeposits = BTreeMap<RewardAccount, Coin>;

/// `required_top_level_guards = {+ credential => plutus_data/ nil}`
/// (`defs.cddl:820`). Carried by a sub transaction body at key 24.
pub type RequiredTopLevelGuards = BTreeMap<StakeCredential, Nullable<PlutusData>>;

/// `account_balance_intervals = {+ credential => account_balance_interval}`
/// (`defs.cddl:824`).
pub type AccountBalanceIntervals = BTreeMap<StakeCredential, AccountBalanceInterval>;

/// `account_balance_interval` (`defs.cddl:826`) is a two element array whose
/// two arms between them permit a missing lower bound or a missing upper
/// bound, but never both. The three legal combinations are named here so that
/// the fourth cannot be constructed or decoded.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum AccountBalanceInterval {
    /// `[coin, nil]`
    LowerBound(Coin),
    /// `[coin, coin]`
    Bounded(Coin, Coin),
    /// `[nil, coin]`
    UpperBound(Coin),
}

impl<'b, C> minicbor::Decode<'b, C> for AccountBalanceInterval {
    fn decode(d: &mut minicbor::Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
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
        e.array(2)?;

        match self {
            AccountBalanceInterval::LowerBound(l) => {
                e.encode_with(l, ctx)?;
                e.null()?;
            }
            AccountBalanceInterval::Bounded(l, u) => {
                e.encode_with(l, ctx)?;
                e.encode_with(u, ctx)?;
            }
            AccountBalanceInterval::UpperBound(u) => {
                e.null()?;
                e.encode_with(u, ctx)?;
            }
        }

        Ok(())
    }
}

/// `sub_transaction_body` (`defs.cddl:798`), the body of a nested transaction.
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
/// (`defs.cddl:795`).
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
pub struct SubTransaction<'b> {
    #[b(0)]
    pub sub_transaction_body: KeepRaw<'b, SubTransactionBody<'b>>,

    #[n(1)]
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,

    #[n(2)]
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,
}

/// `sub_transactions = nonempty_oset<sub_transaction>` (`defs.cddl:793`).
pub type SubTransactions<'b> = NonEmptySet<SubTransaction<'b>>;

/// `transaction_body` (`defs.cddl`). Relative to Conway: key 14 is `guards`
/// rather than `required_signers`, and keys 23, 25 and 26 are new.
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

    #[n(25)]
    pub direct_deposits: Option<DirectDeposits>,

    #[n(26)]
    pub account_balance_intervals: Option<AccountBalanceIntervals>,
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
/// (`defs.cddl:433`), so the Conway type cannot be reused even though the
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

/// `transaction_witness_set` (`defs.cddl:831`). The rule text is identical to
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

/// `auxiliary_data_map` gains `? 5 : [* plutus_v4_script]` (`defs.cddl:898`).
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

/// `block_body` (`defs.cddl:102`). Dijkstra replaces Conway's segregated
/// witness layout with complete inline transactions, and reserves a Leios and
/// a Peras certificate slot in every ranking block.
#[derive(Serialize, Deserialize, Encode, Decode, Debug, PartialEq, Clone)]
pub struct BlockBody<'b> {
    #[n(0)]
    pub invalid_transactions: Nullable<NonEmptySet<TransactionIndex>>,

    /// The node emits this list as an indefinite length array in 827 of the
    /// 1478 blocks measured off the Musashi immutable database, and as a
    /// definite one in the rest, so the distinction is preserved rather than
    /// normalised.
    #[b(1)]
    pub transactions: MaybeIndefArray<Tx<'b>>,

    #[n(2)]
    pub leios_certificate: Nullable<LeiosCertificate>,

    #[n(3)]
    pub peras_certificate: Nullable<PerasCertificate>,
}

/// `block = [header, block_body]` (`defs.cddl:3`).
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

/// `transaction = [transaction_body, transaction_witness_set, auxiliary_data/ nil]`
/// (`defs.cddl:5`). Conway's `is_valid` bool at position 2 is gone: the flag is
/// stripped when a transaction enters a block, so it cannot appear here.
#[derive(Clone, Serialize, Deserialize, Encode, Decode, Debug, PartialEq)]
pub struct Tx<'b> {
    #[b(0)]
    pub transaction_body: KeepRaw<'b, TransactionBody<'b>>,

    #[n(1)]
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,

    #[n(2)]
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,
}

impl Eq for Tx<'_> {}

/// `transaction_mempool` (`defs.cddl`) permits the Conway four element shape on
/// submission only, with `is_valid` required to be `true`. A transaction read
/// back out of a block is always three elements.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TxMempool<'b> {
    pub transaction_body: KeepRaw<'b, TransactionBody<'b>>,
    pub transaction_witness_set: KeepRaw<'b, WitnessSet<'b>>,
    /// `true` when the submitted encoding carried the deprecated `is_valid`
    /// flag, which is the only value the flag is allowed to take.
    pub is_valid_supplied: bool,
    pub auxiliary_data: Nullable<KeepRaw<'b, AuxiliaryData>>,
}

impl<'b, C> minicbor::Decode<'b, C> for TxMempool<'b> {
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

        Ok(TxMempool {
            transaction_body,
            transaction_witness_set,
            is_valid_supplied,
            auxiliary_data: d.decode_with(ctx)?,
        })
    }
}

impl<C> minicbor::Encode<C> for TxMempool<'_> {
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

impl Eq for TxMempool<'_> {}

#[cfg(test)]
mod tests {
    use super::{Block, Header};
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
        let leios_tail = minicbor::to_vec(as_dijkstra.header_body.leios_certified)
            .unwrap()
            .len()
            + minicbor::to_vec(&as_dijkstra.header_body.leios_announcement)
                .unwrap()
                .len();
        assert_eq!(
            raw.len() - babbage_bytes.len(),
            leios_tail,
            "the dropped tail should be exactly leios_certified plus leios_announcement"
        );

        assert_eq!(minicbor::to_vec(&as_dijkstra).unwrap(), raw);
        assert!(as_dijkstra.header_body.leios_certified);
        assert!(matches!(
            as_dijkstra.header_body.leios_announcement,
            crate::Nullable::Some(_)
        ));
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
