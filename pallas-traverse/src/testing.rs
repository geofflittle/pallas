//! Transaction builders shared by this crate's tests.
//!
//! No fixture on the Musashi chain reaches a script witness, a redeemer, a
//! datum, a reference script or a proposal, so the cases that exercise those
//! are built rather than read. Every builder here writes CBOR directly, so a
//! test that reads a field back is reading bytes rather than a value this
//! crate encoded from its own types.

use pallas_codec::minicbor::{self, data::Tag};

/// Wrap a Dijkstra `block_transaction` around parts that are already CBOR.
///
/// Four elements, with the validity flag last.
pub fn dijkstra_block_tx(
    body: &[u8],
    witness_set: &[u8],
    auxiliary_data: Option<&[u8]>,
    valid: bool,
) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(4).unwrap();
    e.writer_mut().extend_from_slice(body);
    e.writer_mut().extend_from_slice(witness_set);
    write_aux(&mut e, auxiliary_data);
    e.bool(valid).unwrap();
    e.into_writer()
}

/// Wrap a Dijkstra `mempool_transaction` around the same parts.
///
/// Three elements, and no validity flag, because the block producer who sets
/// it has not seen the transaction yet.
pub fn dijkstra_mempool_tx(
    body: &[u8],
    witness_set: &[u8],
    auxiliary_data: Option<&[u8]>,
) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(3).unwrap();
    e.writer_mut().extend_from_slice(body);
    e.writer_mut().extend_from_slice(witness_set);
    write_aux(&mut e, auxiliary_data);
    e.into_writer()
}

/// Wrap a Conway `transaction` around the same parts. Four elements, with the
/// validity flag and the auxiliary data the other way round.
pub fn conway_tx(
    body: &[u8],
    witness_set: &[u8],
    auxiliary_data: Option<&[u8]>,
    valid: bool,
) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(4).unwrap();
    e.writer_mut().extend_from_slice(body);
    e.writer_mut().extend_from_slice(witness_set);
    e.bool(valid).unwrap();
    write_aux(&mut e, auxiliary_data);
    e.into_writer()
}

fn write_aux(e: &mut minicbor::Encoder<Vec<u8>>, auxiliary_data: Option<&[u8]>) {
    match auxiliary_data {
        Some(aux) => e.writer_mut().extend_from_slice(aux),
        None => {
            e.null().unwrap();
        }
    }
}

/// A transaction body with the three keys every era requires and nothing else:
/// one input, no outputs, and a fee.
pub fn minimal_body() -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(3).unwrap();
    write_mandatory_keys(&mut e);
    e.into_writer()
}

/// The same body with a `proposal_procedure` at key 20.
pub fn body_with_proposal(proposal: &[u8]) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(4).unwrap();
    write_mandatory_keys(&mut e);

    e.u8(20).unwrap();
    e.tag(Tag::new(258)).unwrap();
    e.array(1).unwrap();
    e.writer_mut().extend_from_slice(proposal);

    e.into_writer()
}

fn write_mandatory_keys(e: &mut minicbor::Encoder<Vec<u8>>) {
    e.u8(0).unwrap();
    e.tag(Tag::new(258)).unwrap();
    e.array(1).unwrap();
    e.array(2).unwrap();
    e.bytes(&[0x11; 32]).unwrap();
    e.u8(0).unwrap();

    e.u8(1).unwrap();
    e.array(0).unwrap();

    e.u8(2).unwrap();
    e.u32(1_000).unwrap();
}

/// A witness set carrying nothing.
pub fn empty_witness_set() -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(0).unwrap();
    e.into_writer()
}

/// A witness set carrying one native script at key 1.
pub fn witness_set_with_native_script(script: &[u8]) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(1).unwrap();
    e.u8(1).unwrap();
    e.tag(Tag::new(258)).unwrap();
    e.array(1).unwrap();
    e.writer_mut().extend_from_slice(script);
    e.into_writer()
}

/// A witness set carrying one redeemer at key 5, with the purpose tag and the
/// index asked for.
///
/// Dijkstra's `redeemers` rule is map only, so this is the one shape the era
/// accepts. The datum is an empty constructor and the execution units are
/// arbitrary, because nothing here reads them.
pub fn witness_set_with_redeemer(tag: u8, index: u32) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(1).unwrap();

    e.u8(5).unwrap();
    e.map(1).unwrap();

    // redeemers_key = [tag, index]
    e.array(2).unwrap();
    e.u8(tag).unwrap();
    e.u32(index).unwrap();

    // redeemers_value = [plutus_data, ex_units]
    e.array(2).unwrap();
    e.tag(Tag::new(121)).unwrap();
    e.array(0).unwrap();
    e.array(2).unwrap();
    e.u32(1_000).unwrap();
    e.u32(2_000).unwrap();

    e.into_writer()
}

/// `script_pubkey = (0, addr_keyhash)`, a clause every era since Shelley has.
pub fn native_script_pubkey(keyhash: u8) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(2).unwrap();
    e.u8(0).unwrap();
    e.bytes(&[keyhash; 28]).unwrap();
    e.into_writer()
}

/// `script_require_guard = (6, credential)`, the clause new in Dijkstra. No
/// earlier era's native script type can hold it.
pub fn native_script_require_guard(keyhash: u8) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(2).unwrap();
    e.u8(6).unwrap();
    e.array(2).unwrap();
    e.u8(0).unwrap();
    e.bytes(&[keyhash; 28]).unwrap();
    e.into_writer()
}

/// Auxiliary data in the post Alonzo map form, carrying one native script at
/// key 1 and one PlutusV4 script at key 5.
///
/// Key 5 is new in Dijkstra, and it and a reference script are the two routes
/// a V4 script has into a transaction.
pub fn post_alonzo_aux_data(native_script: &[u8], plutus_v4: Option<&[u8]>) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.tag(Tag::new(259)).unwrap();
    e.map(if plutus_v4.is_some() { 2 } else { 1 }).unwrap();

    e.u8(1).unwrap();
    e.array(1).unwrap();
    e.writer_mut().extend_from_slice(native_script);

    if let Some(script) = plutus_v4 {
        e.u8(5).unwrap();
        e.array(1).unwrap();
        e.bytes(script).unwrap();
    }

    e.into_writer()
}

/// An output in the post Alonzo map form, carrying a reference script at key
/// 3. The script itself rides inside a `#6.24` byte string.
pub fn post_alonzo_output_with_script_ref(address: &[u8], coin: u64, script: &[u8]) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(3).unwrap();

    e.u8(0).unwrap();
    e.bytes(address).unwrap();

    e.u8(1).unwrap();
    e.u64(coin).unwrap();

    e.u8(3).unwrap();
    e.tag(Tag::new(24)).unwrap();
    e.bytes(script).unwrap();

    e.into_writer()
}

/// `plutus_v4_script` as a `script` alternative, which is the fourth arm and
/// the one no earlier era has.
pub fn script_ref_plutus_v4(bytes: &[u8]) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.array(2).unwrap();
    e.u8(4).unwrap();
    e.bytes(bytes).unwrap();
    e.into_writer()
}

/// A transaction body with one input, one output written as raw CBOR, and a
/// fee.
pub fn body_with_output(output: &[u8]) -> Vec<u8> {
    let mut e = minicbor::Encoder::new(Vec::new());
    e.map(3).unwrap();

    e.u8(0).unwrap();
    e.tag(Tag::new(258)).unwrap();
    e.array(1).unwrap();
    e.array(2).unwrap();
    e.bytes(&[0x11; 32]).unwrap();
    e.u8(0).unwrap();

    e.u8(1).unwrap();
    e.array(1).unwrap();
    e.writer_mut().extend_from_slice(output);

    e.u8(2).unwrap();
    e.u32(1_000).unwrap();

    e.into_writer()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two transaction fixtures `pallas-utxorpc` reads are these builders
    /// run once and written down.
    ///
    /// That crate cannot call a `#[cfg(test)]` module of this one, so without
    /// this the two would be separate hand written copies of one shape, free
    /// to drift. Here they are one shape with a byte for byte check.
    #[test]
    fn the_shared_transaction_fixtures_are_what_these_builders_write() {
        let proposal = hex::decode(include_str!(
            "../../test_data/proposal-param-change-key48.hex"
        ))
        .unwrap();
        let built = dijkstra_block_tx(
            &body_with_proposal(&proposal),
            &empty_witness_set(),
            None,
            true,
        );
        assert_eq!(
            hex::encode(&built),
            include_str!("../../test_data/dijkstra-proposal.tx").trim(),
            "test_data/dijkstra-proposal.tx has drifted from the builder"
        );

        let guard = native_script_require_guard(0x7a);
        let v4 = [0xd8, 0x79, 0x80];
        let output =
            post_alonzo_output_with_script_ref(&[0x60; 29], 2_000_000, &script_ref_plutus_v4(&v4));
        let built = dijkstra_block_tx(
            &body_with_output(&output),
            &witness_set_with_native_script(&guard),
            Some(&post_alonzo_aux_data(&guard, Some(&v4))),
            true,
        );
        assert_eq!(
            hex::encode(&built),
            include_str!("../../test_data/dijkstra-scripts.tx").trim(),
            "test_data/dijkstra-scripts.tx has drifted from the builder"
        );
    }
}
