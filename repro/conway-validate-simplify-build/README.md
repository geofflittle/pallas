# Apparatus for the 2026-10-16 conway validate simplify build

Subject: pallas branch fix/validate-conway-deposits-witnesses-collateral rebuilt as cfc9b39a (deposits), 66d4bd93 (witnesses), d1075450 (collateral) on base e6c38ac5. The tree before the change is repro/conway-validate-combined-before-simplify (fe81b2d3).

Box: any host with docker and the pallas-ci:1 image. Paths below use /opt/build/cvsimplify-build-1016 (the A variable in every script).

1. Copy run.sh, job.sh, gate.sh, arm.sh and arms-all.sh to the box directory, copy arms-new/ to its arms/ directory, and give it a cargo home at cargo/.
2. `sh sync.sh <rev> <tree>` exports a commit of the pallas repository to the box as <tree>.
3. `gate.sh <tree>` runs fmt check, clippy with and without phase2 and the validate tests with and without phase2.
4. `arms-all.sh` runs the baseline and every arm in arms/ against the tree named tip, verdicts in logs/arms-all.out.

arms-src/ holds the hand written arms, arms-new/ the exact patches against d1075450 that regen.sh produced from arms-src/ and from the review arms (scratch repository simplify-review, branch repro/conway-validate-simplify-review).

Expected verdict lines:

- `gate.sh dep` (cfc9b39a): every job exit=0, `totals passed=132 failed=0`, with phase2 `totals passed=138 failed=0`
- `gate.sh wit` (66d4bd93): every job exit=0, `totals passed=143 failed=0`, with phase2 `totals passed=149 failed=0`
- `gate.sh tip` (d1075450): every job exit=0, `totals passed=144 failed=0`, with phase2 `totals passed=150 failed=0`
- `arm.sh baseline` on tip: `exit=0`, `totals passed=144 failed=0`
- `arm.sh A16m-drep-deposit-zero`, `arm.sh A17m-gov-deposit-zero`, `arm.sh F4v1-v1-ref-hash-tag`, `arm.sh F4v2-v2-ref-hash-tag`: `exit=0`
- every other arm in arms-new/: `exit=101`, failing tests as listed in arms-all.table
- `arm.sh F5r-check-minting-ref-inputs-only` fails only plutus_reference_scripts_need_a_redeemer_for_every_purpose
- `arm.sh M1-check-minting-never-refuses` fails only minted_policy_needs_a_script

results/ holds the logs of the run, arms-all.table the verdict per arm.
