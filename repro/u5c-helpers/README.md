# Apparatus for the traverse helpers and the rebuilt utxorpc commit

Area on the box: `/opt/build/u5c-helpers-2026-10-09` with `src`, `base`, `target`, `target-base`, `target-msrv` and `cargo`. Run each cargo command alone.

Setup: `mkdir -p` the area dirs, copy `run.sh`, `run-base.sh`, `verify.sh` and `compare.py` into the area, then `sync.sh <worktree>` copies a worktree into `src`.

## Per commit verification

`verify.sh <label>` after `sync.sh` of a worktree at that commit. Expected last lines:

- a23080fb: `test-run result lines 53 passed 930 failed 0`, `test unstable result lines 6 passed 180 failed 0`, `verify a23080fb exit=0`
- 5f80f71e: `passed 930`, `passed 182`, `verify 5f80f71e exit=0`
- b36ead39: `passed 932`, `passed 185`, `verify b36ead39 exit=0`
- bd200492: `passed 941`, `passed 220`, `verify bd200492 exit=0`

## Red and mutant branches

Sync the branch and run `run.sh cargo test -p <crate> --features unstable --lib <filter>`.

- repro/traverse-account-keys-red, crate pallas-traverse, filter account_keys: `FAILED. 1 passed; 1 failed`, `left: (None, None, None)`
- repro/traverse-signer-credentials-red, crate pallas-traverse, filter signers: `FAILED. 1 passed; 2 failed`, `left: []`
- repro/u5c-sub-verdict-red, crate pallas-utxorpc, filter a_sub_transaction_maps_the_verdict: `FAILED. 1 passed; 1 failed`, diff `< true` against `> false`
- repro/u5c-mutant-script-guards-dropped, crate pallas-utxorpc, filter guards_map: `FAILED. 1 passed; 1 failed`, the ScriptHash entry missing
- repro/u5c-mutant-sub-key-24, -25, -26, crate pallas-traverse, filter account_keys: `FAILED. 1 passed; 1 failed`, message `the sub transaction reports keys 24 to 26 of the sub body`

## Allow arm

repro/u5c-allow-needless-update-removed: `run.sh cargo clippy -p pallas-utxorpc --all-targets -- -D warnings`, with and without `--features unstable`. Expected `struct update has no effect` at shared.rs 1146, 1350 and 1537, exit 101.

## Tip probes (this branch)

After `sync.sh` of this branch, copy `pallas-utxorpc/tests/audit_blocks.rs` and `probe_pre_dijkstra.rs` into `src` by hand, since `sync.sh` leaves them out.

1. `run.sh cargo test -p pallas-utxorpc --features unstable --test audit_blocks -- --nocapture`: `AUDIT TOTAL blocks=15 panicking_mappings=0`
2. `PROBE_OUT=/target/probe-tip run.sh cargo test -p pallas-utxorpc --test probe_pre_dijkstra -- --nocapture`, and the same with `run-base.sh` and `PROBE_OUT=/target/probe-base` on `git archive f574d839` with the probe file added: `PROBE mapped=54 skipped=1 of 55` each.
3. `compare.py` over the v1alpha files of both: `TOTAL files 54 changed 0`. Over the v1beta files: `TOTAL files 54 changed 9`, `PATH 8 .body.tx[].guards`, and the deep file preprod-5183974 differs by one added `guards` entry.
4. `run.sh cargo test -p pallas-utxorpc --features unstable --lib probe_ -- --nocapture --test-threads 1`:
   - `PROBE bls_key present via map_cert true via map_cert_kind false traverse bls_key true`
   - `PROBE guards unstable=true files 108 of 109 txs 511 subs 1 nonempty 35 script_guards 0 mismatches 0 skipped ["conway8.block"]`
   - `PROBE primitives nil None absent None | mapped nil==default true | mapped (34,38nil)==(34) true`
   - `PROBE parent successful false sub successful false sub fee Some(BigInt { big_int: Some(Int(0)) }) parent fee Some(BigInt { big_int: Some(Int(700000)) })`
5. The same without `--features unstable`: `PROBE guards unstable=false files 90 of 109 txs 462 subs 0 nonempty 34 script_guards 0 mismatches 0`
