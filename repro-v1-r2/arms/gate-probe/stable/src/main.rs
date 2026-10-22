fn main() {
    let tx = pallas_utxorpc::v1beta::spec::cardano::Tx::default();
    println!("GATE stable v1beta tx inputs={}", tx.inputs.len());
}
