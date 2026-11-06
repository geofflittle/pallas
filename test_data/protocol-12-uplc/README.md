# Protocol 12 raw programs

Sixteen raw Plutus programs (thirteen PlutusV3, two PlutusV1, one PlutusV2), each with
the arguments it is applied to, the testing cost model of its language, a budget of
14000000 memory and 10000000000 steps, and the result the Haskell ledger's script
evaluation at protocol 12 gave it: the units it spent, or its failure.

Sources: cardano-ledger 9df9f98cce86c5f2195142b2c02dd9b39dd9dc7c with plutus-ledger-api
1.71.0.0, a writer that builds each program with plutus-core and evaluates it with the
ledger's evaluation function for its language.

| File | sha256 |
|---|---|
| slice.jsonl | 91fb952ea7cc56e55dd0e4a9b2fdf61735261c9fa30df37ec7f6f3566c980152 |
