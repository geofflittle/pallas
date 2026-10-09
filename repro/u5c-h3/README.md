# Apparatus for the native script fold and the rebuilt utxorpc commit

Area on the box: `/opt/build/u5c-h3-2026-10-09` with `src`, `target`, `target-msrv` and `cargo`, plus `<tree>` and `target-<tree>` for `run-tree.sh`. Run each cargo command alone. `verify.sh` calls the box's `/opt/build/ci-all.sh` (copy in `ci-all.sh.box`) with `CI_PROBE_AREA` set to this area.

Setup: `mkdir -p` the area dirs, copy `run.sh`, `run-tree.sh`, `verify.sh` and `compare.py` into the area and `chmod +x` them. `sync.sh <worktree>` copies a worktree into `src`.

## Final tip 8e37862f

`sync.sh <worktree at 8e37862f>`, then `verify.sh final`. Expected last lines: `test-run result lines 53 passed 944 failed 0`, `test-unstable result lines 8 passed 394 failed 0`, `clippy unstable exit=0`, `clippy stable exit=0`, `verify final exit=0`.

## Equivalence probe

Copy `probe_native_equiv.rs` into `pallas-utxorpc/tests` of the tree, then `PROBE_OUT=/target/probe-native run.sh cargo test -p pallas-utxorpc --features unstable --test probe_native_equiv -- --nocapture`. For bd200492, extract `git archive bd200492` into `<area>/bd`, add the probe and the blueprint dir, and run the same through `TREE=bd run-tree.sh`.

Expected on both: `PROBE native files=108 skipped=1 of 109 txs=510 scripts=94`, and `cmp` of the two `native.txt` exits 0 (sha256 `fb1a478b9edb97fe2a066d9d2d786b6e21df311c9e60363f5bbc83da3d0d07cb`). On repro/u5c-h3-mutant-any-as-all the count stays 94 and `cmp` reports `differ: byte 311926, line 87`, the four shelley4.tx lines.

## Mutants (each on its own branch, parent 4f2d0d28, same code as 8e37862f)

`sync.sh` the branch, then `run.sh cargo test -p pallas-utxorpc --features unstable --lib`:

- repro/u5c-h3-mutant-guard-as-all: `FAILED. 64 passed; 1 failed`
- repro/u5c-h3-mutant-guard-panics: `FAILED. 56 passed; 9 failed`, 9 `not implemented: guard clause`
- repro/u5c-h3-mutant-any-as-all: `FAILED. 63 passed; 2 failed`
- repro/u5c-h3-mutant-nofk-cast: `FAILED. 63 passed; 2 failed`

Traverse arms, `run.sh cargo test -p pallas-traverse --features unstable --lib fold`:

- repro/traverse-native-fold-red-tests: exit 101, `E0107` and `method fold not found`
- repro/traverse-native-fold-mutant-recursive: the two depth tests abort with `has overflowed its stack`
- repro/traverse-native-fold-mutant-held-dropped: `FAILED. 3 passed; 2 failed`, `left: ("all []", 8)`
- repro/traverse-native-fold-mutant-guard-as-pubkey: `FAILED. 4 passed; 1 failed`, `left: "all [pubkey 7a, pubkey 7a]"`
