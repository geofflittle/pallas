# Reference comparison

Each layer's tests compare pallas with its reference on the inputs in `records/`, through the outcome and tally of the `ledger-oracle-judge` crate's `comparison` module.

- `records/LAYER.jsonl`: a header naming the reference pin, the program that wrote the records, the layer and its input classes, then one input per line with the reference's answer.
- `overlay/CRATE/tests/`: the integration tests of each crate.
- `overlay.sh HEAD SOURCE CRATE...`: copies the tests and records into a pallas tree and adds the comparison crate as a dev-dependency.
- `run.sh HEAD SOURCE LAYER`: overlays one layer's tests on a pallas tree and runs them.

A test refuses records whose pin differs from the one the head's `pallas-primitives/src/dijkstra/defs.cddl` names, and fails on any disagreement, panic or declared class with no judged input. An allowed difference names the layer and the test that own it.

```
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV decode
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV validation
```
