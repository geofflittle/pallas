use std::{collections::BTreeMap, fmt::Debug, ops::Deref};

use super::data::Data;
use super::script_context::{
    GenericScriptInfo, GenericScriptPurpose, GenericTxInfoV1, GenericTxInfoV2, GenericTxInfoV3,
    ScriptContext, TimeRange, TxInInfo, TxInfo, from_alonzo_output,
};
use pallas_addresses::{
    Address, ShelleyDelegationPart, ShelleyPaymentPart, StakeAddress, StakePayload,
};
use pallas_codec::utils::{
    AnyUInt, Bytes, Int, KeepRaw, KeyValuePairs, NonEmptyKeyValuePairs, Nullable, PositiveCoin,
};
use pallas_crypto::hash::Hash;
use pallas_primitives::NonZeroInt;
use pallas_primitives::conway::{
    AssetName, BigInt, Certificate, Coin, Constitution, Constr, CostModel, CostModels, DRep,
    DRepVotingThresholds, DatumOption, ExUnitPrices, ExUnits, GovAction, GovActionId, Mint,
    PlutusData, PolicyId, PoolVotingThresholds, ProposalProcedure, ProtocolParamUpdate,
    RationalNumber, Redeemer, ScriptRef, StakeCredential, TransactionInput, TransactionOutput,
    Value, Vote, Voter, VotingProcedure,
};
#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;
use pallas_traverse::ComputeHash;

pub fn convert_tag_to_constr(tag: u64) -> Option<u64> {
    if (121..=127).contains(&tag) {
        Some(tag - 121)
    } else if (1280..=1400).contains(&tag) {
        Some(tag - 1280 + 7)
    } else {
        None
    }
}

pub fn convert_constr_to_tag(constr: u64) -> Option<u64> {
    if (0..=6).contains(&constr) {
        Some(121 + constr)
    } else if (7..=127).contains(&constr) {
        Some(1280 - 7 + constr)
    } else {
        None // 102 otherwise
    }
}

pub static ANY_TAG: u64 = 102;

pub(super) fn wrap_multiple_with_constr(index: u64, data: Vec<PlutusData>) -> PlutusData {
    let converted = convert_constr_to_tag(index);
    PlutusData::Constr(Constr {
        tag: converted.unwrap_or(ANY_TAG),
        any_constructor: converted.map_or(Some(index), |_| None),
        fields: pallas_codec::utils::MaybeIndefArray::Indef(data),
    })
}

fn wrap_with_constr(index: u64, data: PlutusData) -> PlutusData {
    wrap_multiple_with_constr(index, vec![data])
}

fn empty_constr(index: u64) -> PlutusData {
    wrap_multiple_with_constr(index, vec![])
}

pub(super) struct WithWrappedTransactionId<'a, T>(pub(super) &'a T);

struct WithWrappedStakeCredential<'a, T>(&'a T);

pub(super) struct WithZeroAdaAsset<'a, T>(pub(super) &'a T);

struct WithOptionDatum<'a, T>(&'a T);

struct WithArrayRational<'a, T>(&'a T);

struct WithPartialCertificates<'a, T>(&'a T);

pub(super) struct WithRegistrationDeposit<'a, T>(pub(super) &'a T, pub(super) bool);

pub trait ToPlutusData {
    fn to_plutus_data(&self) -> PlutusData;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MintValue {
    pub mint_value: Mint,
}

impl ToPlutusData for bool {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            false => empty_constr(0),
            true => empty_constr(1),
        }
    }
}
impl ToPlutusData for StakeAddress {
    fn to_plutus_data(&self) -> PlutusData {
        match self.payload() {
            StakePayload::Stake(x) => wrap_with_constr(0, x.to_plutus_data()),
            StakePayload::Script(x) => wrap_with_constr(1, x.to_plutus_data()),
        }
    }
}

impl ToPlutusData for Address {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            Address::Shelley(shelley_address) => {
                let payment_part = shelley_address.payment();
                let stake_part = shelley_address.delegation();

                let payment_part_plutus_data = match payment_part {
                    ShelleyPaymentPart::Key(payment_keyhash) => {
                        wrap_with_constr(0, payment_keyhash.to_plutus_data())
                    }
                    ShelleyPaymentPart::Script(script_hash) => {
                        wrap_with_constr(1, script_hash.to_plutus_data())
                    }
                };

                let stake_part_plutus_data = match stake_part {
                    ShelleyDelegationPart::Key(stake_keyhash) => Some(wrap_with_constr(
                        0,
                        StakeCredential::AddrKeyhash(*stake_keyhash).to_plutus_data(),
                    ))
                    .to_plutus_data(),
                    ShelleyDelegationPart::Script(script_hash) => Some(wrap_with_constr(
                        0,
                        StakeCredential::ScriptHash(*script_hash).to_plutus_data(),
                    ))
                    .to_plutus_data(),
                    ShelleyDelegationPart::Pointer(pointer) => Some(wrap_multiple_with_constr(
                        1,
                        vec![
                            pointer.slot().to_plutus_data(),
                            pointer.tx_idx().to_plutus_data(),
                            pointer.cert_idx().to_plutus_data(),
                        ],
                    ))
                    .to_plutus_data(),
                    ShelleyDelegationPart::Null => None::<StakeCredential>.to_plutus_data(),
                };

                wrap_multiple_with_constr(0, vec![payment_part_plutus_data, stake_part_plutus_data])
            }
            Address::Stake(stake_address) => stake_address.to_plutus_data(),
            _ => unreachable!(),
        }
    }
}

impl ToPlutusData for WithWrappedTransactionId<'_, TransactionInput> {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                wrap_with_constr(0, self.0.transaction_id.to_plutus_data()),
                PlutusData::BigInt(BigInt::Int((self.0.index as i128).try_into().unwrap())),
            ],
        )
    }
}

impl ToPlutusData for TransactionInput {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.transaction_id.to_plutus_data(),
                PlutusData::BigInt(BigInt::Int((self.index as i128).try_into().unwrap())),
            ],
        )
    }
}

impl<const BYTES: usize> ToPlutusData for Hash<BYTES> {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BoundedBytes(self.to_vec().into())
    }
}

impl ToPlutusData for Bytes {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BoundedBytes(self.to_vec().into())
    }
}

impl<K: ToPlutusData, V: ToPlutusData> ToPlutusData for (K, V) {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(0, vec![self.0.to_plutus_data(), self.1.to_plutus_data()])
    }
}

impl<A> ToPlutusData for Vec<A>
where
    A: ToPlutusData,
{
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::Array(pallas_codec::utils::MaybeIndefArray::Def(
            self.iter().map(|p| p.to_plutus_data()).collect(),
        ))
    }
}

impl<K, V> ToPlutusData for KeyValuePairs<K, V>
where
    K: ToPlutusData + Clone,
    V: ToPlutusData + Clone,
{
    fn to_plutus_data(&self) -> PlutusData {
        let mut data_vec: Vec<(PlutusData, PlutusData)> = vec![];
        for (key, value) in self.iter() {
            data_vec.push((key.to_plutus_data(), value.to_plutus_data()))
        }
        PlutusData::Map(KeyValuePairs::Def(data_vec))
    }
}

impl<K, V> ToPlutusData for BTreeMap<K, V>
where
    K: ToPlutusData + Clone,
    V: ToPlutusData + Clone,
{
    fn to_plutus_data(&self) -> PlutusData {
        let mut data_vec: Vec<(PlutusData, PlutusData)> = vec![];
        for (key, value) in self.iter() {
            data_vec.push((key.to_plutus_data(), value.to_plutus_data()))
        }
        PlutusData::Map(KeyValuePairs::Def(data_vec))
    }
}

impl<P: Clone + Debug> ToPlutusData
    for WithWrappedTransactionId<'_, KeyValuePairs<GenericScriptPurpose<P>, Redeemer>>
{
    fn to_plutus_data(&self) -> PlutusData {
        let mut data_vec: Vec<(PlutusData, PlutusData)> = vec![];
        for (key, value) in self.0.iter() {
            data_vec.push((
                WithWrappedTransactionId(key).to_plutus_data(),
                value.to_plutus_data(),
            ))
        }
        PlutusData::Map(KeyValuePairs::Def(data_vec))
    }
}

impl ToPlutusData for WithRegistrationDeposit<'_, Vec<Certificate>> {
    fn to_plutus_data(&self) -> PlutusData {
        self.0
            .iter()
            .map(|cert| WithRegistrationDeposit(cert, self.1))
            .collect::<Vec<_>>()
            .to_plutus_data()
    }
}

impl<P: Clone + ToPlutusData> ToPlutusData
    for WithRegistrationDeposit<'_, KeyValuePairs<GenericScriptPurpose<P>, Redeemer>>
{
    fn to_plutus_data(&self) -> PlutusData {
        let mut data_vec: Vec<(PlutusData, PlutusData)> = vec![];
        for (key, value) in self.0.iter() {
            data_vec.push((
                WithRegistrationDeposit(key, self.1).to_plutus_data(),
                value.to_plutus_data(),
            ))
        }
        PlutusData::Map(KeyValuePairs::Def(data_vec))
    }
}

impl<T: ToPlutusData> ToPlutusData for KeepRaw<'_, T> {
    fn to_plutus_data(&self) -> PlutusData {
        self.deref().to_plutus_data()
    }
}

impl<A: ToPlutusData> ToPlutusData for Option<A> {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            None => empty_constr(1),
            Some(data) => wrap_with_constr(0, data.to_plutus_data()),
        }
    }
}

impl ToPlutusData for Option<KeepRaw<'_, DatumOption<'_>>> {
    // NoOutputDatum = 0 | OutputDatumHash = 1 | OutputDatum = 2
    fn to_plutus_data(&self) -> PlutusData {
        self.clone().map(|x| x.unwrap()).to_plutus_data()
    }
}

impl ToPlutusData for Option<DatumOption<'_>> {
    // NoOutputDatum = 0 | OutputDatumHash = 1 | OutputDatum = 2
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            None => empty_constr(0),
            Some(option) => match option {
                DatumOption::Hash(hash) => wrap_with_constr(1, hash.to_plutus_data()),
                DatumOption::Data(data) => wrap_with_constr(2, data.0.clone().unwrap()),
            },
        }
    }
}

impl ToPlutusData for AnyUInt {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            AnyUInt::U8(n) => PlutusData::BigInt(BigInt::Int(Int::from(*n as i64))),
            AnyUInt::U16(n) => PlutusData::BigInt(BigInt::Int(Int::from(*n as i64))),
            AnyUInt::U32(n) => PlutusData::BigInt(BigInt::Int(Int::from(*n as i64))),
            AnyUInt::U64(n) => PlutusData::BigInt(BigInt::Int(Int::try_from(*n as i128).unwrap())),
            AnyUInt::MajorByte(n) => PlutusData::BigInt(BigInt::Int(Int::from(*n as i64))), /* is this correct? I don't know exactly what is does */
        }
    }
}

impl ToPlutusData for Int {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(BigInt::Int(*self))
    }
}

impl ToPlutusData for BigInt {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(self.clone())
    }
}

impl ToPlutusData for i64 {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(BigInt::Int(Int::from(*self)))
    }
}

impl ToPlutusData for u32 {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(BigInt::Int(Int::try_from(*self as i128).unwrap()))
    }
}

impl ToPlutusData for u64 {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(BigInt::Int(Int::try_from(*self as i128).unwrap()))
    }
}

impl ToPlutusData for usize {
    fn to_plutus_data(&self) -> PlutusData {
        PlutusData::BigInt(BigInt::Int(Int::try_from(*self as i128).unwrap()))
    }
}

impl ToPlutusData for PositiveCoin {
    fn to_plutus_data(&self) -> PlutusData {
        u64::from(self).to_plutus_data()
    }
}

impl ToPlutusData for WithZeroAdaAsset<'_, Value> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            Value::Coin(coin) => {
                PlutusData::Map(KeyValuePairs::Def(vec![coin_to_plutus_data(coin)]))
            }
            Value::Multiasset(coin, multiassets) => {
                let transformed: Vec<(PolicyId, NonEmptyKeyValuePairs<AssetName, PositiveCoin>)> =
                    multiassets
                        .iter()
                        .map(|(policy_id, tokens)| {
                            let kvp = NonEmptyKeyValuePairs::try_from(
                                tokens
                                    .iter()
                                    .map(|(asset_name, amount)| (asset_name.clone(), *amount))
                                    .collect::<Vec<_>>(),
                            )
                            .expect("Empty tokens map");
                            (*policy_id, kvp)
                        })
                        .collect();

                value_to_plutus_data(
                    transformed.iter(),
                    |amount| u64::from(amount).to_plutus_data(),
                    vec![coin_to_plutus_data(coin)],
                )
            }
        }
    }
}

impl ToPlutusData for Value {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            Value::Coin(coin) => PlutusData::Map(KeyValuePairs::Def(if *coin > 0 {
                vec![coin_to_plutus_data(coin)]
            } else {
                vec![]
            })),
            Value::Multiasset(coin, multiassets) => {
                let transformed: Vec<(PolicyId, NonEmptyKeyValuePairs<AssetName, PositiveCoin>)> =
                    multiassets
                        .iter()
                        .map(|(policy_id, tokens)| {
                            let kvp = NonEmptyKeyValuePairs::try_from(
                                tokens
                                    .iter()
                                    .map(|(asset_name, amount)| (asset_name.clone(), *amount))
                                    .collect::<Vec<_>>(),
                            )
                            .expect("Empty tokens map");
                            (*policy_id, kvp)
                        })
                        .collect();

                value_to_plutus_data(
                    transformed.iter(),
                    |amount| u64::from(amount).to_plutus_data(),
                    if *coin > 0 {
                        vec![coin_to_plutus_data(coin)]
                    } else {
                        vec![]
                    },
                )
            }
        }
    }
}

impl ToPlutusData for WithZeroAdaAsset<'_, MintValue> {
    fn to_plutus_data(&self) -> PlutusData {
        let transformed: Vec<(PolicyId, NonEmptyKeyValuePairs<AssetName, NonZeroInt>)> = self
            .0
            .mint_value
            .iter()
            .map(|(policy_id, tokens)| {
                let kvp = NonEmptyKeyValuePairs::try_from(
                    tokens
                        .iter()
                        .map(|(asset_name, amount)| (asset_name.clone(), *amount))
                        .collect::<Vec<_>>(),
                )
                .expect("Empty tokens map");
                (*policy_id, kvp)
            })
            .collect();

        value_to_plutus_data(
            transformed.iter(),
            |amount| i64::from(amount).to_plutus_data(),
            vec![(
                Bytes::from(vec![]).to_plutus_data(),
                PlutusData::Map(KeyValuePairs::Def(vec![(
                    AssetName::from(vec![]).to_plutus_data(),
                    0_i64.to_plutus_data(),
                )])),
            )],
        )
    }
}

impl ToPlutusData for MintValue {
    fn to_plutus_data(&self) -> PlutusData {
        let transformed: Vec<(PolicyId, NonEmptyKeyValuePairs<AssetName, NonZeroInt>)> = self
            .mint_value
            .iter()
            .map(|(policy_id, tokens)| {
                let kvp = NonEmptyKeyValuePairs::try_from(
                    tokens
                        .iter()
                        .map(|(asset_name, amount)| (asset_name.clone(), *amount))
                        .collect::<Vec<_>>(),
                )
                .expect("Empty tokens map");
                (*policy_id, kvp)
            })
            .collect();

        value_to_plutus_data(
            transformed.iter(),
            |amount| i64::from(amount).to_plutus_data(),
            vec![],
        )
    }
}

fn value_to_plutus_data<'a, I, Q>(
    mint: I,
    from_quantity: fn(&'a Q) -> PlutusData,
    mut data_vec: Vec<(PlutusData, PlutusData)>,
) -> PlutusData
where
    I: Iterator<Item = &'a (PolicyId, NonEmptyKeyValuePairs<AssetName, Q>)>,
    Q: Clone,
{
    for (policy_id, assets) in mint {
        let mut assets_vec = vec![];
        for (asset, amount) in assets.iter() {
            assets_vec.push((asset.to_plutus_data(), from_quantity(amount)));
        }
        data_vec.push((
            policy_id.to_plutus_data(),
            PlutusData::Map(KeyValuePairs::Def(assets_vec)),
        ));
    }

    PlutusData::Map(KeyValuePairs::Def(data_vec))
}

fn coin_to_plutus_data(coin: &Coin) -> (PlutusData, PlutusData) {
    (
        Bytes::from(vec![]).to_plutus_data(),
        PlutusData::Map(KeyValuePairs::Def(vec![(
            AssetName::from(vec![]).to_plutus_data(),
            coin.to_plutus_data(),
        )])),
    )
}

impl ToPlutusData for ScriptRef<'_> {
    fn to_plutus_data(&self) -> PlutusData {
        match &self {
            ScriptRef::NativeScript(native_script) => native_script.compute_hash().to_plutus_data(),
            ScriptRef::PlutusV1Script(plutus_v1) => plutus_v1.compute_hash().to_plutus_data(),
            ScriptRef::PlutusV2Script(plutus_v2) => plutus_v2.compute_hash().to_plutus_data(),
            ScriptRef::PlutusV3Script(plutus_v3) => plutus_v3.compute_hash().to_plutus_data(),
        }
    }
}

impl<'a> ToPlutusData for WithOptionDatum<'a, WithZeroAdaAsset<'a, TransactionOutput<'_>>> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0.0 {
            TransactionOutput::Legacy(legacy_output) => {
                WithOptionDatum(&WithZeroAdaAsset(&from_alonzo_output(legacy_output)))
                    .to_plutus_data()
            }

            TransactionOutput::PostAlonzo(post_alonzo_output) => wrap_multiple_with_constr(
                0,
                vec![
                    Address::from_bytes(&post_alonzo_output.address)
                        .unwrap()
                        .to_plutus_data(),
                    WithZeroAdaAsset(&post_alonzo_output.value).to_plutus_data(),
                    match post_alonzo_output.datum_option.clone().map(|x| x.unwrap()) {
                        Some(DatumOption::Hash(hash)) => Some(hash).to_plutus_data(),
                        _ => None::<Hash<32>>.to_plutus_data(),
                    },
                ],
            ),
        }
    }
}

impl ToPlutusData for WithZeroAdaAsset<'_, TransactionOutput<'_>> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            TransactionOutput::Legacy(legacy_output) => {
                WithZeroAdaAsset(&from_alonzo_output(legacy_output)).to_plutus_data()
            }
            TransactionOutput::PostAlonzo(post_alonzo_output) => wrap_multiple_with_constr(
                0,
                vec![
                    Address::from_bytes(&post_alonzo_output.address)
                        .unwrap()
                        .to_plutus_data(),
                    WithZeroAdaAsset(&post_alonzo_output.value).to_plutus_data(),
                    post_alonzo_output
                        .datum_option
                        .clone()
                        .map(|x| x.unwrap())
                        .to_plutus_data(),
                    post_alonzo_output
                        .script_ref
                        .as_ref()
                        .map(|s| s.clone().unwrap())
                        .to_plutus_data(),
                ],
            ),
        }
    }
}

impl ToPlutusData for TransactionOutput<'_> {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            TransactionOutput::Legacy(legacy_output) => {
                from_alonzo_output(legacy_output).to_plutus_data()
            }
            TransactionOutput::PostAlonzo(post_alonzo_output) => wrap_multiple_with_constr(
                0,
                vec![
                    Address::from_bytes(&post_alonzo_output.address)
                        .unwrap()
                        .to_plutus_data(),
                    post_alonzo_output.value.to_plutus_data(),
                    post_alonzo_output.datum_option.to_plutus_data(),
                    post_alonzo_output
                        .script_ref
                        .as_ref()
                        .map(|s| s.clone().unwrap())
                        .to_plutus_data(),
                ],
            ),
        }
    }
}

impl ToPlutusData for StakeCredential {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            StakeCredential::AddrKeyhash(addr_keyhas) => {
                wrap_with_constr(0, addr_keyhas.to_plutus_data())
            }
            StakeCredential::ScriptHash(script_hash) => {
                wrap_with_constr(1, script_hash.to_plutus_data())
            }
        }
    }
}

impl ToPlutusData for WithPartialCertificates<'_, Vec<Certificate>> {
    fn to_plutus_data(&self) -> PlutusData {
        self.0
            .iter()
            .map(WithPartialCertificates)
            .collect::<Vec<_>>()
            .to_plutus_data()
    }
}

impl ToPlutusData for WithPartialCertificates<'_, Certificate> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            Certificate::StakeRegistration(stake_credential)
            | Certificate::Reg(stake_credential, _) => wrap_with_constr(
                0,
                WithWrappedStakeCredential(stake_credential).to_plutus_data(),
            ),

            Certificate::StakeDeregistration(stake_credential)
            | Certificate::UnReg(stake_credential, _) => wrap_with_constr(
                1,
                WithWrappedStakeCredential(stake_credential).to_plutus_data(),
            ),

            Certificate::StakeDelegation(stake_credential, pool_keyhash) => {
                wrap_multiple_with_constr(
                    2,
                    vec![
                        WithWrappedStakeCredential(stake_credential).to_plutus_data(),
                        pool_keyhash.to_plutus_data(),
                    ],
                )
            }

            Certificate::PoolRegistration {
                operator,
                vrf_keyhash,
                pledge: _,
                cost: _,
                margin: _,
                reward_account: _,
                pool_owners: _,
                relays: _,
                pool_metadata: _,
            } => wrap_multiple_with_constr(
                3,
                vec![operator.to_plutus_data(), vrf_keyhash.to_plutus_data()],
            ),

            Certificate::PoolRetirement(pool_keyhash, epoch) => wrap_multiple_with_constr(
                4,
                vec![pool_keyhash.to_plutus_data(), epoch.to_plutus_data()],
            ),

            certificate => {
                unreachable!("unexpected certificate type in V1/V2 script context: {certificate:?}")
            }
        }
    }
}

impl ToPlutusData for WithRegistrationDeposit<'_, Certificate> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            Certificate::StakeRegistration(stake_credential) => wrap_multiple_with_constr(
                0,
                vec![
                    stake_credential.to_plutus_data(),
                    None::<PlutusData>.to_plutus_data(),
                ],
            ),

            Certificate::Reg(stake_credential, amount) => wrap_multiple_with_constr(
                0,
                vec![
                    stake_credential.to_plutus_data(),
                    self.1.then_some(*amount).to_plutus_data(),
                ],
            ),

            Certificate::StakeDeregistration(stake_credential) => wrap_multiple_with_constr(
                1,
                vec![
                    stake_credential.to_plutus_data(),
                    None::<PlutusData>.to_plutus_data(),
                ],
            ),

            Certificate::UnReg(stake_credential, amount) => wrap_multiple_with_constr(
                1,
                vec![
                    stake_credential.to_plutus_data(),
                    self.1.then_some(*amount).to_plutus_data(),
                ],
            ),

            Certificate::StakeDelegation(stake_credential, pool_id) => wrap_multiple_with_constr(
                2,
                vec![
                    stake_credential.to_plutus_data(),
                    wrap_with_constr(0, pool_id.to_plutus_data()),
                ],
            ),

            Certificate::VoteDeleg(stake_credential, drep) => wrap_multiple_with_constr(
                2,
                vec![
                    stake_credential.to_plutus_data(),
                    wrap_with_constr(1, drep.to_plutus_data()),
                ],
            ),

            Certificate::StakeVoteDeleg(stake_credential, pool_id, drep) => {
                wrap_multiple_with_constr(
                    2,
                    vec![
                        stake_credential.to_plutus_data(),
                        wrap_multiple_with_constr(
                            2,
                            vec![pool_id.to_plutus_data(), drep.to_plutus_data()],
                        ),
                    ],
                )
            }

            Certificate::StakeRegDeleg(stake_credential, pool_id, deposit) => {
                wrap_multiple_with_constr(
                    3,
                    vec![
                        stake_credential.to_plutus_data(),
                        wrap_multiple_with_constr(0, vec![pool_id.to_plutus_data()]),
                        deposit.to_plutus_data(),
                    ],
                )
            }

            Certificate::VoteRegDeleg(stake_credential, drep, deposit) => {
                wrap_multiple_with_constr(
                    3,
                    vec![
                        stake_credential.to_plutus_data(),
                        wrap_multiple_with_constr(1, vec![drep.to_plutus_data()]),
                        deposit.to_plutus_data(),
                    ],
                )
            }

            Certificate::StakeVoteRegDeleg(stake_credential, pool_id, drep, deposit) => {
                wrap_multiple_with_constr(
                    3,
                    vec![
                        stake_credential.to_plutus_data(),
                        wrap_multiple_with_constr(
                            2,
                            vec![pool_id.to_plutus_data(), drep.to_plutus_data()],
                        ),
                        deposit.to_plutus_data(),
                    ],
                )
            }

            Certificate::RegDRepCert(drep_credential, deposit, _anchor) => {
                wrap_multiple_with_constr(
                    4,
                    vec![drep_credential.to_plutus_data(), deposit.to_plutus_data()],
                )
            }

            Certificate::UpdateDRepCert(drep_credential, _anchor) => {
                wrap_multiple_with_constr(5, vec![drep_credential.to_plutus_data()])
            }

            Certificate::UnRegDRepCert(drep_credential, deposit) => wrap_multiple_with_constr(
                6,
                vec![drep_credential.to_plutus_data(), deposit.to_plutus_data()],
            ),

            Certificate::PoolRegistration {
                operator,
                vrf_keyhash,
                pledge: _,
                cost: _,
                margin: _,
                reward_account: _,
                pool_owners: _,
                relays: _,
                pool_metadata: _,
            } => wrap_multiple_with_constr(
                7,
                vec![operator.to_plutus_data(), vrf_keyhash.to_plutus_data()],
            ),

            Certificate::PoolRetirement(pool_keyhash, epoch) => wrap_multiple_with_constr(
                8,
                vec![pool_keyhash.to_plutus_data(), epoch.to_plutus_data()],
            ),

            Certificate::AuthCommitteeHot(cold_credential, hot_credential) => {
                wrap_multiple_with_constr(
                    9,
                    vec![
                        cold_credential.to_plutus_data(),
                        hot_credential.to_plutus_data(),
                    ],
                )
            }

            Certificate::ResignCommitteeCold(cold_credential, _anchor) => {
                wrap_multiple_with_constr(10, vec![cold_credential.to_plutus_data()])
            }
        }
    }
}

impl ToPlutusData for DRep {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            DRep::Key(hash) => {
                wrap_with_constr(0, StakeCredential::AddrKeyhash(*hash).to_plutus_data())
            }
            DRep::Script(hash) => {
                wrap_with_constr(0, StakeCredential::ScriptHash(*hash).to_plutus_data())
            }
            DRep::Abstain => empty_constr(1),
            DRep::NoConfidence => empty_constr(2),
        }
    }
}

impl ToPlutusData for Redeemer {
    fn to_plutus_data(&self) -> PlutusData {
        self.data.clone()
    }
}

impl ToPlutusData for PlutusData {
    fn to_plutus_data(&self) -> PlutusData {
        self.clone()
    }
}

impl ToPlutusData for TimeRange {
    fn to_plutus_data(&self) -> PlutusData {
        fn bound(bound: Option<u64>, is_lower: bool) -> PlutusData {
            match bound {
                Some(x) => wrap_multiple_with_constr(
                    0,
                    vec![
                        wrap_with_constr(1, x.to_plutus_data()),
                        // NOTE: Finite lower bounds are always inclusive, unlike upper bounds.
                        is_lower.to_plutus_data(),
                    ],
                ),
                None => wrap_multiple_with_constr(
                    0,
                    vec![
                        empty_constr(if is_lower { 0 } else { 2 }),
                        // NOTE: Infinite bounds are always exclusive, by convention.
                        true.to_plutus_data(),
                    ],
                ),
            }
        }

        wrap_multiple_with_constr(
            0,
            vec![
                bound(self.lower_bound, true),
                bound(self.upper_bound, false),
            ],
        )
    }
}

pub(super) trait LegacyContextData {
    fn to_plutus_data_v1(&self) -> PlutusData;
    fn to_plutus_data_v2(&self) -> PlutusData;
}

impl LegacyContextData for TransactionOutput<'_> {
    fn to_plutus_data_v1(&self) -> PlutusData {
        WithOptionDatum(&WithZeroAdaAsset(self)).to_plutus_data()
    }

    fn to_plutus_data_v2(&self) -> PlutusData {
        WithZeroAdaAsset(self).to_plutus_data()
    }
}

impl LegacyContextData for TxInInfo<'_> {
    fn to_plutus_data_v1(&self) -> PlutusData {
        WithOptionDatum(&WithZeroAdaAsset(&WithWrappedTransactionId(self))).to_plutus_data()
    }

    fn to_plutus_data_v2(&self) -> PlutusData {
        WithZeroAdaAsset(&WithWrappedTransactionId(self)).to_plutus_data()
    }
}

fn legacy_array<A>(items: &[A], encode: impl Fn(&A) -> PlutusData) -> PlutusData {
    PlutusData::Array(pallas_codec::utils::MaybeIndefArray::Def(
        items.iter().map(encode).collect(),
    ))
}

impl<'a> ToPlutusData for WithZeroAdaAsset<'a, WithWrappedTransactionId<'a, TxInInfo<'_>>> {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                WithWrappedTransactionId(&self.0.0.out_ref).to_plutus_data(),
                WithZeroAdaAsset(&self.0.0.resolved).to_plutus_data(),
            ],
        )
    }
}

impl<'a> ToPlutusData
    for WithOptionDatum<'a, WithZeroAdaAsset<'a, WithWrappedTransactionId<'a, TxInInfo<'_>>>>
{
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                WithWrappedTransactionId(&self.0.0.0.out_ref).to_plutus_data(),
                WithOptionDatum(&WithZeroAdaAsset(&self.0.0.0.resolved)).to_plutus_data(),
            ],
        )
    }
}

impl ToPlutusData for TxInInfo<'_> {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.out_ref.to_plutus_data(),
                self.resolved.to_plutus_data(),
            ],
        )
    }
}

// NOTE: This is a _small_ abuse of the 'WithWrappedTransactionId'. We know the
// wrapped is needed for V1 and V2, and it also appears that for V1 and V2, the
// certifying purpose mustn't include the certificate index. So, we also
// short-circuit it here.
impl<P: Debug> ToPlutusData for WithWrappedTransactionId<'_, GenericScriptPurpose<P>> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            GenericScriptPurpose::Minting(policy_id) => {
                wrap_with_constr(0, policy_id.to_plutus_data())
            }
            GenericScriptPurpose::Spending(out_ref, ()) => {
                wrap_with_constr(1, WithWrappedTransactionId(out_ref).to_plutus_data())
            }
            GenericScriptPurpose::Rewarding(stake_credential) => wrap_with_constr(
                2,
                WithWrappedStakeCredential(stake_credential).to_plutus_data(),
            ),
            GenericScriptPurpose::Certifying(_, dcert) => {
                wrap_with_constr(3, WithPartialCertificates(dcert).to_plutus_data())
            }
            purpose => {
                unreachable!("unsupported purpose for V1 or V2 script context: {purpose:?}")
            }
        }
    }
}

impl<P: ToPlutusData> ToPlutusData for WithRegistrationDeposit<'_, GenericScriptPurpose<P>> {
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            GenericScriptPurpose::Minting(policy_id) => {
                wrap_with_constr(0, policy_id.to_plutus_data())
            }
            GenericScriptPurpose::Spending(out_ref, ()) => {
                wrap_with_constr(1, out_ref.to_plutus_data())
            }
            GenericScriptPurpose::Rewarding(stake_credential) => {
                wrap_with_constr(2, stake_credential.to_plutus_data())
            }
            GenericScriptPurpose::Certifying(ix, dcert) => wrap_multiple_with_constr(
                3,
                vec![
                    ix.to_plutus_data(),
                    WithRegistrationDeposit(dcert, self.1).to_plutus_data(),
                ],
            ),
            GenericScriptPurpose::Voting(voter) => {
                wrap_multiple_with_constr(4, vec![voter.to_plutus_data()])
            }
            GenericScriptPurpose::Proposing(ix, procedure) => {
                wrap_multiple_with_constr(5, vec![ix.to_plutus_data(), procedure.to_plutus_data()])
            }
        }
    }
}

impl ToPlutusData for ProposalProcedure {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.deposit.to_plutus_data(),
                Address::from_bytes(&self.reward_account)
                    .unwrap()
                    .to_plutus_data(),
                self.gov_action.to_plutus_data(),
            ],
        )
    }
}

impl<T> ToPlutusData for Nullable<T>
where
    T: ToPlutusData + Clone,
{
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            Nullable::Some(t) => wrap_with_constr(0, t.to_plutus_data()),
            Nullable::Null | Nullable::Undefined => empty_constr(1),
        }
    }
}

impl ToPlutusData for GovActionId {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.transaction_id.to_plutus_data(),
                self.action_index.to_plutus_data(),
            ],
        )
    }
}

fn cost_models_data(
    unknown: &BTreeMap<u64, CostModel>,
    known: &[(u64, &Option<CostModel>)],
) -> PlutusData {
    let mut models = unknown.clone();
    for (key, model) in known {
        if let Some(model) = model {
            models.insert(*key, model.clone());
        }
    }
    models.to_plutus_data()
}

impl ToPlutusData for CostModels {
    fn to_plutus_data(&self) -> PlutusData {
        cost_models_data(
            &self.unknown,
            &[
                (0, &self.plutus_v1),
                (1, &self.plutus_v2),
                (2, &self.plutus_v3),
            ],
        )
    }
}

impl ToPlutusData for ProtocolParamUpdate {
    fn to_plutus_data(&self) -> PlutusData {
        let entries = conway_update_entries(self).into_iter();
        Data::map(entries.map(|(key, p)| (key.to_plutus_data(), p)).collect())
    }
}

fn set_value<T: ToPlutusData>(x: &Option<T>) -> Option<PlutusData> {
    x.as_ref().map(ToPlutusData::to_plutus_data)
}

fn set_rational(x: &Option<RationalNumber>) -> Option<PlutusData> {
    x.as_ref().map(|x| WithArrayRational(x).to_plutus_data())
}

fn conway_update_entries(update: &ProtocolParamUpdate) -> Vec<(u64, PlutusData)> {
    let ProtocolParamUpdate {
        minfee_a,
        minfee_b,
        max_block_body_size,
        max_transaction_size,
        max_block_header_size,
        key_deposit,
        pool_deposit,
        maximum_epoch,
        desired_number_of_stake_pools,
        pool_pledge_influence,
        expansion_rate,
        treasury_growth_rate,
        min_pool_cost,
        ada_per_utxo_byte,
        cost_models_for_script_languages,
        execution_costs,
        max_tx_ex_units,
        max_block_ex_units,
        max_value_size,
        collateral_percentage,
        max_collateral_inputs,
        pool_voting_thresholds,
        drep_voting_thresholds,
        min_committee_size,
        committee_term_limit,
        governance_action_validity_period,
        governance_action_deposit,
        drep_deposit,
        drep_inactivity_period,
        minfee_refscript_cost_per_byte,
    } = update;
    [
        (0, set_value(minfee_a)),
        (1, set_value(minfee_b)),
        (2, set_value(max_block_body_size)),
        (3, set_value(max_transaction_size)),
        (4, set_value(max_block_header_size)),
        (5, set_value(key_deposit)),
        (6, set_value(pool_deposit)),
        (7, set_value(maximum_epoch)),
        (8, set_value(desired_number_of_stake_pools)),
        (9, set_rational(pool_pledge_influence)),
        (10, set_rational(expansion_rate)),
        (11, set_rational(treasury_growth_rate)),
        (16, set_value(min_pool_cost)),
        (17, set_value(ada_per_utxo_byte)),
        (18, set_value(cost_models_for_script_languages)),
        (19, set_value(execution_costs)),
        (20, set_value(max_tx_ex_units)),
        (21, set_value(max_block_ex_units)),
        (22, set_value(max_value_size)),
        (23, set_value(collateral_percentage)),
        (24, set_value(max_collateral_inputs)),
        (25, set_value(pool_voting_thresholds)),
        (26, set_value(drep_voting_thresholds)),
        (27, set_value(min_committee_size)),
        (28, set_value(committee_term_limit)),
        (29, set_value(governance_action_validity_period)),
        (30, set_value(governance_action_deposit)),
        (31, set_value(drep_deposit)),
        (32, set_value(drep_inactivity_period)),
        (33, set_rational(minfee_refscript_cost_per_byte)),
    ]
    .into_iter()
    .filter_map(|(key, x)| Some((key, x?)))
    .collect()
}

#[cfg(feature = "unstable")]
impl ToPlutusData for dijkstra::CostModels {
    fn to_plutus_data(&self) -> PlutusData {
        cost_models_data(
            &self.unknown,
            &[
                (0, &self.plutus_v1),
                (1, &self.plutus_v2),
                (2, &self.plutus_v3),
                (3, &self.plutus_v4),
            ],
        )
    }
}

#[cfg(feature = "unstable")]
impl ToPlutusData for dijkstra::ProtocolParamUpdate {
    fn to_plutus_data(&self) -> PlutusData {
        let dijkstra::ProtocolParamUpdate {
            minfee_a,
            minfee_b,
            max_block_body_size,
            max_transaction_size,
            max_block_header_size,
            key_deposit,
            pool_deposit,
            maximum_epoch,
            desired_number_of_stake_pools,
            pool_pledge_influence,
            expansion_rate,
            treasury_growth_rate,
            min_pool_cost,
            ada_per_utxo_byte,
            cost_models_for_script_languages,
            execution_costs,
            max_tx_ex_units,
            max_block_ex_units,
            max_value_size,
            collateral_percentage,
            max_collateral_inputs,
            pool_voting_thresholds,
            drep_voting_thresholds,
            min_committee_size,
            committee_term_limit,
            governance_action_validity_period,
            governance_action_deposit,
            drep_deposit,
            drep_inactivity_period,
            minfee_refscript_cost_per_byte,
            max_ref_script_size_per_block,
            max_ref_script_size_per_tx,
            ref_script_cost_stride,
            ref_script_cost_multiplier,
            max_pledge_leverage,
            min_pool_margin,
            leios_announcement_period_length,
            leios_vote_period_length,
            leios_diffusion_period_length,
            leios_committee_size,
            leios_quorum_stake_threshold,
            max_endorser_block_references_size,
            max_endorser_block_txs_size,
            max_endorser_block_execution_units,
            max_ref_script_size_per_endorser_block,
            peras_min_candidate_block_age,
            peras_healing_factor,
            peras_cert_boost,
            peras_target_committee_size,
            peras_bootstrap_round,
            peras_quorum_threshold_safety_margin,
            ref_inputs_cost_per_multi_asset_policy,
            ref_inputs_cost_per_datum_byte,
        } = self.clone();
        let conway = ProtocolParamUpdate {
            minfee_a,
            minfee_b,
            max_block_body_size,
            max_transaction_size,
            max_block_header_size,
            key_deposit,
            pool_deposit,
            maximum_epoch,
            desired_number_of_stake_pools,
            pool_pledge_influence,
            expansion_rate,
            treasury_growth_rate,
            min_pool_cost,
            ada_per_utxo_byte,
            cost_models_for_script_languages: None,
            execution_costs,
            max_tx_ex_units,
            max_block_ex_units,
            max_value_size,
            collateral_percentage,
            max_collateral_inputs,
            pool_voting_thresholds,
            drep_voting_thresholds,
            min_committee_size,
            committee_term_limit,
            governance_action_validity_period,
            governance_action_deposit,
            drep_deposit,
            drep_inactivity_period,
            minfee_refscript_cost_per_byte,
        };
        let mut entries = conway_update_entries(&conway);
        if let Some(models) = set_value(&cost_models_for_script_languages) {
            let at = entries.partition_point(|(key, _)| *key < 18);
            entries.insert(at, (18, models));
        }
        let leverage = max_pledge_leverage.map(|x| match x {
            Nullable::Some(x) => wrap_with_constr(0, WithArrayRational(&x).to_plutus_data()),
            Nullable::Null | Nullable::Undefined => empty_constr(1),
        });
        let added = [
            (34, set_value(&max_ref_script_size_per_block)),
            (35, set_value(&max_ref_script_size_per_tx)),
            (36, set_value(&ref_script_cost_stride)),
            (37, set_rational(&ref_script_cost_multiplier)),
            (38, leverage),
            (39, set_rational(&min_pool_margin)),
            (40, set_value(&leios_announcement_period_length)),
            (41, set_value(&leios_vote_period_length)),
            (42, set_value(&leios_diffusion_period_length)),
            (43, set_value(&leios_committee_size)),
            (44, set_rational(&leios_quorum_stake_threshold)),
            (45, set_value(&max_endorser_block_references_size)),
            (46, set_value(&max_endorser_block_txs_size)),
            (47, set_value(&max_endorser_block_execution_units)),
            (48, set_value(&max_ref_script_size_per_endorser_block)),
            (49, set_value(&peras_min_candidate_block_age)),
            (50, set_rational(&peras_healing_factor)),
            (51, set_value(&peras_cert_boost)),
            (52, set_value(&peras_target_committee_size)),
            (53, set_value(&peras_bootstrap_round)),
            (54, set_rational(&peras_quorum_threshold_safety_margin)),
            (55, set_value(&ref_inputs_cost_per_multi_asset_policy)),
            (56, set_value(&ref_inputs_cost_per_datum_byte)),
        ];
        entries.extend(added.into_iter().filter_map(|(key, x)| Some((key, x?))));
        Data::map(
            entries
                .into_iter()
                .map(|(key, x)| (key.to_plutus_data(), x))
                .collect(),
        )
    }
}

#[cfg(feature = "unstable")]
impl ToPlutusData for dijkstra::ProposalProcedure {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.deposit.to_plutus_data(),
                Address::from_bytes(&self.reward_account)
                    .unwrap()
                    .to_plutus_data(),
                self.gov_action.to_plutus_data(),
            ],
        )
    }
}

#[cfg(feature = "unstable")]
impl ToPlutusData for dijkstra::GovAction {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            dijkstra::GovAction::ParameterChange(previous_action, params, guardrail) => {
                wrap_multiple_with_constr(
                    0,
                    vec![
                        previous_action.to_plutus_data(),
                        params.as_ref().to_plutus_data(),
                        guardrail.to_plutus_data(),
                    ],
                )
            }
            dijkstra::GovAction::HardForkInitiation(previous_action, version) => {
                GovAction::HardForkInitiation(previous_action.clone(), *version).to_plutus_data()
            }
            dijkstra::GovAction::TreasuryWithdrawals(withdrawals, guardrail) => {
                GovAction::TreasuryWithdrawals(withdrawals.clone(), *guardrail).to_plutus_data()
            }
            dijkstra::GovAction::NoConfidence(previous_action) => {
                GovAction::NoConfidence(previous_action.clone()).to_plutus_data()
            }
            dijkstra::GovAction::UpdateCommittee(previous_action, removed, added, quorum) => {
                GovAction::UpdateCommittee(
                    previous_action.clone(),
                    removed.to_vec().into(),
                    added.clone(),
                    quorum.clone(),
                )
                .to_plutus_data()
            }
            dijkstra::GovAction::NewConstitution(previous_action, constitution) => {
                GovAction::NewConstitution(previous_action.clone(), constitution.clone())
                    .to_plutus_data()
            }
            dijkstra::GovAction::Information => GovAction::Information.to_plutus_data(),
        }
    }
}

impl ToPlutusData for PoolVotingThresholds {
    fn to_plutus_data(&self) -> PlutusData {
        vec![
            WithArrayRational(&self.motion_no_confidence).to_plutus_data(),
            WithArrayRational(&self.committee_normal).to_plutus_data(),
            WithArrayRational(&self.committee_no_confidence).to_plutus_data(),
            WithArrayRational(&self.hard_fork_initiation).to_plutus_data(),
            WithArrayRational(&self.security_voting_threshold).to_plutus_data(),
        ]
        .to_plutus_data()
    }
}

impl ToPlutusData for DRepVotingThresholds {
    fn to_plutus_data(&self) -> PlutusData {
        vec![
            WithArrayRational(&self.motion_no_confidence).to_plutus_data(),
            WithArrayRational(&self.committee_normal).to_plutus_data(),
            WithArrayRational(&self.committee_no_confidence).to_plutus_data(),
            WithArrayRational(&self.update_constitution).to_plutus_data(),
            WithArrayRational(&self.hard_fork_initiation).to_plutus_data(),
            WithArrayRational(&self.pp_network_group).to_plutus_data(),
            WithArrayRational(&self.pp_economic_group).to_plutus_data(),
            WithArrayRational(&self.pp_technical_group).to_plutus_data(),
            WithArrayRational(&self.pp_governance_group).to_plutus_data(),
            WithArrayRational(&self.treasury_withdrawal).to_plutus_data(),
        ]
        .to_plutus_data()
    }
}

impl ToPlutusData for ExUnitPrices {
    fn to_plutus_data(&self) -> PlutusData {
        vec![
            WithArrayRational(&self.mem_price).to_plutus_data(),
            WithArrayRational(&self.step_price).to_plutus_data(),
        ]
        .to_plutus_data()
    }
}

impl ToPlutusData for ExUnits {
    fn to_plutus_data(&self) -> PlutusData {
        vec![self.mem.to_plutus_data(), self.steps.to_plutus_data()].to_plutus_data()
    }
}

impl ToPlutusData for GovAction {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            GovAction::ParameterChange(previous_action, params, guardrail) => {
                wrap_multiple_with_constr(
                    0,
                    vec![
                        previous_action.to_plutus_data(),
                        params.as_ref().to_plutus_data(),
                        guardrail.to_plutus_data(),
                    ],
                )
            }
            GovAction::HardForkInitiation(previous_action, version) => wrap_multiple_with_constr(
                1,
                vec![previous_action.to_plutus_data(), version.to_plutus_data()],
            ),
            GovAction::TreasuryWithdrawals(withdrawals, guardrail) => wrap_multiple_with_constr(
                2,
                vec![
                    KeyValuePairs::from(
                        withdrawals
                            .iter()
                            .map(|(reward_account, amount)| {
                                (
                                    Address::from_bytes(reward_account)
                                        .expect("Invalid stake address in treasury withdrawal?"),
                                    *amount,
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .to_plutus_data(),
                    guardrail.to_plutus_data(),
                ],
            ),
            GovAction::NoConfidence(previous_action) => {
                wrap_with_constr(3, previous_action.to_plutus_data())
            }
            GovAction::UpdateCommittee(previous_action, removed, added, quorum) => {
                wrap_multiple_with_constr(
                    4,
                    vec![
                        previous_action.to_plutus_data(),
                        removed.to_plutus_data(),
                        added.to_plutus_data(),
                        quorum.to_plutus_data(),
                    ],
                )
            }
            GovAction::NewConstitution(previous_action, constitution) => wrap_multiple_with_constr(
                5,
                vec![
                    previous_action.to_plutus_data(),
                    constitution.to_plutus_data(),
                ],
            ),
            GovAction::Information => empty_constr(6),
        }
    }
}

impl ToPlutusData for Constitution {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_with_constr(0, self.guardrail_script.to_plutus_data())
    }
}

impl ToPlutusData for RationalNumber {
    fn to_plutus_data(&self) -> PlutusData {
        (self.numerator, self.denominator).to_plutus_data()
    }
}

impl ToPlutusData for WithArrayRational<'_, RationalNumber> {
    fn to_plutus_data(&self) -> PlutusData {
        vec![self.0.numerator, self.0.denominator].to_plutus_data()
    }
}

impl ToPlutusData for WithWrappedStakeCredential<'_, Vec<(Address, Coin)>> {
    fn to_plutus_data(&self) -> PlutusData {
        self.0
            .iter()
            .map(|(reward_account, amount)| {
                (
                    wrap_with_constr(0, reward_account.to_plutus_data()),
                    *amount,
                )
            })
            .collect::<Vec<_>>()
            .to_plutus_data()
    }
}

impl ToPlutusData for WithWrappedStakeCredential<'_, KeyValuePairs<Address, Coin>> {
    fn to_plutus_data(&self) -> PlutusData {
        KeyValuePairs::from(
            self.0
                .iter()
                .map(|(reward_account, amount)| {
                    (
                        wrap_with_constr(0, reward_account.to_plutus_data()),
                        *amount,
                    )
                })
                .collect::<Vec<_>>(),
        )
        .to_plutus_data()
    }
}

impl ToPlutusData for WithWrappedStakeCredential<'_, StakeCredential> {
    fn to_plutus_data(&self) -> PlutusData {
        wrap_with_constr(0, self.0.to_plutus_data())
    }
}

impl ToPlutusData for Voter {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            Voter::ConstitutionalCommitteeScript(hash) => {
                wrap_with_constr(0, StakeCredential::ScriptHash(*hash).to_plutus_data())
            }
            Voter::ConstitutionalCommitteeKey(hash) => {
                wrap_with_constr(0, StakeCredential::AddrKeyhash(*hash).to_plutus_data())
            }
            Voter::DRepScript(hash) => {
                wrap_with_constr(1, StakeCredential::ScriptHash(*hash).to_plutus_data())
            }
            Voter::DRepKey(hash) => {
                wrap_with_constr(1, StakeCredential::AddrKeyhash(*hash).to_plutus_data())
            }
            Voter::StakePoolKey(hash) => wrap_with_constr(2, hash.to_plutus_data()),
        }
    }
}

impl ToPlutusData for VotingProcedure {
    fn to_plutus_data(&self) -> PlutusData {
        self.vote.to_plutus_data()
    }
}

impl ToPlutusData for Vote {
    fn to_plutus_data(&self) -> PlutusData {
        match self {
            Vote::No => empty_constr(0),
            Vote::Yes => empty_constr(1),
            Vote::Abstain => empty_constr(2),
        }
    }
}

impl<T, P> ToPlutusData for WithRegistrationDeposit<'_, GenericScriptInfo<T, P>>
where
    T: ToPlutusData,
    P: ToPlutusData,
{
    fn to_plutus_data(&self) -> PlutusData {
        match self.0 {
            GenericScriptInfo::Minting(policy_id) => {
                wrap_with_constr(0, policy_id.to_plutus_data())
            }
            GenericScriptInfo::Spending(out_ref, datum) => {
                wrap_multiple_with_constr(1, vec![out_ref.to_plutus_data(), datum.to_plutus_data()])
            }
            GenericScriptInfo::Rewarding(stake_credential) => {
                wrap_with_constr(2, stake_credential.to_plutus_data())
            }
            GenericScriptInfo::Certifying(ix, dcert) => wrap_multiple_with_constr(
                3,
                vec![
                    ix.to_plutus_data(),
                    WithRegistrationDeposit(dcert, self.1).to_plutus_data(),
                ],
            ),
            GenericScriptInfo::Voting(voter) => {
                wrap_multiple_with_constr(4, vec![voter.to_plutus_data()])
            }
            GenericScriptInfo::Proposing(ix, procedure) => {
                wrap_multiple_with_constr(5, vec![ix.to_plutus_data(), procedure.to_plutus_data()])
            }
        }
    }
}

impl ToPlutusData for TxInfo<'_> {
    fn to_plutus_data(&self) -> PlutusData {
        self.to_plutus_data_with_protocol(9)
    }
}

impl TxInfo<'_> {
    /// The transaction info data under protocol major version `protocol_major`.
    pub fn to_plutus_data_with_protocol(&self, protocol_major: u32) -> PlutusData {
        self.clone().into_plutus_data(protocol_major)
    }

    fn into_plutus_data(self, protocol_major: u32) -> PlutusData {
        match self {
            TxInfo::V1(tx_info) => GenericTxInfoV1::from(tx_info).to_plutus_data(),
            TxInfo::V2(tx_info) => GenericTxInfoV2::from(tx_info).to_plutus_data(),
            TxInfo::V3(tx_info) => {
                GenericTxInfoV3::from(tx_info).to_plutus_data_with_protocol(protocol_major)
            }
        }
    }
}

impl<I, O, P: Clone> GenericTxInfoV1<I, O, P> {
    pub(super) fn to_plutus_data(&self) -> PlutusData
    where
        I: LegacyContextData,
        O: LegacyContextData,
    {
        wrap_multiple_with_constr(
            0,
            vec![
                legacy_array(&self.inputs, I::to_plutus_data_v1),
                legacy_array(&self.outputs, O::to_plutus_data_v1),
                WithZeroAdaAsset(&self.fee).to_plutus_data(),
                WithZeroAdaAsset(&self.mint).to_plutus_data(),
                WithPartialCertificates(&self.certificates).to_plutus_data(),
                WithWrappedStakeCredential(&self.withdrawals).to_plutus_data(),
                self.valid_range.to_plutus_data(),
                self.signatories.to_plutus_data(),
                self.data.to_plutus_data(),
                wrap_with_constr(0, self.id.to_plutus_data()),
            ],
        )
    }
}

impl<I, O, P: Clone + Debug> GenericTxInfoV2<I, O, P> {
    pub(super) fn to_plutus_data(&self) -> PlutusData
    where
        I: LegacyContextData,
        O: LegacyContextData,
    {
        wrap_multiple_with_constr(
            0,
            vec![
                legacy_array(&self.inputs, I::to_plutus_data_v2),
                legacy_array(&self.reference_inputs, I::to_plutus_data_v2),
                legacy_array(&self.outputs, O::to_plutus_data_v2),
                WithZeroAdaAsset(&self.fee).to_plutus_data(),
                WithZeroAdaAsset(&self.mint).to_plutus_data(),
                WithPartialCertificates(&self.certificates).to_plutus_data(),
                WithWrappedStakeCredential(&self.withdrawals).to_plutus_data(),
                self.valid_range.to_plutus_data(),
                self.signatories.to_plutus_data(),
                WithWrappedTransactionId(&self.redeemers).to_plutus_data(),
                self.data.to_plutus_data(),
                wrap_with_constr(0, self.id.to_plutus_data()),
            ],
        )
    }
}

impl<I: ToPlutusData, O: ToPlutusData, P: Clone + ToPlutusData> GenericTxInfoV3<I, O, P> {
    pub(super) fn to_plutus_data_with_protocol(&self, protocol_major: u32) -> PlutusData {
        wrap_multiple_with_constr(
            0,
            vec![
                self.inputs.to_plutus_data(),
                self.reference_inputs.to_plutus_data(),
                self.outputs.to_plutus_data(),
                self.fee.to_plutus_data(),
                self.mint.to_plutus_data(),
                WithRegistrationDeposit(&self.certificates, protocol_major > 9).to_plutus_data(),
                self.withdrawals.to_plutus_data(),
                self.valid_range.to_plutus_data(),
                self.signatories.to_plutus_data(),
                WithRegistrationDeposit(&self.redeemers, protocol_major > 9).to_plutus_data(),
                self.data.to_plutus_data(),
                self.id.to_plutus_data(),
                self.votes.to_plutus_data(),
                self.proposal_procedures.to_plutus_data(),
                self.current_treasury_amount.to_plutus_data(),
                self.treasury_donation.to_plutus_data(),
            ],
        )
    }
}

pub(super) fn v3_script_context_data<P: ToPlutusData>(
    tx_info: PlutusData,
    redeemer: &PlutusData,
    purpose: &GenericScriptInfo<Option<PlutusData>, P>,
    protocol_major: u32,
) -> PlutusData {
    wrap_multiple_with_constr(
        0,
        vec![
            tx_info,
            redeemer.to_plutus_data(),
            WithRegistrationDeposit(purpose, protocol_major > 9).to_plutus_data(),
        ],
    )
}

pub(super) fn v1v2_script_context_data<P: Debug>(
    tx_info: PlutusData,
    purpose: &GenericScriptPurpose<P>,
) -> PlutusData {
    wrap_multiple_with_constr(
        0,
        vec![tx_info, WithWrappedTransactionId(purpose).to_plutus_data()],
    )
}

impl ToPlutusData for ScriptContext<'_> {
    fn to_plutus_data(&self) -> PlutusData {
        self.to_plutus_data_with_protocol(9)
    }
}

impl ScriptContext<'_> {
    /// The script context data under protocol major version `protocol_major`.
    pub fn to_plutus_data_with_protocol(&self, protocol_major: u32) -> PlutusData {
        self.clone().into_plutus_data(protocol_major)
    }

    pub(super) fn into_plutus_data(self, protocol_major: u32) -> PlutusData {
        match self {
            ScriptContext::V1V2 { tx_info, purpose } => v1v2_script_context_data(
                tx_info.into_plutus_data(protocol_major),
                &GenericScriptPurpose::from(*purpose),
            ),
            ScriptContext::V3 {
                tx_info,
                redeemer,
                purpose,
            } => v3_script_context_data(
                tx_info.into_plutus_data(protocol_major),
                &redeemer,
                &GenericScriptInfo::from(*purpose),
                protocol_major,
            ),
        }
    }
}
