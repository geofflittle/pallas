# Reference comparison

Each layer's tests compare pallas with its reference on the inputs in `records/`, through the outcome and tally of the `ledger-oracle-judge` crate's `comparison` module.

- `records/PIN/LAYER.jsonl`: a header naming the reference pin, the program that wrote the records, the layer and its input classes, then one input per line with the reference's answer. The Dolos reference writer writes them, built at the pin by `oracle/build-writer.sh PIN WORK OUT`.
- `overlay/CRATE/tests/`: the integration tests of each crate.
- `overlay.sh HEAD SOURCE CRATE...`: copies the tests and records into a pallas tree and adds the comparison crate as a dev-dependency.
- `run.sh HEAD SOURCE LAYER`: overlays one layer's tests on a pallas tree and runs them.
- `run.sh records SOURCE DIR LAYER [KIND...]`: checks the records of LAYER in DIR, a records directory named by its pin, before any test reads them. It fails on records with no rows, a declared class with no row, a row of a class not declared, a target kind with a target no row makes, a KIND the records name neither as a kind nor as a class, or an accepted decode row with no `encoding`.

An accepted row's reference is `{"accepts": DUMP, "encoding": HEX}`, where HEX is the reference's own encoding of the value it read. Decode rows carry it, and a refused row carries none.

A test refuses records whose pin differs from the one the head's `pallas-primitives/src/dijkstra/defs.cddl` names, and fails on any disagreement, panic or declared class with no judged input. An allowed difference names the layer and the test that own it.

```
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV decode
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV validation
```
