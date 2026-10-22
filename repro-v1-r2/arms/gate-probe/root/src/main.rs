fn main() {
    let tx = utxorpc_spec::utxorpc::v1beta::cardano::Tx::default();
    println!("GATE root without unstable v1beta tx inputs={}", tx.inputs.len());
}
