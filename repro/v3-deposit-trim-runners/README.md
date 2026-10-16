# V3 deposit port and trim runners

Box runners for the mutant pass and the gates of the V3 deposit commit and its trim. Copy this directory to `/opt/build/v3dep-1016` on the build box, with `cargo/` a copy of the shared cargo home and the `pallas-ci:1` image present.

## Rerun

From the laptop, in this directory:

```
sh sync.sh dbdadb22 trim
ssh root@u5c.musashi.sundae.fi /opt/build/v3dep-1016/gate.sh trim
sh gen.sh dbdadb22
rsync -a --delete arms/ root@u5c.musashi.sundae.fi:/opt/build/v3dep-1016/arms/
ssh root@u5c.musashi.sundae.fi 'B=trim /opt/build/v3dep-1016/arms-all.sh'
```

For the untrimmed tip, use `sync.sh 77a5d8a0 tip`, `gen.sh 77a5d8a0` and `B=tip`.

## Expected verdict lines

Gate on each of cfc9b39a, 66d4bd93, d1075450, aff43373, 439f3105 and dbdadb22 ends with:

```
gate trim failed=0
```

Mutant run on the trimmed tip (`logs/arms-trim.out`):

```
arm baseline base=trim exit=0 compiled=6
arm D12-v1v2-txinfo-protocol base=trim exit=0 compiled=6
arm DF-default-protocol-10 base=trim exit=0 compiled=6
```

Every other arm prints `exit=101` with at least one failing test. D12 is an equivalent mutant and DF is the untested default of the public `to_plutus_data`.

`logs/arms-tip.out` holds the same run on the untrimmed tip 77a5d8a0, without D10b.
