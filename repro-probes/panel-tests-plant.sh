#!/bin/sh
# usage: plant.sh <arm>  plants one defect in the current tree, prints APPLIED or NOT APPLIED
A=$1
V=pallas-utxorpc/src/v1/mod.rs
T=pallas-traverse/src/tx.rs
C=pallas-utxorpc/Cargo.toml
sub() { F=$1; P=$2; S0=$(md5sum $F); perl -0pi -e "$P" $F; S1=$(md5sum $F); if [ "$S0" = "$S1" ]; then echo "ARM $A NOT APPLIED"; exit 3; fi; echo "ARM $A APPLIED $F"; }
case $A in
successful-always-true) sub $V 's/successful: tx\.is_valid\(\),/successful: true,/' ;;
validity-start-ttl-swap) sub $V 's/then_some\(u5c::TxValidity \{ start, ttl \}\)/then_some(u5c::TxValidity { start: ttl, ttl: start })/' ;;
fee-from-total-collateral) sub $V 's/fee: tx\.fee\(\)\.and_then\(u64_to_bigint\),/fee: tx.total_collateral().and_then(u64_to_bigint),/' ;;
bounded-interval-reversed) sub $V 's/Bounded\(l, u\) => range\(Some\(l\), Some\(u\)\)/Bounded(l, u) => range(Some(u), Some(l))/' ;;
inputs-unsorted) sub $V 's/\.inputs_sorted_set\(\)/.inputs()/' ;;
guard-key-as-script) sub $V 's/Guards::AddrKeyhashes\(x\) => x\.iter\(\)\.map\(map_key_hash\)\.collect\(\),/Guards::AddrKeyhashes(x) => x.iter().map(|h| map_credential(\&pallas_primitives::StakeCredential::ScriptHash(*h))).collect(),/' ;;
sub-hash-whole-subtx) sub $T 's/MultiEraTx::DijkstraSub\(x, _\) => x\.sub_transaction_body\.original_hash\(\),/MultiEraTx::DijkstraSub(x, _) => pallas_crypto::hash::Hasher::<256>::hash(\&minicbor::to_vec(x).unwrap()),/' ;;
starting-reads-account-intervals) sub $T 's/\.starting_account_balance_intervals\n(\s*)\.as_ref\(\),/.account_balance_intervals\n$1.as_ref(),/' ;;
alonzo-script-data-hash-none) sub $T 's/MultiEraTx::AlonzoCompatible\(x, _\) => x\.transaction_body\.script_data_hash\.as_ref\(\),/MultiEraTx::AlonzoCompatible(_, _) => None,/' ;;
babbage-aux-hash-none) sub $T 's/MultiEraTx::Babbage\(x\) => x\n\s*\.transaction_body\n\s*\.auxiliary_data_hash\n\s*\.as_ref\(\)\n\s*\.map\(\|x\| x\.as_slice\(\)\),/MultiEraTx::Babbage(_) => None,/' ;;
gate-v1-in-default) sub $C 's/default = \["u5c-v1alpha-compat"\]/default = ["u5c-v1alpha-compat", "utxorpc-spec\/utxorpc-v1-cardano"]/' ;;
clippy-lint-in-v1) sub $V 's/fn guarding_purpose\(\) -> u5c::RedeemerPurpose \{\n(\s*)u5c::RedeemerPurpose::Guarding\n/fn guarding_purpose() -> u5c::RedeemerPurpose {\n$1return u5c::RedeemerPurpose::Guarding;\n/' ;;
related-skips-reference-inputs) sub pallas-utxorpc/src/shared.rs 's/inputs\.chain\(collateral\)\.chain\(reference_inputs\)\.collect\(\)/{ let _ = reference_inputs; inputs.chain(collateral).collect() }/' ;;
*) echo "unknown arm $A"; exit 2 ;;
esac
