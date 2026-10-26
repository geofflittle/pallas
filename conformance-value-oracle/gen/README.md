# Dijkstra conformance generator

Generates Dijkstra CBOR values, decodes each with the Haskell ledger and with pallas, and compares the ledger reading with the pallas decode, the pallas re-encoding and the v1alpha, v1beta and v1 utxorpc mappers. The same comparers judge the minimal reproductions, the coverage blocks and every block a utxorpc v1 server serves.

## Rerun

Every command runs from the box dir `W=/opt/build/harness-gaps-1026`, with the harness in `$W/gen`.

```
OUT=$W/gen/vout PROBE=$W/gen/bin/pallas-probe-value sh gen/vrun.sh 1000 1 tx   # one kind, sources arb cddl-plain cddl mutate
PROBE=$W/gen/bin/pallas-probe-value sh gen/repro/run-repros.sh              # the minimal reproductions
sh gen/coverage.sh NAME REV PROBE                                           # coverage blocks, then their values
NODE_TIP=slot:hash ORACLE="CMD oracle" sh gen/e2e-run.sh GRPC OUT           # every served block against the ledger
```

Each command prints its verdict lines and exits 1 on any disagreement.

## Value run

`vrun.sh N SEED [KINDS] [SOURCES]` writes `OUT/KIND.SOURCE.*`: the generated values, the ledger results, the probe output, the ledger results of the re-encoding, the counts and one disagreement record per line in `*.dis.jsonl` and `*.schema.dis.jsonl`. `GEN_FROM=dir` reuses the values and ledger results of an earlier run, so a planted probe differs from the base run only in the probe. Generation is deterministic for a seed. A kind and source with other than N values exits 1.

Kinds: `tx` (mempool transaction), `body`, `wits`, `ppu` (protocol parameter update), `block`.

Sources:
- `arb`: the ledger QuickCheck `Arbitrary` instances.
- `cddl-plain`: cuddle generation over the ledger Huddle spec `dijkstraCDDL`, definite lengths only.
- `cddl`: the same with cuddle's indefinite length encodings.
- `mutate`: `arb` values changed by `mutate.py`, to count the inputs pallas accepts and the ledger refuses.
- `given`: generates nothing and reads `OUT/KIND.given.gen.jsonl`, the coverage blocks.
- `cuddle-cli-plain`, `cuddle-cli`: the cuddle 1.8.1.0 command line tool over the ledger CDDL file, through `gen-cuddle-cli.sh`.

`valuecmp.py` compares the pallas value and re-encoding with the ledger's. `schemacmp.py` compares each schema field with the ledger value it maps, by the path tables in `schemas.py`: every bool, the transaction hash against the ledger transaction id, and a rational too wide for 32 bits only as its own leaf and only when the schema holds the nearest value that fits. With `SCHEMA_HELD` set it also writes the agreeing leaves per value, which the coverage check reads.

## Reproductions

`repro/run-repros.sh` runs the value and schema comparisons over `ppu-indef-rational`, `voting-null-anchor`, `golden-tx` and `peras-cert-block` (a block whose Peras certificate the ledger reads). `PROBE` names the probe, and a probe that refuses every input (`plants/refuse-all-probe.py`) exits 1 with `pallas refuses ledger-valid input`.

## Coverage

`coverage.sh NAME REV PROBE` runs the `w42_coverage` test of the `u5c-chain-oracle` example at pallas revision REV over the w42 sample blocks, the synthetic blocks and the generated blocks in `coverage/blocks` (or `BLOCKS`), then runs `vrun.sh` over the same blocks with the source `given`, then `check_coverage_values.py` requires each location the test saw to have agreeing values in every block that holds it.

## End to end

`e2e-run.sh GRPC OUT` walks a utxorpc v1 server by `DumpHistory` to `NODE_TIP` (the node's tip, `slot:hash`), runs `e2e/ebcheck.py` over the inlined endorser transactions of each certifying block, and compares every served value with the ledger's reading of the node's block bytes (`e2e_values.py`). The comparison links each header to the block before by the ledger's hashes, each body to its header body hash, and the served transaction hash to the ledger transaction id, and fails when a walk counter is short of its total. `EB_SAMPLE="SEED COUNT"` compares a seeded sample of endorser positions per certifying block, always with positions 0, 1 and the last. `MAXBLOCKS` stops the walk early. `ORACLE` is the ledger command, taking the kind as its last argument.

Plants of the comparer: `PLANT=slim`, `bls` or `eb` changes one served value before the comparison, and `e2e-plants.sh` runs them over a walk prefix cut by `cut_prefix.py`. Lies of the server: `GRPCURL=e2e/liar.py` with `LIE=midtx`, `txhash`, `droptx`, `body` or `ebbytes` and `REAL_GRPCURL` changes one served value in the walk itself (see the modes in `e2e/liar.py`).

## Pieces

- `docker/Dockerfile`: image `conformance-gen-hs:2`, GHC 9.6.7 with the IntersectMBO libsodium with its VRF functions, secp256k1 v0.3.2 and blst v0.3.14. The oracle binary needs this image at run time.
- `oracle/`: the Haskell program `conformance-gen` (`gen KIND SOURCE N SEED`, `oracle KIND` over hex lines), built inside `cardano-ledger/` at 9df9f98c (the ledger pinned by node 11.2.0.164) with `cabal.project.local` holding `packages: /work/oracle`.
- `probe-value/`: the Rust program `pallas-probe-value KIND` over the generator's JSON lines, built against the pallas tree at `$W/conf` by `plants/build-probes.sh NAME=REV...`.
- `check_*.py`: self checks of the comparers, each run with no arguments.
- `bin/`: `conformance-gen` and the probes. `bin-cuddle/cuddle`.

## Rebuild

A rebuild of the oracle takes about 40 minutes, the probe about 2 minutes.

```
docker build -t conformance-gen-hs:2 $W/gen/docker
H="docker run --rm --user $(id -u):$(id -g) -v $W/gen:/work -e CABAL_DIR=/work/cabal-dir -e HOME=/work -w /work/cardano-ledger conformance-gen-hs:2"
$H cabal update
$H cabal build -j3 conformance-gen
cp "$W/gen/cardano-ledger/$($H cabal list-bin conformance-gen | sed 's|^/work/cardano-ledger/||')" $W/gen/bin/conformance-gen
W=$W sh gen/plants/build-probes.sh base=HEAD_REV
```
