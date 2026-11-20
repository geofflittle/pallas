# Conformance value oracle

The ledger value oracle, the schema comparer, the end to end comparer, the breaks that each must fail them, and the results of the 1000 per kind run and the whole chain end to end run. The decode layer of `reference-comparison/` compares the pallas value and re-encoding with the ledger's.

The box layout these runners expect is /opt/build/value-oracle-1025 on the build host, with `gen/` as below, the pallas tree under `conf/` and a scratch utxorpc server for the end to end run.

## Layout

- `gen/`: the generator, the Haskell oracle (`oracle/`, its image in `images/`), the walker (`probe-value/`), the schema comparer (`schemacmp.py`, with `value.py`), the end to end runner and comparer (`e2e-run.sh`, `e2e-plants.sh`, `e2e_values.py`, `e2e/served.py`, `e2e/ebcheck.py`) and the repro inputs (`repro/`).
- `breaks/`: patches that break one comparer rule each, and `run-breaks.sh`.
- `results/`: the logs and counts of the runs named in the report.

## Runners and expected verdicts

Comparer checks, from `gen/`, each exits 0:

```
python3 -I check_rows.py      aligned rows: 4 cases, 0 wrong
python3 -I check_sets.py      set repeats: 5 cases, 0 wrong
python3 -I check_align.py     oset alignment: 5 cases, 0 wrong
```

Breaks, `sh breaks/run-breaks.sh`, each line ends in exit 1:

```
aligned rows: 4 cases, 2 wrong        rows-value.py.patch
aligned rows: 4 cases, 1 wrong        rows-empty-value.py.patch
set repeats: 5 cases, 2 wrong         sets-schemacmp.py.patch
oset alignment: 5 cases, 2 wrong      align-schemacmp.py.patch
```

End to end prefix, `sh gen/e2e-plants.sh full prefix 9200` from `e2e/`: base exit 0, slim, bls and eb each exit 1 (results/prefix.*.txt).
