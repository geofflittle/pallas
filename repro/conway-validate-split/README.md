# Conway validate split, box runners

Scripts that gated the four split branches on the build box. Copy `run.sh`, `job.sh` and `gate.sh` to `/opt/build/cvsplit-1016` on the box, and run `sync.sh` from the laptop. The CI image `pallas-ci:1` and the cargo home `/opt/build/shared-cargo` must exist on the box.

## Gate one branch

```
sh sync.sh <worktree of the branch> <name>
ssh root@u5c.musashi.sundae.fi /opt/build/cvsplit-1016/gate.sh <name>
```

Expected last line on each of `fix/validate-conway-deposits`, `fix/validate-conway-certificate-witnesses`, `fix/validate-conway-collateral`, `fix/validate-conway-input-eras` and `repro/conway-validate-split-merge-check`: `gate <name> failed=0`.

## Arms

Each arm is a branch. Sync it as its own tree name, then run the test named.

| Branch | Command on the box | Expected line in the log |
|---|---|---|
| `repro/conway-validate-split-certs-registration-fixture` | `gate.sh certs` | `expected missing certificate script, got Err(PostAlonzo(PreservationOfValue))` |
| `repro/conway-validate-split-certs-delegation-fixture-break` | `W=<name> job.sh <name>-test cargo test -p pallas-validate --test conway certificate_script` | `expected missing certificate script, got Err(PostAlonzo(VKWrongSignature))` |
| `repro/conway-validate-split-cert-fixture-funded-registration` | `W=<name> job.sh <name>-test cargo test -p pallas-validate --test conway through_dispatch` | `expected missing certificate script, got Err(PostAlonzo(PreservationOfValue))` |

## Shared target dir

`run-shared-target.sh` is the first runner, with one target dir for every tree. Install it as `run.sh`, sync `d0e68f78` as `deposits` and `d8a4c1af` as `certs`, then run `gate.sh deposits` followed by `gate.sh certs`. The certs test links the library built from the deposits tree, because `pallas_validate` gets the same metadata hash in both trees and the deposits build is newer than the certs sources. Observed line: `expected missing certificate script, got Err(PostAlonzo(VKWrongSignature))`. With `run.sh` the same tree gives `PreservationOfValue`.
