use pallas_crypto::hash::Hash;
use pallas_primitives::{StakeCredential, alonzo};

#[cfg(feature = "unstable")]
use pallas_primitives::dijkstra;

use crate::MultiEraSigners;

impl MultiEraSigners<'_> {
    pub fn as_alonzo(&self) -> Option<&alonzo::RequiredSigners> {
        match self {
            Self::AlonzoCompatible(x) => Some(x),
            _ => None,
        }
    }

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

    /// Collect the signing key hashes named by this value. A Dijkstra credential
    /// set yields its key credentials, and its script credentials are dropped.
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

    /// Collect the credentials named by this value, each key hash read as a key credential.
    pub fn collect_credentials<T>(&self) -> T
    where
        T: FromIterator<StakeCredential>,
    {
        std::iter::empty().collect()
    }
}

#[cfg(test)]
mod tests {
    use pallas_crypto::hash::Hash;
    use pallas_primitives::StakeCredential;

    use crate::MultiEraSigners;

    #[test]
    fn a_value_without_signers_names_no_credential() {
        let absent: Vec<StakeCredential> = MultiEraSigners::NotApplicable.collect_credentials();
        let empty: Vec<StakeCredential> = MultiEraSigners::Empty.collect_credentials();
        assert_eq!((absent, empty), (vec![], vec![]));
    }

    #[test]
    fn alonzo_signers_read_as_key_credentials() {
        let signers = vec![Hash::new([1; 28]), Hash::new([2; 28])];
        let got: Vec<StakeCredential> =
            MultiEraSigners::AlonzoCompatible(&signers).collect_credentials();
        assert_eq!(
            got,
            vec![
                StakeCredential::AddrKeyhash(Hash::new([1; 28])),
                StakeCredential::AddrKeyhash(Hash::new([2; 28])),
            ]
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn dijkstra_guards_name_key_and_script_credentials() {
        use pallas_primitives::dijkstra::{Guards, NonEmptySet};

        let keys = Guards::AddrKeyhashes(NonEmptySet::from_vec(vec![Hash::new([3; 28])]).unwrap());
        let credentials = Guards::Credentials(
            NonEmptySet::from_vec(vec![
                StakeCredential::AddrKeyhash(Hash::new([4; 28])),
                StakeCredential::ScriptHash(Hash::new([5; 28])),
            ])
            .unwrap(),
        );

        let from_keys: Vec<StakeCredential> = MultiEraSigners::Dijkstra(&keys).collect_credentials();
        let from_credentials: Vec<StakeCredential> =
            MultiEraSigners::Dijkstra(&credentials).collect_credentials();

        assert_eq!(
            (from_keys, from_credentials),
            (
                vec![StakeCredential::AddrKeyhash(Hash::new([3; 28]))],
                vec![
                    StakeCredential::AddrKeyhash(Hash::new([4; 28])),
                    StakeCredential::ScriptHash(Hash::new([5; 28])),
                ]
            )
        );
    }
}
