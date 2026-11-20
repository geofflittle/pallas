# Reference comparison

Each layer's tests compare pallas with its reference on the inputs in `records/`, through the outcome and tally of the `ledger-oracle-judge` crate's `comparison` module.

- `records/LAYER.jsonl`: a header naming the reference pin, the program that wrote the records, the layer and its input classes, then one input per line with the reference's answer.
- `overlay/CRATE/tests/`: the integration tests of each crate.
- `overlay.sh HEAD SOURCE CRATE...`: copies the tests and records into a pallas tree and adds the comparison crate as a dev-dependency.

A test refuses records whose pin differs from the one the head's `pallas-primitives/src/dijkstra/defs.cddl` names, and fails on any disagreement, panic or declared class with no judged input.

```
sh reference-comparison/overlay.sh HEAD git=https://github.com/geofflittle/dolos@REV pallas-validate
cd HEAD
RUSTFLAGS="--cfg pallas_dijkstra_evaluator" cargo +nightly-2026-09-04 test -p pallas-validate --features unstable,phase2 --test reference_validation -- --nocapture
```
