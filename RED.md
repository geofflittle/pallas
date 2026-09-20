# The red control for the default build header decode

Parent `f1f31df`, the tip of `feat/dijkstra-era-decode` before the fix. The
decode arms are exactly `f1f31df`'s. Only the test module carries the change,
so this tree is the source under test with the new assertion and nothing else.

Run it:

    rsync -a --delete --exclude target --exclude .git <this worktree>/ musashi-aws:/opt/build/fix-800b/src/
    ssh musashi-aws 'find /opt/build/fix-800b/src -type f -exec touch {} +'
    ssh musashi-aws '/opt/build/fix-800b/run.sh cargo test -p pallas-traverse'

Expected verdict, one failure and it names tag 7:

    test header::tests::an_unknown_envelope_tag_decodes_as_babbage_without_the_feature ... FAILED
    envelope tag 7 gave UnknownEra(7)
    test result: FAILED. 41 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

The fix is commit `3d076e2` on `feat/dijkstra-era-decode`, which turns this
green at 42 passed.
