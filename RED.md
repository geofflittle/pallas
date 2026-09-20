# The red control for the script language marking

Parent `dfdaa4c`, the tip of `feat/dijkstra-era-accessors` after the merge of
the reworked 800 commit and before the marking fix.

`pallas-traverse/src/script_ref.rs` is untouched, so `ScriptLanguage` carries
no `non_exhaustive` and its `PlutusV4` variant carries no `cfg`. Only
`pallas-traverse/src/lib.rs` changes, and the stable variant list still names
`PlutusV4`, so the variant check is satisfied and the marking check is the only
thing that can fire.

Run it:

    rsync -a --delete --exclude target --exclude .git <this worktree>/ musashi-aws:/opt/build/fix-801/src/
    ssh musashi-aws 'find /opt/build/fix-801/src -type f -exec touch {} +'
    ssh musashi-aws '/opt/build/fix-801/run.sh cargo test -p pallas-traverse'

Expected verdict, one failure:

    test attribute_tests::every_public_enum_carries_the_marking_it_is_listed_with ... FAILED
    test result: FAILED. 66 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

The assertion prints the whole list on one line. The two rows that differ come
out with:

    ... | grep -oE '\("ScriptLanguage", [A-Za-z]+\)'
    ("ScriptLanguage", Never)
    ("ScriptLanguage", WithUnstable)

The fix is commit `9878223` on `feat/dijkstra-era-accessors`.
