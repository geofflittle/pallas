# Conformance value oracle

The ledger value oracle, the pallas value walker, the comparers, the plants and breaks that each must fail them, and the results of the 1000 per kind run and the whole chain end to end run.

The box layout these runners expect is /opt/build/value-oracle-1025 on the build host, with `gen/` as below, `run.sh` from `box/` at its root, the pallas tree under `conf/` and a scratch utxorpc server for the end to end run.

## Layout

- `gen/`: the generator, the Haskell oracle (`oracle/`, image `conformance-gen-hs:2`), the walker (`probe-value/`), the comparers (`value.py`, `valuecmp.py`, `schemacmp.py`, `schemas.py`), the runners (`vrun.sh`, `recmp.sh`, `pin.sh`, `coverage.sh`, `e2e-plants.sh`), the end to end comparer (`e2e_values.py`, `e2e/served.py`, `e2e/ebcheck.py`), the repro set (`repro/`) and the probe plants (`plants/`).
- `breaks/`: patches and old copies that break one comparer rule each, and `run-breaks.sh`.
- `box/`: the container wrappers used on the build host.
- `results/`: the logs and counts of the runs named in the report.

## Runners and expected verdicts

Comparer checks, from `gen/`, each exits 0:

```
python3 -I check_rows.py      aligned rows: 4 cases, 0 wrong
python3 -I check_sets.py      set repeats: 5 cases, 0 wrong
python3 -I check_align.py     oset alignment: 5 cases, 0 wrong
python3 -I check_listing.py   tag 24 listing: 5 cases, 0 wrong
```

Breaks, `sh breaks/run-breaks.sh`, each line ends in exit 1:

```
aligned rows: 4 cases, 2 wrong        rows-value.py.patch
aligned rows: 4 cases, 1 wrong        rows-empty-value.py.patch
set repeats: 5 cases, 2 wrong         sets-schemacmp.py.patch
oset alignment: 5 cases, 2 wrong      align-schemacmp.py.patch
tag 24 listing: 5 cases, 1 wrong      tag24-valuecmp.py.patch
```

Generated value count, on the box from `gen/` with a GEN_FROM dir holding 3 tx arb values:

```
OUT=pin/cnt-3 GEN_FROM=pin/trunc sh vrun.sh 3 1 tx arb     exit 0
OUT=pin/cnt-5 GEN_FROM=pin/trunc sh vrun.sh 5 1 tx arb     tx arb: 3 generated values where 5 were asked, exit 1
```

`breaks/vrun-no-count.sh.patch` applied to `vrun.sh` gives exit 0 on the second line.

Comparer pinning, from the box root, where `pin/old` holds `breaks/pin-old`:

```
sh gen/pin.sh pin/old gen gen/vsmall2      LOGS IDENTICAL over 20 value lines, exit 0
sh gen/pin.sh pin/old gen vsmall2          NO VALUES compared in vsmall2, exit 2
sh gen/recmp.sh pin/emptyset               no probe output in pin/emptyset, exit 2
```

`breaks/pin-before-count.sh` and `breaks/recmp-before-count.sh` on the last two lines exit 0.

Repros, `sh repro/run-repros.sh`: `repros: exit 0`. With `PROBE=plants/refuse-all-probe.py`: `DISAGREE  pallas refuses ledger-valid input  planted refusal`, exit 1. With the walker built from pallas `repro/plant-pool-pledge-cost-swap`: `DISAGREE  v1 value differs  body.certs[].cost`, exit 1.

Generator plants, `PROBE=bin/pallas-probe-value-NAME GEN_FROM=vsmall sh vrun.sh 20 1`, each exit 1: `stake-credential-swap` gives `DISAGREE  value differs  body.certs[][][]`, `plutus-integer-wrap` gives `DISAGREE  v1beta value differs  wits.redeemers{}.v.data.v`.

Coverage, `sh gen/coverage.sh NAME BRANCH`: `repro/conformance-value-coverage` gives `AGREES true`, `repro/plant-bls-zero-new-harness` and `repro/plant-bls-spurious-new-harness` give `AGREES false` and exit 101.

End to end prefix, `sh gen/e2e-plants.sh full prefix 9200` from `e2e/`: base exit 0, slim, bls and eb each exit 1 (results/prefix.*.txt).

Value run summary, from this directory:

```
python3 -I gen/vsummary.py results/vrun1000/vrun1000r.log results/vrun1000     summary lines with a disagreement or a differing leaf: 0, exit 0
```

A copy of the log with one summary line changed to `disagreements 1 on 1 values`, or to `leaves differ 2,`, gives exit 1.
