//! Probe for the one runtime claim the PR 815 rebase resolution rests on, that
//! MultiEraCert::kind panics for a Dijkstra certificate and returns a kind for
//! every certificate of an earlier era. Not part of any posted branch.

#![cfg(feature = "unstable")]

use std::panic::{AssertUnwindSafe, catch_unwind};

use pallas_traverse::MultiEraBlock;

fn block(hex_str: &str) -> Vec<u8> {
    hex::decode(hex_str).expect("invalid hex")
}

#[test]
fn a_dijkstra_certificate_makes_kind_panic_with_the_message_the_mapper_carried() {
    let cbor = block(include_str!("../../test_data/dijkstra13.block"));
    let decoded = MultiEraBlock::decode(&cbor).expect("invalid cbor");
    let txs = decoded.txs();
    let certs: Vec<_> = txs.iter().flat_map(|tx| tx.certs()).collect();
    assert_eq!(certs.len(), 2, "this fixture writes two certificates");

    let mut panics = 0usize;
    for cert in certs.iter() {
        let payload = catch_unwind(AssertUnwindSafe(|| cert.kind()))
            .expect_err("a Dijkstra certificate has no kind to report");
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .expect("the panic payload is a string");
        assert_eq!(
            message, "not implemented: map_cert is not yet implemented for Dijkstra",
            "the panic must name what is unimplemented"
        );
        panics += 1;
    }

    assert_eq!(panics, 2, "every Dijkstra certificate must reach the panic");
}

#[test]
fn an_earlier_era_certificate_reads_a_kind_without_panicking() {
    let cbor = block(include_str!("../../test_data/babbage10.block"));
    let decoded = MultiEraBlock::decode(&cbor).expect("invalid cbor");
    let txs = decoded.txs();
    let certs: Vec<_> = txs.iter().flat_map(|tx| tx.certs()).collect();
    assert!(!certs.is_empty(), "this fixture writes a certificate");

    let mut read = 0usize;
    for cert in certs.iter() {
        let kind = catch_unwind(AssertUnwindSafe(|| cert.kind()))
            .expect("an Alonzo certificate must not reach the Dijkstra panic");
        assert!(kind.is_some(), "an Alonzo certificate reports a kind");
        read += 1;
    }

    assert_eq!(read, certs.len(), "every certificate must read a kind");
}
