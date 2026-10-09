//! Does clippy's needless_update fire on a struct literal emitted by a local
//! macro, for the module whose struct has no field beyond those listed?
//! Expected when it fires: one needless_update error, at the `narrow` expansion.

pub mod narrow {
    #[derive(Default, Debug)]
    pub struct Params {
        pub a: u64,
        pub b: u64,
    }
}

pub mod wide {
    #[derive(Default, Debug)]
    pub struct Params {
        pub a: u64,
        pub b: u64,
        pub c: u64,
    }
}

macro_rules! shared {
    ($m:ident) => {
        pub mod $m {
            pub fn build(x: u64) -> crate::$m::Params {
                crate::$m::Params {
                    a: x,
                    b: x,
                    ..Default::default()
                }
            }
        }
    };
}

pub mod mapped {
    shared!(narrow);
    shared!(wide);
}
