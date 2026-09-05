//! Internal supporting utilities

use pallas_primitives::{alonzo, babbage, byron, conway, dijkstra};

macro_rules! clone_tx_fn {
    ($fn_name:ident, $era:tt) => {
        fn $fn_name<'b>(block: &'b $era::Block, index: usize) -> Option<$era::Tx<'b>> {
            let transaction_body = block.transaction_bodies.get(index).cloned()?;

            let transaction_witness_set = block.transaction_witness_sets.get(index)?.clone();

            let success = !block
                .invalid_transactions
                .as_ref()
                .map(|x| x.contains(&(index as u32)))
                .unwrap_or(false);

            let auxiliary_data = block
                .auxiliary_data_set
                .iter()
                .find_map(|(idx, val)| {
                    if idx.eq(&(index as u32)) {
                        Some(val)
                    } else {
                        None
                    }
                })
                .cloned()
                .into();

            let x = $era::Tx {
                transaction_body,
                transaction_witness_set,
                success,
                auxiliary_data,
            };

            Some(x)
        }
    };
}

clone_tx_fn!(conway_clone_tx_at, conway);
clone_tx_fn!(babbage_clone_tx_at, babbage);
clone_tx_fn!(alonzo_clone_tx_at, alonzo);

pub fn clone_alonzo_txs<'b>(block: &'b alonzo::Block) -> Vec<alonzo::Tx<'b>> {
    (0..block.transaction_bodies.len())
        .step_by(1)
        .filter_map(|idx| alonzo_clone_tx_at(block, idx))
        .collect()
}

pub fn clone_babbage_txs<'b>(block: &'b babbage::Block) -> Vec<babbage::Tx<'b>> {
    (0..block.transaction_bodies.len())
        .step_by(1)
        .filter_map(|idx| babbage_clone_tx_at(block, idx))
        .collect()
}

pub fn clone_conway_txs<'b>(block: &'b conway::Block) -> Vec<conway::Tx<'b>> {
    (0..block.transaction_bodies.len())
        .step_by(1)
        .filter_map(|idx| conway_clone_tx_at(block, idx))
        .collect()
}

pub fn clone_byron_txs<'b>(block: &'b byron::Block) -> Vec<byron::TxPayload<'b>> {
    block.body.tx_payload.iter().cloned().collect()
}

/// Dijkstra transactions are already complete inside the block body, so there
/// is nothing to reassemble from segregated witness and auxiliary data lists.
///
/// Validity is the one piece that is not in the transaction. Dijkstra strips
/// the `is_valid` flag when a transaction enters a block and records the
/// invalid ones as an index set on the block body instead, so it is paired
/// back on here where the block is still in hand.
pub fn clone_dijkstra_txs<'b>(block: &'b dijkstra::Block) -> Vec<(dijkstra::Tx<'b>, bool)> {
    let invalid: &[u32] = match &block.block_body.invalid_transactions {
        pallas_codec::utils::Nullable::Some(x) => x.as_slice(),
        _ => &[],
    };

    block
        .block_body
        .transactions
        .iter()
        .enumerate()
        .map(|(idx, tx)| (tx.clone(), !invalid.contains(&(idx as u32))))
        .collect()
}
