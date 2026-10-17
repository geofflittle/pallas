# Ex units trigger and Conway sum, runners

Box layout: copy this directory to `/opt/build/cv-exfix-1016/` on the build box (image `pallas-ci:1`, cargo cache `/opt/build/shared-cargo`).

- `sync.sh <tree> <rev|WORKTREE>` (laptop): writes a commit of `/Users/geofflittle/code/pallas-wt-conway-finish` to `/opt/build/cv-exfix-1016/<tree>` with a `HEAD_SHA` file.
- `gate.sh <tree>` (box): fmt, clippy default and phase2, tests default and phase2, one line each.
- `arm.sh <tree> <mutant|arms/X.patch> [filter]` (box): copies the tree, plants one defect, runs the phase2 tests.
- `muts.sh <tree> <mutant>...` (box): `arm.sh` per mutant, verdicts appended to `logs/mutants-<tree>.txt`.
- `mut.py` holds the named mutants, `arms/*.patch` the patch mutants, `msg/` the commit 3 body and the rebase todo editor.
- `redeemer_totals.py <tx hex file>...` (laptop, needs cbor2): redeemer count and summed mem and steps per fixture.

## Expected verdicts

Red, branch `repro/conway-ex-units-red` (tests on the old code): `./sync.sh red repro/conway-ex-units-red`, then on the box `find red -type f -exec touch {} + && W=red T=p2 ./run.sh cargo test -p pallas-validate --features phase2 --no-fail-fast --test babbage --test conway -- reference_script_tx_ex_units_exceeded`. Expected exit 101, `babbage_tests::reference_script_tx_ex_units_exceeded` panics `Transaction ex units should be below maximum`, `conway_tests::reference_script_tx_ex_units_exceeded` panics `Ok(())`.

Gate per commit at head f99a7127 (c1 to c6 are 23c05834, 255451e8, 9880a2cd, b59ac5f8, c2c9b716, f99a7127): every gate `exit=0`, tests default/phase2 passed 132/138, 144/150, 148/154, 148/156, 148/156, 148/158. Workspace on c6: clippy exit 0, test `passed=919 failed=0 ignored=19`.

Mutants on c6: `arm none ... exit=0`, `totals passed=158 failed=0`. Exit 0 also for `MX-alonzo-exunits-witness-trigger`, `MC-alonzo-collateral-witness-trigger` (Alonzo verdicts agree under both triggers) and `MN-eval-every-native` (out of scope). Every other mutant exit=101:

- MX-babbage-exunits-witness-trigger: `babbage_tests::reference_script_tx_ex_units_exceeded`
- MX-conway-exunits-witness-trigger, MX-conway-exunits-lazy-sum: `conway_tests::reference_script_tx_ex_units_exceeded`
- MC-babbage-collateral-witness-trigger: `babbage_tests::reference_script_tx_with_collateral_one_lovelace_short`
- MC-conway-collateral-witness-trigger: `conway_tests::reference_script_tx_with_collateral_one_lovelace_short`
- the earlier mutants: the same failing tests as the table in pallas `.claude/docs/2026-10-16_conway-validate-review-fixes.md`, see `mutants-c6.txt`
