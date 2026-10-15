# Box apparatus for the utxorpc v1 rewrite

Scripts that ran every check in `.claude/docs/2026-10-14_utxorpc-v1-mapper-build.md`.
They expect the ssh alias `musashi-aws`, the directory `/opt/build/u5c-v1` on it with
`run.sh` (copied here), a bare clone of the spec repository at `/opt/build/u5c-v1/spec.git`
holding revs 7b5ebd9 and f2d528e6, and the pallas CI image `pallas-ci:1`.

- `run.sh`: one cargo command in the CI image, target dir `/opt/build/u5c-v1/target`.
- `syncdir.sh <worktree>`: rsync a local worktree to the box tree and touch it.
- `sync.sh`: the same for the rewrite worktree path used during the build.
- `box.sh <log> <cargo args>`: run one cargo command and print its verdict lines.
- `percommit.sh <commit>...`: check pallas-utxorpc at each commit, default and unstable.
- `prefix57.sh <log> <args>`: test pallas-utxorpc with the spec patched to 7b5ebd9.

To rerun a red control: check out its `repro/` branch in a worktree, run
`sh syncdir.sh <worktree>`, then the command the findings document names for it,
and compare the verdict line.
