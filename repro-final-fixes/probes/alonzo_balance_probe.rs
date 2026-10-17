
#[cfg(test)]
mod review_probe_balance {
    use super::super::fixtures;
    use super::check_preservation_of_value;
    use pallas_codec::minicbor;
    use pallas_primitives::alonzo::{TransactionBody, Tx};
    use pallas_traverse::Era;

    #[test]
    fn review_probe_withdrawal_balances_as_the_ledger_does() {
        let output = fixtures::legacy_output(fixtures::key_address());
        let utxos = fixtures::utxos(Era::Alonzo, &output);
        let raw = fixtures::tx_spending_input();
        let tx: Tx = minicbor::decode(&raw).unwrap();
        for withdrawal in [None, Some(1_000_000u64)] {
            let mut body: TransactionBody = tx.transaction_body.clone().unwrap();
            body.fee = 5_000_000 + withdrawal.unwrap_or(0);
            body.withdrawals = withdrawal
                .map(|coin| [(fixtures::reward_address().into(), coin)].into_iter().collect());
            println!(
                "PROBE alonzo withdrawal={withdrawal:?} {:?}",
                check_preservation_of_value(&body, &utxos, 2_000_000)
            );
        }
    }
}
