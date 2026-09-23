# Endorser block final amend: runners and expected verdicts

Run from a checkout of each branch, with the box alias `musashi-aws`.

    sh repro-runners/eb-final/sync.sh <tree> /opt/build/wave2-eb/brk
    sh repro-runners/eb-final/boxrun.sh /opt/build/wave2-eb/brk <log> <cargo args>

| branch | cargo args | expected verdict line |
|---|---|---|
| repro/eb-final-from-red | test -p pallas-primitives --features unstable --lib dijkstra::tests::a_mempool_transaction | error: could not compile `pallas-primitives` (lib test) due to 3 previous errors, each E0277 From<MempoolTransaction> not implemented |
| repro/eb-final-from-copies-aux | test -p pallas-primitives --features unstable --lib dijkstra:: | test result: FAILED. 34 passed; 1 failed (only the third address differs) |
| repro/eb-final-from-copies-body-witnesses | test -p pallas-primitives --features unstable --lib dijkstra:: | test result: FAILED. 34 passed; 1 failed (first two addresses differ, third equal) |
| repro/eb-final-alias-any-length-map | test -p pallas-codec -p pallas-primitives -p pallas-traverse --features unstable --no-fail-fast | primitives: test result: FAILED. 143 passed; 1 failed, only an_indefinite_body_is_refused |
| repro/eb-final-pre-amend | check -p pallas-primitives -p pallas-traverse --features unstable,relaxed --all-targets | warning: function `one_entry_with_key_of` is never used |

The logs beside this file are the real output of each run on 2026-09-23.
