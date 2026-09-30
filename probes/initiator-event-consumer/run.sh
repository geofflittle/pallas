#!/bin/sh
# Checks an exhaustive match over the eight InitiatorEvent variants of
# pallas-network2 1.4.0, against the tree two levels up.
# Run from this directory inside the pallas-ci:1 image.
# Expected on 24eb91eb:
#   without features: Finished
#   --features unstable: E0004 `&_` not covered, InitiatorEvent is marked as non-exhaustive
# Expected on repro/ldisc-a-ungated-variant:
#   without features: E0004 `&InitiatorEvent::PeerDisconnected(_, _)` not covered
set -u
echo "== without features"
cargo check 2>&1 | grep -E '^error\[|Finished'
echo "== with unstable"
cargo check --features unstable 2>&1 | grep -E '^error\[|Finished|marked as non-exhaustive'
