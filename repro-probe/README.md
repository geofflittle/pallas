# Endorser block layer probes

`record.py` reads the three endorser block fixtures with a standalone CBOR
walker and Python's hashlib, without pallas, and prints the values the tests
assert: body length and hash, entry count, `.ebtxs` line count, first key,
first size, and first transaction id.

    python3 repro-probe/record.py test_data | diff - repro-probe/expected.txt && echo IDENTICAL

Expected verdict line: `IDENTICAL`.

`run.sh`, `job.sh` and `wait.sh` are the box runners used for the cargo runs,
installed at `/opt/build/pallas-eb` on musashi-aws. `SRC=<tree>` picks the
source tree. Touch the tree immediately before each run, because every tree
mounts at `/pallas` and shares one target directory, so an older touch replays
stale artifacts.

The arms, each a `repro/` branch in this clone, with the command and the
verdict it produced on 2026-09-22:

| branch | command | verdict |
| --- | --- | --- |
| repro/eb-primitives-red-no-alias | `cargo test -p pallas-primitives --features unstable endorser` | E0425 cannot find type `EndorserBlock` |
| repro/eb-codec-pairs-reversed | `cargo test -p pallas-primitives --features unstable dijkstra::endorser` | FAILED 3 passed 2 failed, first key left 2026b0b6 right 455a00b5 on dijkstra-eb2 |
| repro/eb-traverse-txs-reversed | `cargo test -p pallas-traverse --features unstable leios::` | FAILED 23 passed 3 failed, dijkstra-eb2 first transaction id |
| repro/eb-source-on-800-accessor-panic | `cargo test -p pallas-traverse --features unstable leios::` | FAILED 24 passed 2 failed, inputs and outputs not yet implemented for Dijkstra |
