# Dijkstra block fixtures

Real blocks from the Musashi testnet, as lowercase hex of the block exactly as
the node stored it, wrapper and all, no trailing newline.

| | |
| --- | --- |
| network | Musashi, network magic 164 |
| node release | ouroboros-leios `prototype-2026w36` |
| ledger commit | `1587f21a7d1306dc590c2749a5c66232ef66aad0` |
| shelley genesis hash | `1944510a4fd91415444285231058f6f6ff0f6f3ff3d0356c76c00c5a77f29567` |
| chain start | 2026-09-07T00:00:00Z, slot length 1, epoch length 21600 |
| Conway to Dijkstra | last Conway block 4254 at slot 86373, first Dijkstra block 4255 at slot 86463 |

The blocks were read out of the immutable chunks of a relay's chain database
(`https://leios1-rel-a-1.play.dev.cardano.org/leios.full.tar.zst`) with
pallas-hardano's chunk reader. Each hash is blake2b-256 of the header CBOR and
was checked against the header hash in the node's own secondary index, together
with the block's CRC32 and the header's offset and size.

| file | chunk/index | slot | block | hash | bytes | tag | txs | why |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| conway5.block | 00079/39 | 86373 | 4254 | `802112126cc600a6afc5193a0150aafd9f6563bec28207df7e6c20cc62e95f8e` | 862 | 7 | 0 | the last block before the fork, a Conway body under a twelve element header body |
| dijkstra1.block | 00080/0 | 86463 | 4255 | `d0c2a26a0192baf397b75cd38137987d82036c269089362842888279f3e19daf` | 862 | 8 | 0 | the first Dijkstra block, no transactions |
| dijkstra2.block | 00080/22 | 86855 | 4277 | `adb23531ebb61891912e6a4bdabcbaaa053223d2de342eedbaa9b6af4fb526f3` | 1689 | 8 | 1 | the first certificate |
| dijkstra3.block | 00264/18 | 285530 | 14094 | `294b3df1e6758e6f17f2b5a09ed469c6ec37c2db4d274264c8ee5edabe31229a` | 1064 | 8 | 1 | a plain transfer, the smallest block with a transaction |
| dijkstra4.block | 00268/0 | 289441 | 14212 | `f8926da4333a3ce5fdb7b60a00d80eb23da0823c6962f06abfdee149b59dae41` | 1827 | 8 | 1 | the first auxiliary data |
| dijkstra5.block | 00277/8 | 299514 | 14534 | `7b7c9f48ac331106e9f9ef03856090bc6275f09fca5bb4c789068d04c54b079a` | 2269 | 8 | 3 | three transactions |
| dijkstra6.block | 00270/0 | 291625 | 14278 | `dec1d7087ff0191191fd3bac1559ae1b9c40b93ad33ec9cfba1f2e4722019a23` | 3483 | 8 | 4 | four transactions with five certificates between them |
| dijkstra7.block | 00288/0 | 311104 | 14936 | `920a4883bf663cd3640af8ee87292edd391ff9b99debef2ba556f2f8e9d5761d` | 1232 | 8 | 1 | a one transaction block from a later chunk |
| dijkstra8.block | 00344/8 | 371723 | 17403 | `33a48eee693522320891dd4d1da8ee33c288c854eb25337802dbc4c912571d07` | 962 | 8 | 0 | the first Leios certificate, with an announcement |
| dijkstra9.block | 00346/20 | 374306 | 17512 | `112495349409ccaa7a57e810aa59b014337612438a3e07cd5ce1bb3126dfbbea` | 925 | 8 | 0 | a Leios certificate without an announcement |
| dijkstra10.block | 00341/23 | 369030 | 17297 | `917c72dcd2d2222df5cc82fcebea55c4f9135e5f1491afd66d79d48d52e42f60` | 3373 | 8 | 7 | body key 3, map and array outputs, certificates, auxiliary data, a bare set |
| dijkstra11.block | 00327/33 | 354033 | 16808 | `e45c1dc810ddfb36ffb9647eaf08861b4611fb4e872a227c00337dddf2680b8c` | 1088 | 8 | 1 | the smallest map form output |
| dijkstra12.block | 00352/29 | 380643 | 17794 | `3e0e56a9af0cb26e0641747ca835874135d02d35a29820a5e4de6beb37c17914` | 5689 | 8 | 24 | the smallest indefinite length container |
| dijkstra13.block | 00278/24 | 301082 | 14594 | `cf522686b27e452b3e261904058c7e323f3723e2f5c629e5a7542579b59474b4` | 1274 | 8 | 1 | certificate tag 9, vote delegation |
| dijkstra14.block | 00574/21 | 620349 | 28687 | `9b481f4b4fa46de9a1bde085570b5fc9d90f161f99bde7f63bfe4ed20dbc37bf` | 1734 | 8 | 1 | a transaction body writing its certificate set as a bare array rather than under tag 258 |
| dijkstra15.block | 00344/11 | 371916 | 17406 | `0db84efa0259153a240cecacd0f9e52f942d40f96b132ebd0d5b3526e19b3a7b` | 964 | 8 | 0 | an announced endorser block size of 71103, which needs the five byte uint |
| dijkstra16.block | 00287/58 | 311025 | 14935 | `c9d7bca094227279830e2e2110acbb965dc9e90d469ac97594d40bc8e295735c` | 862 | 8 | 0 | no transactions, no Leios certificate and no announcement, which is what the endorser tests build a certifying block from |

The `tag` column is the block wrapper tag. A Dijkstra header arriving on its
own over chainsync carries envelope tag 7 rather than the wrapper tag 8, and
that number was observed on a node rather than read from any file here.

What each fixture's transactions carry. Every other key and shape the era
models is exercised by no fixture and is modelled from the CDDL alone.

| file | body keys | witness keys | outputs | certificate tags | set arms | indefinite | leios certificate | announcement |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dijkstra2.block | 0, 1, 2, 4 | 0 | array | 3 | tagged | 0 | nil | nil |
| dijkstra3.block | 0, 1, 2 | 0 | array | none | tagged | 0 | nil | nil |
| dijkstra4.block | 0, 1, 2, 4, 7 | 0 | array | 3, 7 | tagged | 0 | nil | nil |
| dijkstra5.block | 0, 1, 2, 4, 7 | 0 | array | 3, 7 | tagged | 0 | nil | nil |
| dijkstra6.block | 0, 1, 2, 4, 7 | 0 | array | 2, 3, 7 | tagged | 0 | nil | nil |
| dijkstra7.block | 0, 1, 2, 4 | 0 | array | 2 | tagged | 0 | nil | nil |
| dijkstra8.block | none | none | none | none | none | 0 | present | present |
| dijkstra9.block | none | none | none | none | none | 0 | present | nil |
| dijkstra10.block | 0, 1, 2, 3, 4, 7 | 0 | map, array | 3, 7 | tagged, bare | 0 | nil | nil |
| dijkstra11.block | 0, 1, 2 | 0 | map | none | bare | 0 | nil | nil |
| dijkstra12.block | 0, 1, 2 | 0 | array | none | tagged | 1 | nil | nil |
| dijkstra13.block | 0, 1, 2, 4 | 0 | array | 7, 9 | tagged | 0 | nil | nil |
| dijkstra14.block | 0, 1, 2, 3, 4 | 0 | map | 3 | bare | 0 | nil | nil |
| dijkstra15.block | none | none | none | none | none | 0 | present | present |
| dijkstra16.block | none | none | none | none | none | 0 | nil | nil |

## The endorser block fixtures

Nine files describe three endorser blocks of the Musashi chain that ran before
the one above, under node release `prototype-2026w35`. The `.ebbody` file is
the body as leios-fetch delivered it, the `.ebtxs` file is one hex transaction
per line in body order with its byte string envelope still on, and the
`.header` file is the ranking block header whose `eb_announcement` names that
body.

The bodies and the transactions were pulled over leios-fetch from that chain's
relay. The headers were cut afterwards from the same chain's immutable
database, kept beside the node that captured the bodies, by walking one chunk
with a CBOR decoder and taking each block's header span. Every header hash
below is blake2b-256 of the header bytes and equals what that node's own
secondary index records for the block.

| file | chunk/index | slot | block | announcing header hash | bytes | certifies | announces |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dijkstra-eb1.header | 00397/47 | 429789 | 21507 | `abba50f39b31ca7ed67ebe72f588668073a66090a33504a76d9abbc1d6a9d3b5` | 892 | yes | `dijkstra-eb1.ebbody`, 37 bytes, `f753a2b1e556780d36961a1cde7ba05ea3ff9a58047b03385d5242b5c4610b5c` |
| dijkstra-eb2.header | 00368/27 | 397855 | 19847 | `779e95c2816db83f41528b1b8260034f68c8f817c4f32edadc05de0fc16f22fb` | 896 | no | `dijkstra-eb2.ebbody`, 1082 bytes, `8f21f4531c3d5e6cf6939821221d01bc6c4f33c6470de4b1a852dd99ea19883d` |
| dijkstra-eb3.header | 00348/23 | 376369 | 18772 | `a4c183c4234560ae182fd5e56a021f0f4b13d5019bab155a536db9e8bbebca90` | 893 | yes | `dijkstra-eb3.ebbody`, 15303 bytes, `a8f2a746b33a74bf39f9f8a56e108746d1512a5af91f48ba4336a61b81cf5fcd` |

The `certifies` column is `block_body_contains_leios_cert`, which names the
announcement made before its own header rather than the one that header makes,
so one header can carry both and two of these do.

The header alone is cut rather than the whole block because that chain writes a
four element block body led by a nil, and this era models the three element
body of the chain above. The block at slot 397855 also weighs 88105 bytes
against 896 for its header.

What each body names, read with a standalone CBOR walker and Python's
`hashlib.blake2b` rather than with pallas. The count is both the body's map
length and the number of lines in its `.ebtxs` file. The first key is
blake2b-256 of the first transaction with its envelope removed, and the first
transaction id is blake2b-256 of that transaction's body span.

| file | transactions | first key | first size | first transaction id |
| --- | --- | --- | --- | --- |
| dijkstra-eb1.ebbody | 1 | `a69f9fc581e5914a101a3e619f5ce64b6bce76721fd5eed0cbad0e0f6d411cc5` | 229 | `a2a3715bac697e28991d003c62a0c280fb91af40944f7bbcee27d972ad0f0a08` |
| dijkstra-eb2.ebbody | 30 | `455a00b521f35f2c0a6ff0a59296c3316de6219af206c13d2e95870f66541fec` | 200 | `1839e14ac4327a8a8f6c00d2bfbdb0b4a95bb6dc92d84d3a45d3ff9ada7964c2` |
| dijkstra-eb3.ebbody | 425 | `b6813cef46bc2f5b0ce9761e7177138f93edf6d4b9c95ff28b071a4ce0978d6b` | 201 | `1c60a3e90174e349a40a302742de803692d763dc906c51c1c21e253d841d555c` |
