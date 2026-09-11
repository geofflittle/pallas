use pallas_crypto::hash::Hash;
use pallas_primitives::alonzo;

#[cfg(feature = "unstable")]
use pallas_primitives::{StakeCredential, dijkstra};

use crate::MultiEraSigners;

impl MultiEraSigners<'_> {
    pub fn as_alonzo(&self) -> Option<&alonzo::RequiredSigners> {
        match self {
            Self::AlonzoCompatible(x) => Some(x),
            _ => None,
        }
    }

    /// The Dijkstra guards behind this value, which carry more than a required
    /// signer list can: the second arm of `guards` is a credential set, and a
    /// script credential is not a signing key.
    #[cfg(feature = "unstable")]
    pub fn as_dijkstra(&self) -> Option<&dijkstra::Guards> {
        match self {
            Self::Dijkstra(x) => Some(x),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::AlonzoCompatible(x) => x.is_empty(),
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x) => match x {
                dijkstra::Guards::AddrKeyhashes(x) => x.is_empty(),
                dijkstra::Guards::Credentials(x) => x.is_empty(),
            },
            Self::NotApplicable | Self::Empty => true,
        }
    }

    /// Collect the signing key hashes named by this value.
    ///
    /// For Dijkstra's credential arm this yields only the key credentials. A
    /// script credential is a guard rather than a signer and has no signing
    /// key, so it is reachable through `MultiEraSigners::as_dijkstra`
    /// instead of being reported here as if it were a key hash.
    pub fn collect<'a, T>(&'a self) -> T
    where
        T: FromIterator<&'a Hash<28>>,
    {
        match self {
            Self::NotApplicable => std::iter::empty().collect(),
            Self::Empty => std::iter::empty().collect(),
            Self::AlonzoCompatible(x) => x.iter().collect(),
            #[cfg(feature = "unstable")]
            Self::Dijkstra(x) => match x {
                dijkstra::Guards::AddrKeyhashes(x) => x.iter().collect(),
                dijkstra::Guards::Credentials(x) => x
                    .iter()
                    .filter_map(|c| match c {
                        StakeCredential::AddrKeyhash(h) => Some(h),
                        StakeCredential::ScriptHash(_) => None,
                    })
                    .collect(),
            },
        }
    }
}
