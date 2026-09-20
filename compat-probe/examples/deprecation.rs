// The twin of the main probe. The main probe allows the deprecated lint, so
// it would pass just as well if the restored items carried no deprecation at
// all. This arm denies the lint and names each restored item, so it MUST
// refuse to compile, once per item, against a release that marks them.
//
// Expected verdict: `cargo build --example deprecation` fails with seven
// `error: use of deprecated associated function` lines, one per item below.
// A build that succeeds means a restored item lost its deprecation.
#![deny(deprecated)]

use std::collections::HashMap;

use pallas_utxorpc::{LedgerContext, TxoRef, UtxoMap};
use pallas_utxorpc::v1beta::Mapper;

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        Some(HashMap::new())
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

type M = Mapper<NoLedger>;

fn main() {
    let _ = M::map_purpose;
    let _ = M::map_native_script;
    let _ = M::map_any_script;
    let _ = M::map_alonzo_compatible_cert;
    let _ = M::map_conway_cert;
    let _ = M::map_conway_pparams_update;
    let _ = M::map_conway_gov_action;
}
