# Conway validate fixes, saved controls

Build record: .claude/docs/2026-10-15_conway-validate-fixes-build.md on the pallas checkout.

## Box setup

The box area is root@u5c.musashi.sundae.fi:/opt/build/cvfix-1015, removed after the run. To rerun, copy box/job.sh and box/run.sh there, then `sh box/sync.sh <checkout> <W>` puts a tree at `$A/<W>`. job.sh runs a command in the pallas-ci:1 image and prints its exit code and result lines.

## Planted defects

Each plant is a whole source file. `W=<W> TREE=<checkout> FEATURES=<flags> sh box/red2.sh NAME "<filter>" <relpath>:planted/<file>` swaps it in, runs `cargo test -p pallas-validate`, and restores the file. `box/ci-red.sh NAME <relpath> planted/<file> <command>` does the same for any command.

| Plant | Tree | Path | Command | Expected verdict |
|---|---|---|---|---|
| conway.rs.f2-drop | fix branch 682a957f | pallas-validate/src/phase1/conway.rs | `--lib input_eras` with `DROP=<source>` set in the container | drop=none passes, each other source fails on its own needed row turned to `[]` |
| to_plutus_data.rs.f5-legacy | 8ad39693 | pallas-validate/src/phase2/to_plutus_data.rs | `--features phase2 --lib script_context` | 1 failed, 5 passed, StakingHash constructor missing in row one |
| conway.rs.f6-any-root | 5f6d8659 | pallas-validate/src/phase1/conway.rs | `--test conway byron` | 1 failed, row 4 `"Ok(())"` |
| conway.rs.f10-decoding | ebd61d54 | pallas-validate/src/phase1/conway.rs | `--lib input_eras` | 2 failed, 9 passed |
| utils.rs.f7-declared | 03f2835e | pallas-validate/src/utils.rs | `--features phase2 --lib registration_deposits` | 3 failed, 3 passed |
| phase2-mod.rs.f9-lint | db1e9cc0 | pallas-validate/src/phase2/mod.rs | `cargo clippy -p pallas-validate --all-targets --features phase2 -- -D warnings` | exit 101, clone on `u8`. Workspace clippy exit 0 |
| pr9-conway.rs.f2-any-language | 52174f13 | pallas-validate/src/phase1/conway.rs | `--features unstable --lib native_reference_languages` | 1 failed, unneeded rows ScriptIntegrityHash |
| pr9-dijkstra.rs.no-deposit | 52174f13 or later | pallas-validate/src/phase1/dijkstra.rs | `--features unstable --lib dijkstra` | 6 failed, 35 passed, PreservationOfValue |
| pr9-conway.rs.f3-native-redeemer | e90bfa9b | pallas-validate/src/phase1/conway.rs | `--features unstable --lib native_reference_timelocks` | 1 failed, `left: "Err(PostAlonzo(RedeemerMissing))"` |
| pr9-conway.rs.computed-ref-hash | e90bfa9b | pallas-validate/src/phase1/conway.rs | `--features unstable --lib native_reference` | 1 failed, 9 passed, `left: "Err(PostAlonzo(ScriptWitnessMissing))"` |
| pr9-dijkstra.rs.lint | 627ccd7e | pallas-validate/src/phase1/dijkstra.rs | clippy with `--features unstable` | exit 101, clone on `u64`. Workspace and phase2 clippy exit 0 |
| pr10-native_evaluator.rs.lint | d1a26f65 | pallas-validate/src/phase2/native_evaluator.rs | clippy with `--features phase2,unstable` | exit 101, clone on `i64`. Workspace, phase2 and unstable clippy exit 0 |

The red trees for F1 to F6 are the branches repro/conway-validate-f1-red, f2-red, f3-red, f4-red, f5-red and f6-red. red/ holds the sources they were built from.

## Apparatus

- arms-tip/run_arms_tip.sh reruns the blind lens arms, expected lines in its header.
- mut-tip holds the mutant sources at db1e9cc0. `TREE=<checkout of db1e9cc0> sh mut-tip/install.sh`, then on the box `arm.sh ARM` per arm and `REACH=1 arm.sh none` for reach counts. Expected: arms-summary-tip.txt and arms-killers-tip.txt (30 arms, 29 killed, I2 survives).
- box-logs holds every box log of the run.
