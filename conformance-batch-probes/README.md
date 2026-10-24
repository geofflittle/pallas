# Conformance batch probes, 2026-10-24

Probes, fixture sources and harness scripts behind the conformance batch report
(sundae-v4 `.claude/docs/2026-10-24_conformance-pallas-batch.md`). Every runner
below runs on the build box. The ledger oracle is the `conformance-gen-hs:2`
image with `/opt/build/conformance-gen-1024` mounted at `/work`.

## box/

The harness copied to `/opt/build/pallas-conformance-1024`.

- `run.sh TREE cargo ...` runs cargo in the `pallas-ci:1` image over one tree, target `/w/target` unless `TARGET` is set.
- `checks.sh TREE BP STEP...` runs the named checks (fmt, three-unstable, three, workspace, workspace-unstable, clippy, clippy-unstable, blueprint, blueprint-primitives, nodefault-primitives, msrv), all when no step is named, and prints one `STEP exit N passed P failed F` line each. `U` overrides the unstable feature list. It touches the tree's sources first, since cargo replays a binary built from another tree that shares the target.
- `regen-m1.sh DEST`, `regen-m2.sh`, `regen-flat.sh`, `regen-d2.sh` rebuild the generated fixtures of each branch into `DEST` from `compose*.json` and `manifest-*.txt`. Expected: `diff -r DEST TREE/test_data/dijkstra-generated` prints nothing for the files the manifest names.
- `*_candidates.py`, `pick.py`, `cutitem.py`, `flat_items.py`, `frame.py`, `extract.py` choose and cut generator values for those fixtures.

## probe-merge/

The pallas side of the done run, built against the merge tree. `run.sh 1000 1`
in `/opt/build/conformance-gen-1024` with `PROBE` set to its binary writes one
`*.summary.txt` per kind and source.

## probe-schemas/, done-run/ (2026-10-25)

`probe-schemas` maps every value pallas decodes through the v1alpha, v1beta
and v1 utxorpc mappers with `unstable` on, and writes a `schemas` field of
`{ok, facts}` or `{ok, err}` per schema beside the v1 fields `compare.py`
reads. `done-run/run.sh` is the done run with two additions: `GEN_FROM=DIR`
reuses the generated values and ledger readings of an earlier run, and when
the probe writes `schemas` it runs `schemas.py`, which prints one
`schema S KIND:` line per schema with its panics, errors and disagreements.

```
W=/opt/build/pallas-schemas-1025   # cargo-home and probe-target
docker run --rm --user $(id -u):$(id -g) -v $W:/work -e HOME=/work/home \
  -e CARGO_HOME=/work/cargo-home -e CARGO_TARGET_DIR=/work/probe-target \
  -w /work/probe-schemas pallas-ci:1 cargo build --release
cd /opt/build/conformance-gen-1024
PROBE=bin/pallas-probe-schemas-SHA OUT=out-schemas-X GEN_FROM=out-merge-1000 ./run.sh 1000 1
```

`pallas-tree` next to `probe-schemas` is a symlink to the tree under test.
Expected on 23c3a1ce, from `/usr/bin/grep -h "^schema v1beta tx" out-schemas-before/*.summary.txt`:
`panics 717` (arb), `panics 479` (cddl-plain), `panics 356` (cddl), each
`map_redeemer is not yet implemented for Dijkstra`. Expected on the merge with
the guarding fix: `panics 0` on every `schema` line. `done-run/records/` holds
both runs' summary lines (both with `GEN_FROM=out-merge-1000`) and the
coverage runs.

`box/ci-branches.sh NAME:BRANCH:BP...` extracts each branch of `repo.git` to
`trees/NAME` and runs `checks.sh` on it. `box/coverage.sh NAME BRANCH BP`
runs `w42_coverage` on one branch. Expected: `exit 0`, `64`, `AGREES true` on
`repro/conformance-schemas-coverage`, and `exit 101` with `a location u5c does
not hold is failing` on `repro/conformance-schemas-coverage-planted`.

## probe-split/

Maps generated Dijkstra blocks and transactions with the v1alpha and v1beta
utxorpc mappers, with traverse `unstable` on and utxorpc `unstable` off.

```
TARGET=target-split ./run.sh probe-split cargo build --release
O=/opt/build/conformance-gen-1024/out-merge-100   # run.sh 100 1 output
for k in block tx; do for s in arb cddl-plain cddl; do
  echo "== $k $s"; bin-probe-split $k < $O/$k.$s.gen.jsonl | cut -d' ' -f2- | sort | uniq -c
done; done
```

Expected, split arm: `tx arb` gives `90 v1alpha PANIC not implemented: collect_all_scripts has no arm for this native script` and `10 v1alpha PANIC not implemented: map_script_ref has no arm for this reference script`. With `--features utxorpc-unstable` (the control arm) every v1alpha line is `ok` or `decode error`, and `tx arb` gives `67 v1beta PANIC not implemented: map_redeemer is not yet implemented for Dijkstra`.

## probe-m2/

`sh short-vote.sh PALLAS_TREE` feeds the ledger the governance body as
committed and with its vote written as `8102`. Expected: line 1 `"ok":true`,
line 2 `"ok":false` with `expected list len or indef`. `short-vote.txt` is a
recorded run, `m2-red.txt` the red test run.

## probe-d2/

`sh version-info.sh PALLAS_TREE` feeds the generated block, its two 2^32
mutations and its indefinite form to the ledger. Expected lines are in the
script header (True, False at 398, False at 403, True).
`version-info-inputs.txt` holds the four inputs.

## w42-fixtures/

`sh regen-w42-blocks.sh SAMPLES_RB_DIR PALLAS_TREE` rewrites
`test_data/dijkstra-rc-2026w42-7.block` and `-11.block` from the devnet samples.
Expected: `git status` shows no change in the tree. `blocks.oracle.jsonl` is
the ledger's reading of both blocks.

## fx-m1/, fx-flat/

The generated fixtures as first written, with the ledger facts for each.
