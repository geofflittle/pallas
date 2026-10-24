# Dijkstra generated vectors

Values generated from the Dijkstra ledger CDDL and decoded by the Haskell
ledger, as lowercase hex, no trailing newline.

| | |
| --- | --- |
| ledger commit | `9df9f98cce86c5f2195142b2c02dd9b39dd9dc7c` |
| spec | `dijkstraCDDL`, the Huddle spec of `cardano-ledger-dijkstra` |
| generator | cuddle over that spec, with definite lengths only |
| seed and count | seed 1, 100 values per kind |

A cut value names its kind, its source, its index and the item path inside
that value, written as `[i]` for array element `i`, `{k}` for map key `k` and
`tag258` for the set tag. A framed value places the cut items inside the
fixed CBOR shown, at each `{}` in order, so that the ledger decodes it as a
transaction body. Every file below is a value the ledger's decoder for its
kind accepted. The expected values in `pallas-primitives/src/conway/model.rs`
are what the ledger read from these bytes.

| file | from | items | frame | kind |
| --- | --- | --- | --- | --- |
| body-certificates-ending-in-null.hex | body, cddl-plain | 95 `{4} tag258 [0]`, 21 `{4} [2]`, 11 `{4} [1]`, 16 `{4} [1]` | `a400d90102800180020004d9010284{}{}{}{}` | transaction body |
| body-governance-ending-in-null.hex | body, cddl-plain | 12 `{19}`, 71 `{20} tag258 [2]`, 22 `{20} tag258 [4]`, 22 `{20} tag258 [0]`, 25 `{20} tag258 [0]` | `a500d90102800180020013{}14d9010284{}{}{}{}` | transaction body |

Each item ends in an absent value that the CDDL types as `T / nil`: a pool
registration without metadata, a committee resignation, a DRep registration
and a DRep update without an anchor, a vote without an anchor, a constitution
without a guardrail script, and a parameter change, a treasury withdrawal and
a no confidence action without a previous action or policy.
