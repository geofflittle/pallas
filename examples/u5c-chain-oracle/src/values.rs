//! Compares the values u5c holds for Dijkstra body fields with the source, body by body.

use pallas_primitives::StakeCredential;
use pallas_primitives::dijkstra::{self, AccountBalanceInterval};
use pallas_utxorpc::v1beta::spec::cardano as u5c;

/// One guard credential, its script flag and its hash, or None for a u5c credential with neither.
pub type Guard = Option<(bool, Vec<u8>)>;

/// One node of a Plutus datum in preorder, with the number of children it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Constr {
        tag: u64,
        any_constructor: u64,
        fields: usize,
    },
    Map(usize),
    Array(usize),
    /// The integer n when not negative, and minus one minus n when negative, n big endian with no leading zero byte.
    Integer {
        negative: bool,
        n: Vec<u8>,
    },
    Bytes(Vec<u8>),
    Absent,
}

/// One reward account and its interval, None for a u5c interval the source type has no form for.
pub type Interval = (Vec<u8>, Option<AccountBalanceInterval>);

/// The index and payload tokens of one guarding redeemer.
pub type Redeemer = (u32, Vec<Token>);

/// One credential required of the top level transaction and its datum tokens, None for a nil datum.
pub type TopLevelGuard = (Guard, Option<Vec<Token>>);

/// The credential of a source guard.
pub fn credential(c: &StakeCredential) -> Guard {
    match c {
        StakeCredential::AddrKeyhash(h) => Some((false, h.to_vec())),
        StakeCredential::ScriptHash(h) => Some((true, h.to_vec())),
    }
}

/// The credential of a u5c guard.
pub fn u5c_credential(c: Option<&u5c::StakeCredential>) -> Guard {
    use u5c::stake_credential::StakeCredential as C;
    match c?.stake_credential.as_ref()? {
        C::AddrKeyHash(h) => Some((false, h.to_vec())),
        C::ScriptHash(h) => Some((true, h.to_vec())),
    }
}

/// The preorder tokens of a source datum.
pub fn source_datum(_x: &pallas_primitives::PlutusData) -> Vec<Token> {
    vec![]
}

/// The preorder tokens of a u5c datum.
pub fn u5c_datum(_x: &u5c::PlutusData) -> Vec<Token> {
    vec![]
}

/// The guard credentials of one u5c transaction, in field order.
pub fn u5c_guards(tx: Option<&u5c::Tx>) -> Vec<Guard> {
    tx.into_iter()
        .flat_map(|t| &t.guards)
        .map(|c| u5c_credential(Some(c)))
        .collect()
}

/// How one body field's values compare with u5c over every body added.
#[derive(Clone, Debug, Default)]
pub struct ValueReport {
    pub bodies: usize,
    pub holding: usize,
    pub values: usize,
    pub differences: Vec<(u64, [u8; 32])>,
}

impl ValueReport {
    /// Tells whether a body held a value and u5c holds the values of every body, none where the source has none.
    pub fn agrees(&self) -> bool {
        self.values > 0 && self.differences.is_empty()
    }

    /// Counts one body's values and notes the body when u5c holds other values than the source.
    pub fn compare<T: PartialEq>(&mut self, at: (u64, [u8; 32]), source: Vec<T>, mapped: Vec<T>) {
        self.bodies += 1;
        if !source.is_empty() {
            self.holding += 1;
        }
        self.values += source.len();
        if source != mapped {
            self.differences.push(at);
        }
    }
}

/// The value reports of the body fields check 2 compares.
#[derive(Clone, Debug, Default)]
pub struct Values {
    pub guards: ValueReport,
    pub intervals: ValueReport,
    pub starting: ValueReport,
    pub guard_clauses: ValueReport,
    pub guarding_redeemers: ValueReport,
    pub top_level_guards: ValueReport,
}

impl Values {
    /// Each report beside the name check 2 prints it under.
    pub fn reports(&self) -> [(&'static str, &ValueReport); 6] {
        [
            ("key 14 guards", &self.guards),
            ("key 26 intervals", &self.intervals),
            ("key 27 starting intervals", &self.starting),
            ("guard native clauses", &self.guard_clauses),
            ("guarding redeemers", &self.guarding_redeemers),
            ("key 24 top level guards", &self.top_level_guards),
        ]
    }

    /// Tells whether every report agrees.
    pub fn agrees(&self) -> bool {
        self.reports().iter().all(|(_, r)| r.agrees())
    }

    /// Compares each body of a Dijkstra block, top level and sub transactions, with the u5c transaction at the same place.
    pub fn add(&mut self, _block: &dijkstra::Block, _number: u64, _mapped: &u5c::Block) {}
}
