#!/bin/sh
# probe-b.sh, box side with D set to a dir holding parent, rev (2da54519) and old (d436ced2) trees with probe819.rs included.
# Copies probe819b.rs over each tree's probe and prints the ban and demote probe lines.
# The parent is left out: its complete_handshake returns unit and it sends no range at a handshake, so the original probe measures it.
# Expected: rev prints sends_to_..=0 and recovered_on_other_peer=1 three times, old prints sends_to_..=1 and recovered_on_other_peer=0.
set -u
cd "$D"
I=pallas-network2/src/behavior/initiator
for t in rev old; do
  cp probe819b.rs "$t/$I/probe819.rs"
  find "$t" -exec touch {} +
  out=$(sh "$D/dk.sh" "$t" "cargo test -p pallas-network2 --lib probe819 -- --nocapture --test-threads=1 2>&1")
  echo "$out" > "$D/log-b-$t.txt"
  echo "B $t | $(echo "$out" | grep -E '^test result' | head -1)"
  echo "$out" | grep -oE 'PROBE819 (violation|demote|ban_cmd).*' | sed "s/^/B $t | /"
done
