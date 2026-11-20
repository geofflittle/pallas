# Dijkstra conformance generator

Generates Dijkstra CBOR values, decodes each with the Haskell ledger and with pallas, and compares the ledger reading with the v1alpha, v1beta and v1 utxorpc mappers. The same comparers judge every block a utxorpc v1 server serves.

## Rerun

Every command runs from a work dir `W` with the harness in `$W/gen`.

```
NODE_TIP=slot:hash ORACLE="CMD oracle" sh gen/e2e-run.sh GRPC OUT           # every served block against the ledger
```

The command prints its verdict lines and exits 1 on any disagreement.

Kinds: `tx` (mempool transaction), `body`, `wits`, `ppu` (protocol parameter update), `block`, `eb` (endorser block body).

`schemacmp.py` compares each schema field with the ledger value it maps: every bool, the transaction hash against the ledger transaction id, and a rational too wide for 32 bits only as its own leaf and only when the schema holds the nearest value that fits. It also compares the redeemer the v1 mapper attaches to each input, withdrawal and certificate with the one the ledger's order of that field names.

## End to end

`e2e-run.sh GRPC OUT` walks a utxorpc v1 server by `DumpHistory` to `NODE_TIP` (the node's tip, `slot:hash`), runs `e2e/ebcheck.py` over the inlined endorser transactions of each certifying block, and compares every served value with the ledger's reading of the node's block bytes (`e2e_values.py`). The comparison links each header to the block before by the ledger's hashes, each body to its header body hash, and the served transaction hash to the ledger transaction id, and fails when a walk counter is short of its total. `EB_SAMPLE="SEED COUNT"` compares a seeded sample of endorser positions per certifying block, always with positions 0, 1 and the last. `MAXBLOCKS` stops the walk early. `ORACLE` is the ledger command, taking the kind as its last argument.

Plants of the comparer: `PLANT=slim`, `bls` or `eb` changes one served value before the comparison, and `e2e-plants.sh` runs them over a walk prefix cut by `cut_prefix.py`. Lies of the server: `GRPCURL=e2e/liar.py` with `LIE=midtx`, `txhash`, `droptx`, `body` or `ebbytes` and `REAL_GRPCURL` changes one served value in the walk itself (see the modes in `e2e/liar.py`).

## Pieces

- `images/conformance-gen-hs/Dockerfile`: the generator image, GHC 9.6.7 with the IntersectMBO libsodium with its VRF functions, secp256k1 v0.3.2 and blst v0.3.14, each at a commit, the base at a digest and the Debian packages at a snapshot. The oracle binary needs this image at run time.
- `images/pallas-ci/Dockerfile`: the pallas CI image, Rust 1.98.0 and 1.88 with rustfmt and clippy, and Python with cbor2 for `coverage/synth.py`.
- `image-name.sh NAME [DIR]` prints the name of the image of `images/NAME`, or of `DIR/NAME`, tagged with the hash of its files, and `build-image.sh NAME [DIR]` builds it when docker has none of that name.
- `oracle/`: the Haskell program `conformance-gen` (`gen KIND SOURCE N SEED`, `oracle KIND` over hex lines), built against the cardano-ledger commit of `ledger.rev` (the ledger pinned by node 11.2.0.164).
- `generator-name.sh` prints the generator's name, the hashes of `oracle/` and `build-generator.sh`, the ledger commit and the image's tag.
- `probe-value/`: the Rust program `pallas-probe-value KIND` over the generator's JSON lines.
- `check_*.py`: self checks of the comparers, each run with no arguments. `e2e/check_dial_retry.py` checks that a server call is repeated only after a failed dial. `check_docker_logs.py` checks that every `docker run` line of the scripts sets `--log-driver none`, so a long run writes no container log to the host disk.
- `bin/`: `conformance-gen` and the probes. `bin-cuddle/cuddle`.

## Rebuild

A rebuild of the oracle takes about 40 minutes.

```
gen=$(sh gen/build-generator.sh $W/ledger-work $W/generators)
cp "$gen" $W/gen/bin/conformance-gen
```

`build-generator.sh WORK OUTDIR` builds the image when docker has none, checks the ledger commit out in `WORK/cardano-ledger`, builds `oracle/` there with the cabal dir `WORK/cabal-dir` (or a third argument) in the image held to 4g, and writes the binary to `OUTDIR` under the name `generator-name.sh` prints.
