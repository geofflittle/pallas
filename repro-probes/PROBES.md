# v1 mapper test probes

subtx_oracle.py decodes test_data/dijkstra-subtx.tx with its own CBOR walker and hashlib blake2b, no pallas code.

    python3 -I repro-probes/subtx_oracle.py test_data/dijkstra-subtx.tx

Expected lines include:

    outer id 74e2116ca6e0c809f156aa062a9d4ee8b156322618846f1bdea5d9ad7a206a12
    outer fee 700000
    sub0 id 63ac433eefaed71cf0fa2c7134f82ea38f3c6c338b3d5129c686c47b44162691
    sub0 script data hash 860576d43755cdb611c2f5fdd10d38a91e11362cebe52b198e8beacd900d883e
    sub0 reference inputs [('c095234678d98a74cd1488bda1971fbd2e9c828acf57f7673c679ec601abf7a6', 0)]

invalid_scan.py lists test_data blocks with a nonempty invalid transaction list and .tx fixtures whose validity flag is false. Expected: none of either.

arms.sh and run.sh run the planted break branches repro/v1-panel-fixes-* on musashi-aws under /opt/build/v1-panel-fixes, in the pallas-ci:1 image, after the clone in src holds them:

    ./arms.sh repro/v1-panel-fixes-sub-hash-whole-subtx

Each arm prints one FAILED test, the one its branch name targets.
