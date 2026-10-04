use pallas_primitives::dijkstra;
use pallas_traverse::MultiEraTx;
use pallas_utxorpc::LedgerContext;
use pallas_utxorpc::v1beta::Mapper;

use crate::effects::Unmapped;
use crate::model::Credential;

fn deposits(x: Option<&dijkstra::DirectDeposits>) -> Vec<(Credential, u64)> {
    x.into_iter()
        .flatten()
        .map(|(account, coin)| {
            (
                Credential::from_reward_account(account).expect("a reward account"),
                *coin,
            )
        })
        .collect()
}

/// Reads the sub transactions and direct deposits of a Dijkstra transaction, mapping each sub transaction as a transaction of its own.
pub fn unmapped<C: LedgerContext>(
    mapper: &Mapper<C>,
    tx: &dijkstra::BlockTransaction,
) -> Option<Unmapped> {
    let body = &tx.transaction_body;
    if body.sub_transactions.is_none() && body.direct_deposits.is_none() {
        return None;
    }
    let subs = body
        .sub_transactions
        .iter()
        .flat_map(|subs| subs.iter())
        .map(|sub| {
            let mapped = mapper.map_tx(&MultiEraTx::from_dijkstra_sub(sub, tx.success));
            let deposits = deposits(sub.sub_transaction_body.direct_deposits.as_ref());
            (mapped, deposits)
        })
        .collect();
    Some(Unmapped {
        deposits: deposits(body.direct_deposits.as_ref()),
        subs,
    })
}
