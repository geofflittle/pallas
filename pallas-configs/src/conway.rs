use num_rational::Ratio;
use serde::de::{
    self, MapAccess, SeqAccess, Unexpected, Visitor,
    value::{MapAccessDeserializer, SeqAccessDeserializer},
};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fmt};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenesisFile {
    pub pool_voting_thresholds: PoolVotingThresholds,
    pub d_rep_voting_thresholds: DRepVotingThresholds,
    pub committee_min_size: u64,
    pub committee_max_term_length: u32,
    pub gov_action_lifetime: u32,
    pub gov_action_deposit: u64,
    pub d_rep_deposit: u64,
    pub d_rep_activity: u32,
    pub min_fee_ref_script_cost_per_byte: u64,
    pub plutus_v3_cost_model: Vec<i64>,
    pub constitution: Constitution,
    pub committee: Committee,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PoolVotingThresholds {
    pub committee_normal: f32,
    pub committee_no_confidence: f32,
    pub hard_fork_initiation: f32,
    pub motion_no_confidence: f32,
    pub pp_security_group: f32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DRepVotingThresholds {
    pub motion_no_confidence: f32,
    pub committee_normal: f32,
    pub committee_no_confidence: f32,
    pub update_to_constitution: f32,
    pub hard_fork_initiation: f32,
    pub pp_network_group: f32,
    pub pp_economic_group: f32,
    pub pp_technical_group: f32,
    pub pp_gov_group: f32,
    pub treasury_withdrawal: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Constitution {
    pub anchor: Anchor,
    pub script: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub data_hash: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Committee {
    pub members: HashMap<String, u64>,
    pub threshold: Fraction,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(from = "FractionForm")]
pub struct Fraction {
    pub numerator: u64,
    pub denominator: u64,
}

struct FractionForm(Fraction);

impl From<FractionForm> for Fraction {
    fn from(form: FractionForm) -> Self {
        form.0
    }
}

#[derive(Deserialize)]
struct FractionTerms {
    numerator: u64,
    denominator: u64,
}

impl From<FractionTerms> for Fraction {
    fn from(terms: FractionTerms) -> Self {
        Self {
            numerator: terms.numerator,
            denominator: terms.denominator,
        }
    }
}

impl<'de> Deserialize<'de> for FractionForm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer
            .deserialize_any(FractionFormVisitor)
            .map(FractionForm)
    }
}

struct FractionFormVisitor;

impl<'de> Visitor<'de> for FractionFormVisitor {
    type Value = Fraction;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a fraction object, a two element array or a non negative number")
    }

    fn visit_map<A>(self, map: A) -> Result<Fraction, A::Error>
    where
        A: MapAccess<'de>,
    {
        FractionTerms::deserialize(MapAccessDeserializer::new(map)).map(Fraction::from)
    }

    fn visit_seq<A>(self, seq: A) -> Result<Fraction, A::Error>
    where
        A: SeqAccess<'de>,
    {
        FractionTerms::deserialize(SeqAccessDeserializer::new(seq)).map(Fraction::from)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Fraction, E> {
        Ok(Fraction {
            numerator: value,
            denominator: 1,
        })
    }

    fn visit_i64<E>(self, value: i64) -> Result<Fraction, E>
    where
        E: de::Error,
    {
        match u64::try_from(value) {
            Ok(value) => self.visit_u64(value),
            Err(_) => Err(E::invalid_value(Unexpected::Signed(value), &self)),
        }
    }

    fn visit_f64<E>(self, value: f64) -> Result<Fraction, E>
    where
        E: de::Error,
    {
        decimal_fraction(value).ok_or_else(|| E::invalid_value(Unexpected::Float(value), &self))
    }
}

fn decimal_fraction(value: f64) -> Option<Fraction> {
    if value < 0.0 {
        return None;
    }

    // Display writes the shortest decimal that parses back to the same f64, with no exponent.
    let text = value.abs().to_string();
    let (whole, decimals) = text.split_once('.').unwrap_or((&text, ""));
    let numerator = format!("{whole}{decimals}").parse().ok()?;
    let denominator = 10u64.checked_pow(u32::try_from(decimals.len()).ok()?)?;
    let ratio = Ratio::new(numerator, denominator);

    Some(Fraction {
        numerator: *ratio.numer(),
        denominator: *ratio.denom(),
    })
}

impl From<Fraction> for pallas_primitives::conway::RationalNumber {
    fn from(value: Fraction) -> Self {
        Self {
            numerator: value.numerator,
            denominator: value.denominator,
        }
    }
}

pub fn from_file(path: &std::path::Path) -> Result<GenesisFile, std::io::Error> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let parsed: GenesisFile = serde_json::from_reader(reader)?;

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_data_config(network: &str) -> GenesisFile {
        let path = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("..")
            .join("test_data")
            .join(format!("{network}-conway-genesis.json"));

        from_file(&path).unwrap()
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
    fn a_committee_threshold_object_keeps_its_terms() {
        let threshold = load_test_data_config("mainnet").committee.threshold;

        assert_eq!((threshold.numerator, threshold.denominator), (2, 3));
    }

    #[test]
    fn a_committee_threshold_array_keeps_its_terms() {
        let committee: Committee =
            serde_json::from_str(r#"{ "members": {}, "threshold": [2, 3] }"#).unwrap();

        let threshold = committee.threshold;
        assert_eq!((threshold.numerator, threshold.denominator), (2, 3));
    }

    #[test]
    fn a_committee_threshold_number_loads_as_a_fraction() {
        let threshold = load_test_data_config("proto-devnet").committee.threshold;

        assert_eq!((threshold.numerator, threshold.denominator), (0, 1));
    }

    #[test]
    fn a_committee_threshold_decimal_loads_as_its_exact_fraction() {
        let cases = [
            ("0.67", (67, 100)),
            ("0.5", (1, 2)),
            ("0.123456789", (123456789, 1000000000)),
            ("0.6666666666666666", (3333333333333333, 5000000000000000)),
        ];

        for (decimal, expected) in cases {
            let json = format!(r#"{{ "members": {{}}, "threshold": {decimal} }}"#);
            let threshold = serde_json::from_str::<Committee>(&json).unwrap().threshold;

            assert_eq!(
                (threshold.numerator, threshold.denominator),
                expected,
                "{decimal}"
            );
        }
    }
}
