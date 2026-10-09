// Review probes first written for ef82eb82, rerun on the rebuilt utxorpc commit.
// Each probe prints one PROBE line.

use super::*;
use crate::{TxoRef, UtxoMap};

#[derive(Clone)]
struct NoLedger;

impl LedgerContext for NoLedger {
    fn get_utxos(&self, _refs: &[TxoRef]) -> Option<UtxoMap> {
        None
    }

    fn get_slot_timestamp(&self, _slot: u64) -> Option<u64> {
        None
    }
}

fn hexfile(path: &std::path::Path) -> Option<Vec<u8>> {
    let text = std::fs::read_to_string(path).ok()?;
    hex::decode(text.trim()).ok()
}

#[derive(Default, Debug)]
struct Counts {
    txs: usize,
    nonempty: usize,
    script_guards: usize,
    mismatches: usize,
    subs: usize,
}

fn key_hashes(guards: &[u5c::StakeCredential], counts: &mut Counts) -> Vec<Vec<u8>> {
    guards
        .iter()
        .filter_map(|x| match &x.stake_credential {
            Some(u5c::stake_credential::StakeCredential::AddrKeyHash(h)) => Some(h.to_vec()),
            _ => {
                counts.script_guards += 1;
                None
            }
        })
        .collect()
}

fn check(mapped: &u5c::Tx, tx: &trv::MultiEraTx, name: &str, counts: &mut Counts) {
    counts.txs += 1;
    let signers = tx.required_signers();
    let expected: Vec<Vec<u8>> = signers
        .collect::<Vec<&pallas_crypto::hash::Hash<28>>>()
        .into_iter()
        .map(|x| x.to_vec())
        .collect();
    let got = key_hashes(&mapped.guards, counts);
    if !mapped.guards.is_empty() {
        counts.nonempty += 1;
    }
    if got != expected {
        counts.mismatches += 1;
        println!("MISMATCH {name} {} got {got:?} expected {expected:?}", tx.hash());
    }
    #[cfg(feature = "unstable")]
    {
        let subs = tx.sub_transactions();
        assert_eq!(subs.len(), mapped.sub_transactions.len());
        for (sub, sub_mapped) in subs.iter().zip(&mapped.sub_transactions) {
            counts.subs += 1;
            check(sub_mapped, sub, name, counts);
        }
    }
}

#[test]
fn probe_guards_equal_required_signers() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_data");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().to_string())
        .filter(|x| x.ends_with(".block") || x.ends_with(".tx"))
        .collect();
    names.sort();

    let mapper = Mapper::new(NoLedger);
    let mut counts = Counts::default();
    let mut files = 0;
    let mut skipped = Vec::new();

    for name in &names {
        let Some(cbor) = hexfile(&dir.join(name)) else {
            skipped.push(name.clone());
            continue;
        };
        if name.ends_with(".block") {
            let Ok(block) = trv::MultiEraBlock::decode(&cbor) else {
                skipped.push(name.clone());
                continue;
            };
            files += 1;
            for tx in block.txs() {
                let mapped = mapper.map_tx(&tx);
                check(&mapped, &tx, name, &mut counts);
            }
        } else {
            let Ok(tx) = trv::MultiEraTx::decode(&cbor) else {
                skipped.push(name.clone());
                continue;
            };
            files += 1;
            let mapped = mapper.map_tx(&tx);
            check(&mapped, &tx, name, &mut counts);
        }
    }

    println!(
        "PROBE guards unstable={} files {files} of {} txs {} subs {} nonempty {} script_guards {} mismatches {} skipped {:?}",
        cfg!(feature = "unstable"),
        names.len(),
        counts.txs,
        counts.subs,
        counts.nonempty,
        counts.script_guards,
        counts.mismatches,
        skipped
    );
    assert_eq!(counts.mismatches, 0);
}

#[cfg(feature = "unstable")]
fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../test_data")
        .join(name);
    hexfile(&path).expect("the fixture holds hex")
}

#[cfg(feature = "unstable")]
#[test]
fn probe_cert_kind_and_cert_disagree_on_bls_key() {
    let cbor = fixture("dijkstra2.block");
    let block = trv::MultiEraBlock::decode(&cbor).unwrap();
    let txs = block.txs();
    let certs = txs[0].certs();
    let mapper = Mapper::new(NoLedger);

    let via_cert = mapper
        .map_cert(&certs[0], &txs[0], 0)
        .and_then(|x| x.certificate);
    let via_kind = mapper.map_cert_kind(&certs[0].kind().unwrap());

    let bls = |x: &Option<u5c::certificate::Certificate>| match x {
        Some(u5c::certificate::Certificate::PoolRegistration(p)) => p.bls_key.is_some(),
        _ => panic!("a pool registration"),
    };
    println!(
        "PROBE bls_key present via map_cert {} via map_cert_kind {} traverse bls_key {}",
        bls(&via_cert),
        bls(&via_kind),
        certs[0].bls_key().is_some()
    );
}

#[cfg(feature = "unstable")]
fn param_change(body: impl FnOnce(&mut pallas_codec::minicbor::Encoder<Vec<u8>>)) -> Vec<u8> {
    let mut e = pallas_codec::minicbor::Encoder::new(Vec::new());
    e.array(4).unwrap();
    e.u8(0).unwrap();
    e.null().unwrap();
    body(&mut e);
    e.null().unwrap();
    e.into_writer()
}

#[cfg(feature = "unstable")]
#[test]
fn probe_pledge_leverage_nil_and_absent() {
    use pallas_primitives::dijkstra;

    let nil = param_change(|e| {
        e.map(1).unwrap();
        e.u8(38).unwrap();
        e.null().unwrap();
    });
    let absent = param_change(|e| {
        e.map(1).unwrap();
        e.u8(34).unwrap();
        e.u8(1).unwrap();
    });
    let absent_with_34 = param_change(|e| {
        e.map(2).unwrap();
        e.u8(34).unwrap();
        e.u8(1).unwrap();
        e.u8(38).unwrap();
        e.null().unwrap();
    });

    let decode = |cbor: &[u8]| -> dijkstra::GovAction { pallas_codec::minicbor::decode(cbor).unwrap() };
    let a_nil = decode(&nil);
    let a_abs = decode(&absent);
    let a_both = decode(&absent_with_34);

    let field = |a: &dijkstra::GovAction| match a {
        dijkstra::GovAction::ParameterChange(_, p, _) => format!("{:?}", p.max_pledge_leverage),
        _ => panic!("a parameter change"),
    };

    let mapper = Mapper::new(NoLedger);
    let map = |a: &dijkstra::GovAction| {
        mapper.map_multi_era_gov_action(&trv::MultiEraGovAction::from_dijkstra(a))
    };

    println!(
        "PROBE primitives nil {} absent {} | mapped nil==default {} | mapped (34,38nil)==(34) {}",
        field(&a_nil),
        field(&a_abs),
        map(&a_nil)
            == u5c::GovernanceAction {
                governance_action: Some(
                    u5c::governance_action::GovernanceAction::ParameterChangeAction(
                        u5c::ParameterChangeAction {
                            protocol_param_update: Some(u5c::PParams::default()),
                            ..Default::default()
                        }
                    )
                ),
            },
        map(&a_both) == map(&a_abs),
    );
}

#[cfg(feature = "unstable")]
#[test]
fn probe_sub_success_under_a_failed_parent() {
    let cbor = fixture("dijkstra-subtx.tx");
    let tx = trv::MultiEraTx::decode_for_era(trv::Era::Dijkstra, &cbor).unwrap();
    let mut flipped = tx.as_dijkstra().unwrap().clone();
    flipped.success = false;
    let flipped = trv::MultiEraTx::from_dijkstra(&flipped);

    let mapped = Mapper::new(NoLedger).map_tx(&flipped);
    println!(
        "PROBE parent successful {} sub successful {} sub fee {:?} parent fee {:?}",
        mapped.successful,
        mapped.sub_transactions[0].successful,
        mapped.sub_transactions[0].fee,
        mapped.fee
    );
}
