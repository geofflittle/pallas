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

Kinds: `tx` (mempool transaction), `body`, `wits`, `ppu` (protocol parameter update), `block`, `eb` (endorser block body).

Sources:
- `arb`: the ledger QuickCheck `Arbitrary` instances.
- `cddl-plain`: cuddle generation over the ledger Huddle spec `dijkstraCDDL`, definite lengths only.
- `cddl`: the same with cuddle's indefinite length encodings.
- `mutate`: `arb` values changed by `mutate.py`, to count the inputs pallas accepts and the ledger refuses.
- `empty14`: `arb` values with an empty guards set at body key 14.
- `subkeyhash`: `arb` values with a key hash guard in a sub transaction body.
- `given`: generates nothing and reads `OUT/KIND.given.gen.jsonl`, written by `given.py KIND FILES` from transaction corpora, block files and endorser block bodies.
- `cuddle-cli-plain`, `cuddle-cli`: the cuddle 1.8.1.0 command line tool over the ledger CDDL file, through `gen-cuddle-cli.sh`.

`valuecmp.py` compares the pallas value and re-encoding with the ledger's. `schemacmp.py` compares each schema field with the ledger value it maps, by the path tables in `schemas.py`: every bool, the transaction hash against the ledger transaction id, and a rational too wide for 32 bits only as its own leaf and only when the schema holds the nearest value that fits. With `SCHEMA_HELD` set it also writes the agreeing leaves per value, which the coverage check reads. It also compares the redeemer the v1 mapper attaches to each input, withdrawal and certificate with the one the ledger's order of that field names.

A difference `expected.py` names is counted as listed, not as a disagreement.

## Tier

`tier.sh OUT N SEED` runs `vrun.sh` over each kind and source pair of `PAIRS`, `JOBS` (default 4) at a time, the given values read from the files `TX_FILES`, `BLOCK_FILES` and `EB_FILES` name, then `vtally.py OUT` prints one line per comparer and the count of each entry of `expected.py`, and exits 1 on any disagreement, any entry that matched no value and any value listed under a class `expected.py` does not hold.

`tier_test.sh DIR` checks the tier's concurrency, its exit on a failed pair and that it generates the arb values of a kind once, with a stub `vrun.sh` and a `docker` that logs each generator call.

## Reproductions

`repro/run-repros.sh` runs the value and schema comparisons over `ppu-indef-rational`, `voting-null-anchor`, `golden-tx` and `peras-cert-block` (a block whose Peras certificate the ledger reads). `PROBE` names the probe, and a probe that refuses every input (`plants/refuse-all-probe.py`) exits 1 with `pallas refuses ledger-valid input`.

## Coverage

`coverage.sh NAME REV PROBE` runs the `w42_coverage` test of the `u5c-chain-oracle` example at pallas revision REV over the w42 sample blocks, the synthetic blocks and the generated blocks in `coverage/blocks` (or `BLOCKS`), then runs `vrun.sh` over the same blocks with the source `given`, then `check_coverage_values.py` requires each location the test saw to have agreeing values in every block that holds it.

## End to end

`e2e-run.sh GRPC OUT` walks a utxorpc v1 server by `DumpHistory` to `NODE_TIP` (the node's tip, `slot:hash`), runs `e2e/ebcheck.py` over the inlined endorser transactions of each certifying block, and compares every served value with the ledger's reading of the node's block bytes (`e2e_values.py`). The comparison links each header to the block before by the ledger's hashes, each body to its header body hash, and the served transaction hash to the ledger transaction id, and fails when a walk counter is short of its total. `EB_SAMPLE="SEED COUNT"` compares a seeded sample of endorser positions per certifying block, always with positions 0, 1 and the last. `MAXBLOCKS` stops the walk early. `ORACLE` is the ledger command, taking the kind as its last argument.

Plants of the comparer: `PLANT=slim`, `bls` or `eb` changes one served value before the comparison, and `e2e-plants.sh` runs them over a walk prefix cut by `cut_prefix.py`. Lies of the server: `GRPCURL=e2e/liar.py` with `LIE=midtx`, `txhash`, `droptx`, `body` or `ebbytes` and `REAL_GRPCURL` changes one served value in the walk itself (see the modes in `e2e/liar.py`).

## Pieces

- `images/conformance-gen-hs/Dockerfile`: the generator image, GHC 9.6.7 with the IntersectMBO libsodium with its VRF functions, secp256k1 v0.3.2 and blst v0.3.14, each at a commit, the base at a digest and the Debian packages at a snapshot. The oracle binary needs this image at run time.
- `images/pallas-ci/Dockerfile`: the pallas CI image, Rust 1.98.0 and 1.88 with rustfmt and clippy.
- `image-name.sh NAME [DIR]` prints the name of the image of `images/NAME`, or of `DIR/NAME`, tagged with the hash of its files, and `build-image.sh NAME [DIR]` builds it when docker has none of that name. Dolos names its build image with these two scripts at its harness commit. `image_test.sh` also checks that no file of the checkout runs one of these images by another tag.
- `oracle/`: the Haskell program `conformance-gen` (`gen KIND SOURCE N SEED`, `oracle KIND` over hex lines), built against the cardano-ledger commit of `ledger.rev` (the ledger pinned by node 11.2.0.164).
- `generator-name.sh` prints the generator's name, the hashes of `oracle/` and `build-generator.sh`, the ledger commit and the image's tag.
- `probe-value/`: the Rust program `pallas-probe-value KIND` over the generator's JSON lines, built against the pallas tree at `$W/conf` by `plants/build-probes.sh NAME=REV...`.
- `check_*.py`: self checks of the comparers, the mutation modes and the expected difference table, each run with no arguments. `e2e/check_dial_retry.py` checks that a server call is repeated only after a failed dial. `check_docker_logs.py` checks that every `docker run` line of the scripts sets `--log-driver none`, so a long run writes no container log to the host disk.
- `bin/`: `conformance-gen` and the probes. `bin-cuddle/cuddle`.

## Rebuild

A rebuild of the oracle takes about 40 minutes, the probe about 2 minutes.

```
gen=$(sh gen/build-generator.sh $W/ledger-work $W/generators)
cp "$gen" $W/gen/bin/conformance-gen
W=$W sh gen/plants/build-probes.sh base=HEAD_REV
```

`build-generator.sh WORK OUTDIR` builds the image when docker has none, checks the ledger commit out in `WORK/cardano-ledger`, builds `oracle/` there with the cabal dir `WORK/cabal-dir` (or a third argument) in the image held to 4g, and writes the binary to `OUTDIR` under the name `generator-name.sh` prints. `image_test.sh DIR` checks the names and the image build with a docker that logs each call.
