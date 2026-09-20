# The red control for the script language variant gate

Parent `9878223`, the fix commit on `feat/dijkstra-era-accessors`, with one
thing put back: `ScriptLanguage::PlutusV4` and its `tag()` arm lose their
`#[cfg(feature = "unstable")]`. The enum keeps
`#[cfg_attr(feature = "unstable", non_exhaustive)]`, so the marking check is
satisfied and the stable variant list is the only thing that can fire.

This is the arm the marking check cannot see. A marking says how a caller may
match the enum, it says nothing about which variants the enum has.

Run it:

    rsync -a --delete --exclude target --exclude .git <this worktree>/ musashi-aws:/opt/build/fix-801/src/
    ssh musashi-aws 'find /opt/build/fix-801/src -type f -exec touch {} +'
    ssh musashi-aws '/opt/build/fix-801/run.sh cargo test -p pallas-traverse'

Expected verdict, a compile refusal naming the variant:

    error[E0004]: non-exhaustive patterns: `&script_ref::ScriptLanguage::PlutusV4` not covered
        --> pallas-traverse/src/lib.rs:839:31
         |
     839 |                           match value {
         |                                 ^^^^^ pattern `&script_ref::ScriptLanguage::PlutusV4` not covered

Putting both `cfg` lines back is `9878223` itself, which is green at 67 passed.
