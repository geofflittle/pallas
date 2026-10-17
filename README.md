# Conway validate fixes, runners

Box layout: copy this directory to `/opt/build/cv-fixes-1016/` on the build box (image `pallas-ci:1`, cargo cache `/opt/build/shared-cargo`).

- `sync.sh <tree> <rev|WORKTREE>` (laptop): writes a commit of the worktree at `/Users/geofflittle/code/pallas-wt-conway-finish` to `/opt/build/cv-fixes-1016/<tree>` with a `HEAD_SHA` file.
- `gate.sh <tree>` (box): fmt, clippy default and phase2, tests default and phase2, one line each.
- `arm.sh <tree> <mutant|arms/X.patch> [filter]` (box): copies the tree, applies one planted defect, runs the phase2 tests.
- `mut.py` holds the named mutants, `arms/*.patch` the patch mutants.
- `msg/` holds the reworded commit bodies and the rebase todo editor.

## Expected verdicts at head a4a2feb0

Gate per commit (c1 to c6 are 23c05834, 255451e8, 983992fe, c8b558ce, 9f9629b7, a4a2feb0): every gate `exit=0`, tests default/phase2 passed 132/138, 144/150, 147/153, 147/155, 147/155, 147/157.

Arms on c6: `arm none ... exit=0`, `totals passed=157 failed=0`. `MN-eval-every-native` exit=0 (out of scope). Every other mutant exit=101 with the failing test:

- A15m, A16m, A17m, A18m: `conway_tests::deposits_are_required_through_dispatch`
- DF1, DF2: `phase2::to_plutus_data::tests::public_encoding_is_the_protocol_9_encoding`
- K-cutoff-eq-10, K-certs-site-false, K-redeemers-site-false: `phase2::to_plutus_data::tests::v3_deposit_context_protocol_boundaries_at_every_occurrence`
- K-ge-9, K-gt-10, K-purpose-site-false: that test and `conway_tests::phase2_certificates::script_registration_with_its_redeemer_evaluates`
- MR-unreg-refund-zero: `phase1::conway::tests::deposits_refunds_withdrawals_and_donations_balance` and `each_certificate_charges_or_refunds_its_deposit`
- MR-drep-refund-zero, MR-dereg-refund-zero: `phase1::conway::tests::each_certificate_charges_or_refunds_its_deposit`
- MW-remaining-first-only: `phase1::conway::tests::native_certificate_checks_authorization_without_redeemers`
- MS-deleg-unwrapped: `conway_tests::phase2_certificates::v1_v2_context_encodes_stake_certificates_as_the_ledger_does`
- M762-spend-mint-reward-unfiltered: `native_reference_certificate_needs_no_redeemer` and `native_reference_scripts_need_no_redeemer_for_any_purpose`
- M762-mint-reward-unfiltered: `phase1::conway::tests::native_reference_scripts_need_no_redeemer_for_any_purpose`
- MV1-ref-hash-as-v2: `phase1::conway::tests::plutus_reference_scripts_need_a_redeemer_for_every_purpose` (message `PlutusV1`)
- PB-babbage-ref-collateral-1: `babbage_tests::successful_mainnet_tx_with_plutus_v2_script`

Red for the collateral trigger: branch `repro/conway-review-f2-red` (tests, old trigger). `cargo test -p pallas-validate --test alonzo --test babbage -- redeemers_without_scripts_require_collateral reference_script_tx_with_collateral_one_lovelace_short` gives exit 101, the Alonzo test panics `Unexpected error (Alonzo(ScriptWitnessMissing))` and the Babbage test panics `Collateral balance should contain the minimum lovelace`.
