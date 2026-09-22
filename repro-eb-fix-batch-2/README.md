# Runners for the endorser block fix batch 2

`box-test.sh <ref> <label> <cargo args>` puts the tree of `<ref>` on musashi-aws
under /opt/build/pallas-eb/src-<label> and runs the cargo command in the
pallas-ci:1 container through `box-job.sh` and `box-wait.sh` (copies of the box
harness scripts `job.sh`, `wait.sh` and `run.sh` in /opt/build/pallas-eb).

Planted defects and the verdict each must give with
`box-test.sh <branch> <label> cargo test -p pallas-traverse --features unstable leios`:

| branch | expected |
| --- | --- |
| repro/eb-resolve-unbound-red | 29 passed, 1 failed, the body the parent did not announce is resolved |
| repro/eb-envelope-trailing-red | 29 passed, 1 failed, "a transaction with bytes after it must be refused" |
| repro/eb-resolve-skips-announcement | 32 passed, 1 failed, a_body_the_certified_announcement_does_not_name_is_refused, "a body of another length must be refused" |
| repro/eb-closure-tx-trailing-accepted | 31 passed, 2 failed, both trailing byte tests |
| repro/eb-resolve-skips-parent | 32 passed, 1 failed, an_announcement_the_blocks_parent_did_not_make_is_refused |
| repro/eb-announced-size-only | 32 passed, 1 failed, "wrong refusal: transaction 243 is 201 bytes" |

`plants.log` holds the recorded output. `harvest-run.sh` is the container runner
used under /opt/build/pallas-eb-harvest for the leios-fetch harvest, and
`fetch.log` its output. `txid-envelopes.py` unwraps an `.ebtxs` file into
transaction files for `cardano-cli dijkstra transaction txid`, with `txids.log`
its output. `ebentries.py` reads an `.ebbody` without pallas.
