# Dijkstra conformance generator

Generates Dijkstra CBOR values, decodes each with the Haskell ledger and with pallas, and compares the ledger reading with the pallas decode, the pallas re-encoding and the pallas v1 utxorpc mapper.

## Rerun

```
cd /opt/build/conformance-gen-1024
./run.sh 100 1                                   # every kind, sources arb cddl-plain cddl, about 4 minutes
./run.sh 100 1 "tx body wits ppu block" "cuddle-cli-plain cuddle-cli"   # the standalone cuddle tool over the CDDL text
sh repro/run-repros.sh                           # the minimal reproductions, each prints its expected verdict
```

Outputs go to `out/KIND.SOURCE.*`. `summary.txt` holds the counts by class, `dis.jsonl` one record per disagreement with the input hex and both sides. Generation is deterministic for a seed (the same seed gives the same sha256 of `gen.jsonl`), so only the summaries and the gzipped disagreement records of the recorded run are kept.

Kinds: `tx` (mempool transaction), `body`, `wits`, `ppu` (protocol parameter update), `block`.

Sources:
- `arb`: the ledger QuickCheck `Arbitrary` instances.
- `cddl-plain`: cuddle generation over the ledger Huddle spec `dijkstraCDDL`, definite lengths only.
- `cddl`: the same with cuddle's indefinite length encodings ("twiddle").
- `cuddle-cli-plain`, `cuddle-cli`: the cuddle 1.8.1.0 command line tool over `cardano-ledger/eras/dijkstra/impl/cddl/data/dijkstra.cddl`, without and with twiddle.

## Pieces

- `docker/Dockerfile`: image `conformance-gen-hs:2`, GHC 9.6.7 with the IntersectMBO libsodium with its VRF functions, secp256k1 v0.3.2 and blst v0.3.14. The oracle binary needs this image at run time.
- `oracle/`: the Haskell program `conformance-gen` (`gen KIND SOURCE N SEED`, `oracle KIND` over hex lines). Built inside `cardano-ledger/` at 9df9f98c (the ledger pinned by node 11.2.0.164) with `cabal.project.local` holding `packages: /work/oracle`.
- `probe/`: the Rust program `pallas-probe KIND` over the generator's JSON lines, built against `pallas/` (detached at 260efea, the merge of seven heads on e7ea1726).
- `probe-strings/`: the same probe against `pallas-strings/`, the merge with `fix/decode-indefinite-length-strings` (0c46a87) on top, recorded as branch `repro/indef-strings-on-merge` in `pallas/`.
- `compare.py`: classes each disagreement. `diffvalue.py`: first path where the pallas re-encoding differs from the input.
- `gen-cuddle-cli.sh`: the standalone cuddle tool as a source.
- `bin/`: `conformance-gen`, `pallas-probe`, `pallas-probe-strings`. `bin-cuddle/cuddle`.

## Rebuild

The cabal store and the cargo caches were deleted. A rebuild of the oracle takes about 40 minutes (ledger dependencies), the probe about 2 minutes.

```
W=/opt/build/conformance-gen-1024
docker build -t conformance-gen-hs:2 $W/docker
H="docker run --rm --user $(id -u):$(id -g) -v $W:/work -e CABAL_DIR=/work/cabal-dir -e HOME=/work -w /work/cardano-ledger conformance-gen-hs:2"
$H cabal update
$H cabal build -j3 conformance-gen
cp "$W/cardano-ledger/$($H cabal list-bin conformance-gen | sed 's|^/work/cardano-ledger/||')" $W/bin/conformance-gen
$H cabal install -j3 cuddle-1.8.1.0 --installdir /work/bin-cuddle --install-method copy --overwrite-policy always

docker run --rm --user $(id -u):$(id -g) -v $W:/work -e CARGO_HOME=/work/cargo-home -e CARGO_TARGET_DIR=/work/probe-target -w /work/probe pallas-ci:1 cargo build --release
cp $W/probe-target/release/pallas-probe $W/bin/pallas-probe

git -C $W/pallas worktree add ../pallas-strings repro/indef-strings-on-merge
docker run --rm --user $(id -u):$(id -g) -v $W:/work -e CARGO_HOME=/work/cargo-home -e CARGO_TARGET_DIR=/work/probe-target -w /work/probe-strings pallas-ci:1 cargo build --release
cp $W/probe-target/release/pallas-probe $W/bin/pallas-probe-strings
PROBE=$W/bin/pallas-probe-strings OUT=$W/out-strings ./run.sh 100 1
```
