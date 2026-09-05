use num_rational::Rational64;
use pallas_crypto::hash::Hash;
use pallas_primitives::conway::{Epoch, RationalNumber};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, str::FromStr};

fn deserialize_rational<'de, D>(
    deserializer: D,
) -> Result<pallas_primitives::alonzo::RationalNumber, D::Error>
where
    D: Deserializer<'de>,
{
    let s = f32::deserialize(deserializer)?;
    let r = Rational64::approximate_float(s)
        .ok_or(serde::de::Error::custom("can't turn float into rational"))?;

    let r = pallas_primitives::alonzo::RationalNumber {
        numerator: *r.numer() as u64,
        denominator: *r.denom() as u64,
    };

    Ok(r)
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenDelegs {
    pub delegate: Option<String>,
    pub vrf: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVersion {
    pub minor: u64,
    pub major: u64,
}

impl From<ProtocolVersion> for pallas_primitives::alonzo::ProtocolVersion {
    fn from(value: ProtocolVersion) -> Self {
        (value.major, value.minor)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum NonceVariant {
    NeutralNonce,
    Nonce,
}

impl From<NonceVariant> for pallas_primitives::alonzo::NonceVariant {
    fn from(value: NonceVariant) -> Self {
        match value {
            NonceVariant::NeutralNonce => Self::NeutralNonce,
            NonceVariant::Nonce => Self::Nonce,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtraEntropy {
    pub tag: NonceVariant,
    pub hash: Option<String>,
}

impl From<ExtraEntropy> for pallas_primitives::alonzo::Nonce {
    fn from(value: ExtraEntropy) -> Self {
        Self {
            variant: value.tag.into(),
            hash: value
                .hash
                .map(|x| Hash::<32>::from_str(&x).expect("invalid nonce hash value")),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolParams {
    pub protocol_version: ProtocolVersion,
    pub max_tx_size: u32,
    pub max_block_body_size: u32,
    pub max_block_header_size: u32,
    pub key_deposit: u64,
    #[serde(rename = "minUTxOValue")]
    pub min_utxo_value: u64,
    pub min_fee_a: u32,
    pub min_fee_b: u32,
    pub pool_deposit: u64,
    pub n_opt: u32,
    pub min_pool_cost: u64,
    pub e_max: Epoch,
    pub extra_entropy: ExtraEntropy,

    #[serde(deserialize_with = "deserialize_rational")]
    pub decentralisation_param: RationalNumber,

    #[serde(deserialize_with = "deserialize_rational")]
    pub rho: pallas_primitives::alonzo::RationalNumber,

    #[serde(deserialize_with = "deserialize_rational")]
    pub tau: pallas_primitives::alonzo::RationalNumber,

    #[serde(deserialize_with = "deserialize_rational")]
    pub a0: pallas_primitives::alonzo::RationalNumber,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub hash: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SingleHostAddr {
    pub port: Option<u32>,
    #[serde(rename = "IPv6")]
    pub ipv6: Option<String>,
    #[serde(rename = "IPv4")]
    pub ipv4: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SingleHostName {
    pub port: Option<u32>,
    pub dns_name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MultiHostName {
    pub dns_name: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Relay {
    SingleHostAddr(SingleHostAddr),
    SingleHostName(SingleHostName),
    MultiHostName(MultiHostName),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Credential {
    KeyHash(String),
    ScriptHash(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RewardAccount {
    pub credential: Credential,
    pub network: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
    pub cost: u64,
    #[serde(deserialize_with = "deserialize_rational")]
    pub margin: pallas_primitives::alonzo::RationalNumber,
    pub metadata: Option<Metadata>,
    #[serde(default)]
    pub owners: Vec<String>,
    pub pledge: u64,
    // The ledger writes this as `poolId` and reads either that or the older
    // `publicKey`, so both spellings have to be accepted.
    #[serde(alias = "poolId")]
    pub public_key: String, // pool ID
    pub relays: Vec<HashMap<String, Relay>>,
    // Same pairing as `public_key`. The ledger writes `accountAddress` and
    // still reads `rewardAccount`.
    #[serde(alias = "accountAddress")]
    pub reward_account: RewardAccount,
    pub vrf: String,
    #[serde(default)]
    pub registration_deposit: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Staking {
    pub pools: Option<HashMap<String, Pool>>,
    pub stake: Option<HashMap<String, String>>,
}

/// One injection source from the genesis `extraConfig` block.
///
/// The ledger models this as three arms: no injection, a payload written
/// inline, or a payload in a separate file named alongside its hash. The three
/// are kept apart here because folding the file arm into the empty one
/// produces a genesis that loads cleanly and describes a ledger that does not
/// exist.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Injection<T> {
    /// The payload is written inline under `data`.
    Embedded { data: T },

    /// The payload lives in a separate file, given as path segments together
    /// with the hash the file must have.
    FromFile { file: Vec<String>, hash: String },

    /// The key is present and names no injection at all.
    Absent {},
}

impl<T> Injection<T> {
    /// The inline payload, if there is one.
    ///
    /// The file arm is an error rather than `None`, because the caller merges
    /// this into the genesis ledger and cannot tell an absent injection from
    /// one it failed to read.
    pub fn payload(self, field: &str) -> Result<Option<T>, String> {
        match self {
            Self::Embedded { data } => Ok(Some(data)),
            Self::Absent {} => Ok(None),
            Self::FromFile { file, .. } => Err(format!(
                "extraConfig.{field} names an injection file ({}), which is not resolvable here",
                file.join("/")
            )),
        }
    }
}

/// Genesis data injected alongside the top-level fields.
///
/// A node writes the funds, pools and delegations it was seeded with here
/// rather than at the top level, so a reader that consults only the top level
/// sees an empty chain and reports no error.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtraConfig {
    pub initial_funds: Option<Injection<HashMap<String, u64>>>,
    pub stake_pools: Option<Injection<HashMap<String, Pool>>>,
    pub stake_credentials: Option<Injection<HashMap<String, String>>>,
}

/// Merge injected entries into a top-level map.
///
/// A key present on both sides is an error. The two sides describe the same
/// chain, so a disagreement is a genesis that means two different things and
/// picking one of them would hide that.
fn merge_injected<T>(
    base: &mut Option<HashMap<String, T>>,
    injected: Option<HashMap<String, T>>,
    field: &str,
) -> Result<(), String> {
    let Some(injected) = injected else {
        return Ok(());
    };

    let target = base.get_or_insert_with(HashMap::new);

    for (key, value) in injected {
        if target.contains_key(&key) {
            return Err(format!(
                "extraConfig.{field} injects {key}, which the top-level genesis already declares"
            ));
        }

        target.insert(key, value);
    }

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenesisFileRaw {
    active_slots_coeff: Option<f32>,
    epoch_length: Option<u32>,
    gen_delegs: Option<HashMap<String, GenDelegs>>,
    initial_funds: Option<HashMap<String, u64>>,
    max_lovelace_supply: Option<u64>,
    network_id: Option<String>,
    network_magic: Option<u32>,
    protocol_params: ProtocolParams,
    security_param: Option<u32>,
    slot_length: Option<u32>,
    staking: Option<Staking>,
    system_start: Option<String>,
    update_quorum: Option<u32>,
    extra_config: Option<ExtraConfig>,

    #[serde(rename = "maxKESEvolutions")]
    max_kes_evolutions: Option<u32>,

    #[serde(rename = "slotsPerKESPeriod")]
    slots_per_kes_period: Option<u32>,
}

impl TryFrom<GenesisFileRaw> for GenesisFile {
    type Error = String;

    fn try_from(raw: GenesisFileRaw) -> Result<Self, Self::Error> {
        let mut initial_funds = raw.initial_funds;
        let mut staking = raw.staking;

        if let Some(extra) = raw.extra_config {
            let funds = extra
                .initial_funds
                .map(|x| x.payload("initialFunds"))
                .transpose()?
                .flatten();

            merge_injected(&mut initial_funds, funds, "initialFunds")?;

            let pools = extra
                .stake_pools
                .map(|x| x.payload("stakePools"))
                .transpose()?
                .flatten();

            let credentials = extra
                .stake_credentials
                .map(|x| x.payload("stakeCredentials"))
                .transpose()?
                .flatten();

            if pools.is_some() || credentials.is_some() {
                let staking = staking.get_or_insert(Staking {
                    pools: None,
                    stake: None,
                });

                merge_injected(&mut staking.pools, pools, "stakePools")?;
                merge_injected(&mut staking.stake, credentials, "stakeCredentials")?;
            }
        }

        Ok(GenesisFile {
            active_slots_coeff: raw.active_slots_coeff,
            epoch_length: raw.epoch_length,
            gen_delegs: raw.gen_delegs,
            initial_funds,
            max_lovelace_supply: raw.max_lovelace_supply,
            network_id: raw.network_id,
            network_magic: raw.network_magic,
            protocol_params: raw.protocol_params,
            security_param: raw.security_param,
            slot_length: raw.slot_length,
            staking,
            system_start: raw.system_start,
            update_quorum: raw.update_quorum,
            max_kes_evolutions: raw.max_kes_evolutions,
            slots_per_kes_period: raw.slots_per_kes_period,
        })
    }
}

/// A parsed Shelley genesis file.
///
/// Anything the file carried under `extraConfig` has already been folded into
/// `initial_funds` and `staking` by the time a value of this type exists, so
/// every reader sees one set of funds and one set of pools regardless of which
/// half of the file they were written in.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", try_from = "GenesisFileRaw")]
pub struct GenesisFile {
    pub active_slots_coeff: Option<f32>,
    pub epoch_length: Option<u32>,
    pub gen_delegs: Option<HashMap<String, GenDelegs>>,
    pub initial_funds: Option<HashMap<String, u64>>,
    pub max_lovelace_supply: Option<u64>,
    pub network_id: Option<String>,
    pub network_magic: Option<u32>,
    pub protocol_params: ProtocolParams,
    pub security_param: Option<u32>,
    pub slot_length: Option<u32>,
    pub staking: Option<Staking>,
    pub system_start: Option<String>,
    pub update_quorum: Option<u32>,

    #[serde(rename = "maxKESEvolutions")]
    pub max_kes_evolutions: Option<u32>,

    #[serde(rename = "slotsPerKESPeriod")]
    pub slots_per_kes_period: Option<u32>,
}

pub fn from_file(path: &std::path::Path) -> Result<GenesisFile, std::io::Error> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let parsed: GenesisFile = serde_json::from_reader(reader)?;

    Ok(parsed)
}

pub type GenesisUtxo = (Hash<32>, pallas_addresses::Address, u64);

pub fn shelley_utxos(config: &GenesisFile) -> Vec<GenesisUtxo> {
    match &config.initial_funds {
        None => Vec::new(),
        Some(funds) => funds
            .iter()
            .map(|(addr, amount)| {
                let addr = pallas_addresses::Address::from_hex(addr).unwrap();

                let txid = pallas_crypto::hash::Hasher::<256>::hash(&addr.to_vec());

                (txid, addr, *amount)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_data_config(network: &str) -> GenesisFile {
        let path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join("test_data")
            .join(format!("{network}-shelley-genesis.json"));

        from_file(&path).unwrap()
    }

    #[test]
    fn calc_address_txid() {
        let config = load_test_data_config("golden");
        let utxos = shelley_utxos(&config);
        let utxo = utxos.first().unwrap();
        assert_eq!(
            utxo.0.to_string(),
            "f9ec23569778d1c5f7f43e0e98464335f02fb98b57683faa1c6b18c82921d2da"
        );
        assert_eq!(
            utxo.1.to_bech32().unwrap(),
            "addr_test1qrsm4h32h9r95f8at64ykuugxqu3wvu0s5ay3vg6tlyevjh4e2flkegka00r69gt8c4vkxgf2vnnph3nsvhlkg5ukgxslee3tf"
        );
        assert_eq!(utxo.2, 12157196);
    }

    #[test]
    fn test_preview_json_loads() {
        load_test_data_config("preview");
    }

    #[test]
    fn test_mainnet_json_loads() {
        load_test_data_config("mainnet");
    }

    #[test]
    fn test_partner_staking_parses() {
        let config = load_test_data_config("partner");

        let staking = config
            .staking
            .expect("staking section must be present in partner genesis");

        let pools = staking
            .pools
            .expect("staking pools should be available in partner genesis");
        assert_eq!(pools.len(), 3);

        let pool = pools
            .get("2d0de269b0996fdcd8f19f0b6d7d0bf14363984482f181a5a1ccd036")
            .expect("expected initial pool registration");

        assert_eq!(pool.cost, 0);
        assert_eq!(pool.pledge, 0);
        assert_eq!(pool.owners.len(), 0);
        assert_eq!(pool.relays.len(), 0);
        assert_eq!(pool.reward_account.network, "Testnet");
        match &pool.reward_account.credential {
            Credential::KeyHash(key) => assert_eq!(
                key,
                "f0834f87e577f514cecdd41db9feabcc42671b4b15cd5a27924e571c"
            ),
            _ => panic!("expected key hash credential"),
        }
        assert_eq!(
            pool.vrf,
            "72255c577e9fa3146e397ffeb45a187c48505a5f950216d7b1224d85dc4fbbac"
        );
        assert_eq!(pool.margin.numerator, 0);

        let stake = staking
            .stake
            .expect("delegation map should exist in partner genesis");
        assert_eq!(stake.len(), 3);
        assert_eq!(
            stake
                .get("f441c3ef7ef4a8ade039f4f4224b9ae494125bbcff284df64e8e73d8")
                .expect("stake delegation should exist"),
            "2d0de269b0996fdcd8f19f0b6d7d0bf14363984482f181a5a1ccd036"
        );
    }

    /// The Musashi genesis carries its funds under `extraConfig`, which is
    /// where a current node writes injected genesis data. Reading only the
    /// top-level `initialFunds` yields an empty ledger with no error.
    #[test]
    fn extra_config_initial_funds_reach_the_utxo_set() {
        let config = load_test_data_config("musashi");
        let utxos = shelley_utxos(&config);

        assert_eq!(utxos.len(), 2, "both injected funds must be present");

        let faucet = utxos
            .iter()
            .find(|(_, _, amount)| *amount == 30_000_000_000_000_000)
            .expect("the faucet fund must be present");

        assert_eq!(
            faucet.0.to_string(),
            "3c7a80572fd84ea2ed76de087f33f92146e54cf7a744530ed35ef411d423b0b4"
        );

        let delegator = utxos
            .iter()
            .find(|(_, _, amount)| *amount == 900_000_000)
            .expect("the delegator fund must be present");

        assert_eq!(
            delegator.0.to_string(),
            "e2413ab289b620ec02932da2463cb4edd8cf3146f7f3eb788fafceb8fc10d9ae"
        );
    }

    /// The pool and delegation injected through `extraConfig` must be visible
    /// through the same `staking` accessor as a genesis that declares them at
    /// the top level, and the pool must parse under the names the ledger
    /// writes today (`poolId`, `accountAddress`) rather than only under the
    /// older `publicKey` and `rewardAccount`.
    #[test]
    fn extra_config_staking_reaches_the_staking_accessor() {
        let config = load_test_data_config("musashi");

        let staking = config.staking.expect("staking must be present");
        let pools = staking.pools.expect("pools must be present");

        assert_eq!(pools.len(), 1);

        let pool = pools
            .get("747aca09f322d2dfc56243b839e2d573ab92287684e5e37d66ec0f87")
            .expect("the injected pool must be present");

        assert_eq!(
            pool.public_key,
            "747aca09f322d2dfc56243b839e2d573ab92287684e5e37d66ec0f87"
        );
        assert_eq!(
            pool.vrf,
            "d8252bd637a90ba4dbd2cf63afda20a19888b7895ede067081ce7fb7411a972b"
        );
        assert_eq!(pool.reward_account.network, "Testnet");
        match &pool.reward_account.credential {
            Credential::KeyHash(key) => assert_eq!(
                key,
                "bc2b888f42c68e4e118a4fa16ada52db896bffb61ceea51a58301bf6"
            ),
            _ => panic!("expected a key hash credential"),
        }

        let stake = staking.stake.expect("delegations must be present");
        assert_eq!(stake.len(), 1);
        assert_eq!(
            stake
                .get("5e81366cb6f3c0d14837614afcea669d51b8be9519eaec4a237504f8")
                .expect("the injected delegation must be present"),
            "747aca09f322d2dfc56243b839e2d573ab92287684e5e37d66ec0f87"
        );
    }

    /// The must-not case for the two above. A genesis that never mentions
    /// `extraConfig` must come out exactly as it did before, with nothing
    /// added and nothing counted twice.
    #[test]
    fn a_genesis_without_extra_config_is_untouched() {
        let golden = load_test_data_config("golden");
        assert_eq!(shelley_utxos(&golden).len(), 1);

        let partner = load_test_data_config("partner");
        assert_eq!(shelley_utxos(&partner).len(), 4);

        let staking = partner.staking.expect("partner staking");
        assert_eq!(staking.pools.expect("partner pools").len(), 3);
        assert_eq!(staking.stake.expect("partner stake").len(), 3);
    }

    /// The injection can also name an external file. Nothing here can resolve
    /// one, and treating it as no injection would produce the same empty
    /// ledger this whole change exists to stop, so it has to be an error that
    /// says which arm it saw.
    #[test]
    fn a_file_injection_is_refused_rather_than_read_as_empty() {
        // Built from the real Musashi genesis so that every other field is
        // valid and the file arm is the only thing under test.
        let path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join("test_data")
            .join("musashi-shelley-genesis.json");

        let mut doc: serde_json::Value =
            serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
                .unwrap();

        doc["extraConfig"]["initialFunds"] = serde_json::json!({
            "file": ["genesis", "funds.json"],
            "hash": "0000000000000000000000000000000000000000000000000000000000000000"
        });

        let raw = doc.to_string();

        let err = match serde_json::from_str::<GenesisFile>(&raw) {
            Err(err) => err,
            Ok(_) => panic!("a file injection must not parse into an empty ledger"),
        };

        let msg = err.to_string();
        assert!(
            msg.contains("file"),
            "the error must name the arm it could not resolve, got: {msg}"
        );
    }
}
