# Musashi w36 block fixtures

Ten real blocks cut from a relay's chain database, kept as the oracle for the
Dijkstra era model. Every fixture is lowercase hex of the block exactly as the
node stored it, wrapper and all, with no trailing newline, which is the form
every other block in `test_data` takes.

## Where these bytes come from

| | |
| --- | --- |
| prototype tag | `prototype-2026w36` |
| network | Musashi, network magic 164 |
| snapshot | `https://leios1-rel-a-1.play.dev.cardano.org/leios.full.tar.zst` |
| snapshot size and time | 5358239 bytes, last modified 2026-09-10T15:00:28Z, fetched to the build box 2026-09-10T15:11:17Z |
| snapshot sha256 | `a4ac4e0457d2ae43c36dae82f07beabef357367556e66da97981156d17df90cf` |
| ledger commit | `1587f21a7d1306dc590c2749a5c66232ef66aad0` |
| shelley genesis hash | `1944510a4fd91415444285231058f6f6ff0f6f3ff3d0356c76c00c5a77f29567` |
| chain start | 2026-09-07T00:00:00Z, slot length 1, epoch length 21600, security parameter 108 |
| immutable database | 289 chunks, 14937 blocks, slots 12 to 311104 |
| Conway to Dijkstra | last Conway block 4254 at slot 86373, first Dijkstra block 4255 at slot 86463, the epoch 4 boundary |

The chain begins in Conway (wrapper tag 7) and hard forks to Dijkstra (wrapper
tag 8) one day in. Eight of the ten fixtures are Dijkstra blocks. Two are
Conway, and both are there on purpose: fixture 1 is the block immediately
before the fork, and fixture 10 carries transaction body and witness set keys
that no Dijkstra block on this chain uses.

## The fixtures

Every block hash below is blake2b-256 of the header CBOR. Each was checked
against the header hash the node itself recorded in its secondary index, along
with the block's CRC32 and the header's offset and size, at the moment the
block was cut. A block whose four values did not all agree was never written.

| file | chunk/index | slot | block | hash | bytes | tag | txs | cert | announcement | why |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dijkstra-w36-1.block | 00079/39 | 86373 | 4254 | `802112126cc600a6afc5193a0150aafd9f6563bec28207df7e6c20cc62e95f8e` | 862 | 7 | 0 | false | nil | the last Conway block, the block the fork replaces |
| dijkstra-w36-2.block | 00080/0 | 86463 | 4255 | `d0c2a26a0192baf397b75cd38137987d82036c269089362842888279f3e19daf` | 862 | 8 | 0 | false | nil | the first Dijkstra block, and the zero transaction case |
| dijkstra-w36-3.block | 00080/22 | 86855 | 4277 | `adb23531ebb61891912e6a4bdabcbaaa053223d2de342eedbaa9b6af4fb526f3` | 1689 | 8 | 1 | false | nil | the first Dijkstra block carrying a certificate, body key 4 |
| dijkstra-w36-4.block | 00264/18 | 285530 | 14094 | `294b3df1e6758e6f17f2b5a09ed469c6ec37c2db4d274264c8ee5edabe31229a` | 1064 | 8 | 1 | false | nil | a plain transfer, body keys 0, 1 and 2 only, the smallest block with a transaction |
| dijkstra-w36-5.block | 00268/0 | 289441 | 14212 | `f8926da4333a3ce5fdb7b60a00d80eb23da0823c6962f06abfdee149b59dae41` | 1827 | 8 | 1 | false | nil | the first Dijkstra block with auxiliary data, body key 7 and a non nil auxiliary slot |
| dijkstra-w36-6.block | 00277/8 | 299514 | 14534 | `7b7c9f48ac331106e9f9ef03856090bc6275f09fca5bb4c789068d04c54b079a` | 2269 | 8 | 3 | false | nil | three transactions, one of them with auxiliary data |
| dijkstra-w36-7.block | 00270/0 | 291625 | 14278 | `dec1d7087ff0191191fd3bac1559ae1b9c40b93ad33ec9cfba1f2e4722019a23` | 3483 | 8 | 4 | false | nil | the most transactions in any Dijkstra block on this chain, and the largest one |
| dijkstra-w36-8.block | 00287/58 | 311025 | 14935 | `c9d7bca094227279830e2e2110acbb965dc9e90d469ac97594d40bc8e295735c` | 862 | 8 | 0 | false | nil | the last block of the last sealed chunk |
| dijkstra-w36-9.block | 00288/0 | 311104 | 14936 | `920a4883bf663cd3640af8ee87292edd391ff9b99debef2ba556f2f8e9d5761d` | 1232 | 8 | 1 | false | nil | the immutable tip, from the chunk the node was still appending to |
| dijkstra-w36-10.block | 00000/35 | 780 | 35 | `57899ebff26608bedc371a4754b90f0bc10d27b82471be8c7cc3fbfb9a090c49` | 6336 | 7 | 1 | false | nil | the only block on the chain with body keys 11, 13, 16, 17 and 20, witness keys 0, 5 and 7, and indefinite length arrays |

Map keys each fixture's transactions use, which is what makes the absences
further down mean something:

| file | transaction body keys | witness set keys | indefinite length containers |
| --- | --- | --- | --- |
| dijkstra-w36-1.block | none | none | 0 |
| dijkstra-w36-2.block | none | none | 0 |
| dijkstra-w36-3.block | 0, 1, 2, 4 | 0 | 0 |
| dijkstra-w36-4.block | 0, 1, 2 | 0 | 0 |
| dijkstra-w36-5.block | 0, 1, 2, 4, 7 | 0 | 0 |
| dijkstra-w36-6.block | 0, 1, 2, 4, 7 | 0 | 0 |
| dijkstra-w36-7.block | 0, 1, 2, 4, 7 | 0 | 0 |
| dijkstra-w36-8.block | none | none | 0 |
| dijkstra-w36-9.block | 0, 1, 2, 4 | 0 | 0 |
| dijkstra-w36-10.block | 0, 1, 2, 11, 13, 16, 17, 20 | 0, 5, 7 | 3 arrays |

## What the bytes say about a Dijkstra block's shape

Measured over all 14937 immutable blocks and all 1046 blocks in the volatile
files, with no era decoder involved.

- A block is `[wrapper_tag, block]` and a Dijkstra `block` is a **two** element
  array, `[header, block_body]`.
- `block_body` is a **three** element array. Every one of the 10682 immutable
  Dijkstra blocks reads as `[transactions, nil, nil]`, matching the ledger
  CDDL at `1587f21a`, which is `[transactions, leios_certificate/ nil,
  peras_certificate/ nil]`. It is not a one element array.
- Each transaction is a **four** element array whose last element is a
  **bool**, on every one of the 144 immutable Dijkstra blocks that carries a
  transaction, and on every transaction in the volatile files. This matches
  `block_transaction = [transaction_body, transaction_witness_set,
  auxiliary_data/ nil, bool]`.
- `header` is a two element array and `header_body` is a **twelve** element
  array, in both the Conway and the Dijkstra blocks of this chain. Element 10
  is the certificate bool and element 11 is the announcement or nil, so
  `operational_cert` and `protocol_version` ride as nested arrays rather than
  flattened groups.

## What this chain does not have yet

Each of these is a value that was read, not a field that was skipped.

- **No endorser block announcement.** All 14937 immutable headers and all 1046
  volatile headers decode `eb_announcement` as nil. Highest slot checked
  311104 in the immutable database and 313194 in the volatile files, which is
  the snapshot's own tip.
- **No Leios certificate.** The same headers all decode
  `block_body_contains_leios_cert` as `false`, and independently, every
  Dijkstra block body's `leios_certificate` slot reads as nil. Two fields,
  same answer.
- **No Peras certificate.** Every Dijkstra block body's third element reads as
  nil.
- **No transaction body key 24 and no key 27.** Across both scans the
  transaction body keys that do appear are 0, 1, 2, 3, 4, 7, 11, 13, 16, 17,
  19 and 20, and in the Dijkstra era only 0, 1, 2, 4 and 7. The reader that
  found those is the reader that reports no 24 and no 27.
- **No script witnesses in any Dijkstra block.** Every Dijkstra witness set on
  this chain carries key 0 alone. Fixture 10, a Conway block, is the only
  fixture with a redeemer (key 5) and a Plutus script (key 7).
- **No indefinite length container in any Dijkstra block.** All 10682 read as
  entirely definite. A model that re-encodes indefinite containers as definite
  ones would still round trip these fixtures byte for byte, so these fixtures
  cannot prove that part of a model. Fixture 10 is a Conway block and is the
  one fixture that exercises it.

## No endorser block bytes exist to cut

The snapshot carries `db/leios.db`, a 278528 byte SQLite database with four
tables, `ebs`, `ebTxs`, `ebsMissingTxs` and `txs`. No column in any of them
holds endorser block bytes. `ebs` is `(ebSlot, ebHashBytes, ebBytesSize,
missingTxCount)`, so it records that an endorser block exists and how large it
is, and nothing more.

It also holds exactly one `ebs` row, at `ebSlot` 2962805, hash
`43ca94125edb7e6d3883d28e1eb4311def8d5d4c50fc685affc30c5b2569a6d6`, size
24699, with all 686 of its transactions listed as missing and the `txs` table
empty. That row belongs to the previous Musashi chain: the archived w35 node
database on the build box holds the same row, same slot, same hash, same size,
among its 35604. So the Leios database shipped inside the w36 relay snapshot
is a remnant of the relay's earlier life, not a record of this chain.

No `ebw36-*.block` fixtures were written, because there are no endorser block
bytes in this snapshot to write.

## How to reproduce

The walker that produced every number above is
`pallas-hardano/examples/chunk_facts.rs`.

```
cargo run --release -p pallas-hardano --example chunk_facts -- scan <immutable_dir>
cargo run --release -p pallas-hardano --example chunk_facts -- volatile <volatile_dir>
cargo run --release -p pallas-hardano --example chunk_facts -- cut <immutable_dir> <chunk> <index> <out>
cargo run --release -p pallas-hardano --example chunk_facts -- file <hex_path>
```

The table above is restated in that file's tests, so a fixture and its
provenance row cannot drift apart:

```
cargo test -p pallas-hardano --example chunk_facts
```
