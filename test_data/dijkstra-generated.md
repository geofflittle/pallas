# Dijkstra generated vectors

Values generated from the Dijkstra ledger CDDL and decoded by the Haskell
ledger, as lowercase hex, no trailing newline.

| | |
| --- | --- |
| ledger commit | `9df9f98cce86c5f2195142b2c02dd9b39dd9dc7c` |
| spec | `dijkstraCDDL`, the Huddle spec of `cardano-ledger-dijkstra` |
| generator | cuddle over that spec, with definite lengths only |
| seed and count | seed 1, 100 values per kind |

A whole generated value names its kind, its source and its index. Every file
below is a value the ledger's decoder for its kind accepted. The expected
values in `pallas-primitives/src/dijkstra/tests.rs` are what the ledger read
from these bytes.

| file | from | item | frame | kind |
| --- | --- | --- | --- | --- |
| block-header-version-info.hex | block, cddl-plain, 95 | whole | none | block |

The ledger read its header version info as highest supported major version
1004951745 and software tag 3942894501. It reads the same values when the
version info is written as an indefinite array, and refuses the block when
either field is 2^32.
