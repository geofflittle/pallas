fn main() {
    let tx = pallas_utxorpc::v1::spec::cardano::Tx::default();
    println!("GATE unstable v1 sub transactions={}", tx.sub_transactions.len());
}
