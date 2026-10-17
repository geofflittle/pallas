# Conway validate finish runners

Arms and runners for the 2026-10-16 finish build of `fix/validate-conway-deposits-witnesses-collateral` (final head a136e42a). Report: `.claude/docs/2026-10-16_conway-validate-finish-build.md`.

## Setup on the box

```
ssh musashi-aws mkdir -p /opt/build/conway-finish-1016/arms /opt/build/conway-finish-1016/logs
scp run.sh job.sh arm.sh musashi-aws:/opt/build/conway-finish-1016/
scp arms/*.patch musashi-aws:/opt/build/conway-finish-1016/arms/
./sync.sh head <worktree at a136e42a>
./sync.sh base <worktree at dbdadb22>
```

`sync.sh` copies a worktree (uncommitted edits included) to `/opt/build/conway-finish-1016/<W>`, writes `HEAD_SHA` and touches every file. `arm.sh SRC PATCH FILTER` copies that tree to `arm`, applies one patch (`none` for the control), touches pallas-validate and runs `cargo test -p pallas-validate --features phase2 -- FILTER`.

## Expected verdicts

On `head` (a136e42a), `./arm.sh head <patch> ""`, recorded in `arms-head.out` and `arms-head-rows.out`:

| patch | exit | failing test and assertion |
|---|---|---|
| none | 0 | totals passed=154 failed=0 |
| A15m-key-deposit-zero | 101 | deposits_are_required_through_dispatch, key row [Pres, Pres, Pres] |
| A16m-drep-deposit-zero | 101 | deposits_are_required_through_dispatch, drep row [Pres, Pres, Pres] |
| A17m-gov-deposit-zero | 101 | deposits_are_required_through_dispatch, proposal row [Pres, Pres, Pres] |
| A18m-drep-gov-swapped | 101 | deposits_are_required_through_dispatch, drep and proposal rows |
| K-cutoff-eq-10 | 101 | v3_deposit_context_protocol_boundaries_at_every_occurrence, "certificate list protocol 11" |
| DF1-txinfo-default-10 | 101 | public_encoding_is_the_protocol_9_encoding, "tx info" |
| DF2-context-default-10 | 101 | public_encoding_is_the_protocol_9_encoding, "script context" |

On `base` (dbdadb22), the gap the new tests close: `K-cutoff-eq-10`, `DF1-txinfo-default-10` and `DF2-context-default-10` each print `exit=0` and `totals passed=153 failed=0`.

## Ledger probe

`./ledger.sh` (needs `gh`) prints the cardano-ledger lines at 178bd368 that decide the cutoff, recorded in `ledger.out`.
