#!/bin/sh
# Three Dolos arms against the restacked pallas dirty tip, on the box.
# Setup, from a laptop:
#   mkdir -p /opt/build/ldisc-a/pallas-dirty /opt/build/ldisc-a/dolos /opt/build/ldisc-a/logs
#   git -C ~/code/pallas archive 9fa638333650669f765abebd1905853679e2709d | ssh musashi-aws tar -x -C /opt/build/ldisc-a/pallas-dirty
#   git -C ~/code/dolos-wt-nofollow archive d81d8377 | ssh musashi-aws tar -x -C /opt/build/ldisc-a/dolos
#   copy dolos-patch.toml and dolos-run.sh to /opt/build/ldisc-a, append the patch to dolos/Cargo.toml
# Expected:
#   unchanged:   E0004 `&_` not covered at src/sync/leios.rs:100 (DisconnectReason), EXIT=101
#   wildcard:    Finished, EXIT=0
#   noforward:   E0432 DisconnectReason and E0599 PeerDisconnected, EXIT=101
set -u
W=/opt/build/ldisc-a
find "$W/pallas-dirty" "$W/dolos" -type f -exec touch {} +
"$W/dolos-run.sh" unchanged "cargo check"
grep -E '^error\[|EXIT=' "$W/logs/unchanged.log"
sed -i '102a\            _ => "for a reason this build does not name",' "$W/dolos/src/sync/leios.rs"
"$W/dolos-run.sh" wildcard "cargo check"
grep -E '^error\[|EXIT=' "$W/logs/wildcard.log"
cp "$W/pallas-dirty/pallas/Cargo.toml" "$W/pallas-dirty/pallas/Cargo.toml.keep"
sed -i '/pallas-network2?\/unstable/d' "$W/pallas-dirty/pallas/Cargo.toml"
"$W/dolos-run.sh" noforward "cargo check"
grep -E '^error\[|EXIT=' "$W/logs/noforward.log"
mv "$W/pallas-dirty/pallas/Cargo.toml.keep" "$W/pallas-dirty/pallas/Cargo.toml"
