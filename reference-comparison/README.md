# Reference comparison

Each layer's tests compare pallas with its reference on the inputs in `records/`. A layer implements the `ComparedLayer` trait of the `ledger-oracle-judge` crate's `comparison` module, and its `compare` runs pallas on each input under the layer's time limit and judges and counts each observation by its class and the layer's axis.

- `records/PIN/LAYER.jsonl`: a header naming the reference pin, the program that wrote the records, the layer and its input classes, then one input per line with the reference's answer. The Dolos reference writer writes them, built at the pin by `oracle/build-writer.sh PIN WORK OUT`.
- `overlay/CRATE/tests/`: the integration tests of each crate.
- `overlay.sh HEAD SOURCE CRATE...`: copies the tests and records into a pallas tree and adds the comparison crate as a dev-dependency.
- `run.sh HEAD SOURCE LAYER [KIND...]`: overlays one layer's tests on a pallas tree and runs them, on the inputs whose class or kind of change is a KIND when any is given.
- `run.sh records SOURCE DIR LAYER [KIND...]`: checks the records of LAYER in DIR, a records directory named by its pin, before any test reads them. It fails on records with no rows, a declared class with no row, a row of a class not declared, a target kind with a target no row makes, a KIND the records name neither as a kind nor as a class, or an accepted decode row with no `encoding`.

An accepted row's reference is `{"accepts": DUMP, "encoding": HEX}`, where HEX is the reference's own encoding of the value it read. Decode rows carry it, and a refused row carries none.

A test refuses records whose pin differs from the one the head's `pallas-primitives/src/dijkstra/defs.cddl` names. It fails on no judged observation, a declared class with none, a KIND with no input, a record class not declared, and any disagreement, panic, timeout or observation one side makes and the other does not. An allowed difference names what owns it and passes only when pallas's answer shows its reason.

```
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV decode
sh reference-comparison/run.sh HEAD git=https://github.com/geofflittle/dolos@REV validation
```
