#!/bin/sh
# all.sh, box side, D set: the gates and run15 on the build tree, then run15 and the lib suite on the red arm and on the main arm, each tree with its own target.
# Red arm: the build tree with the is_request call removed from OwnSends::hold.
# Expected: build GATE lines exit=0 and run15 ok=15, red own_keepalive_response_sends_nothing failed and the other 14 ok, main ok=4 failed=10 for the 14 it compiles.
set -u
B=pallas-network2/src/behavior
v15() { echo "$1"; echo "$2 ok=$(echo "$1" | grep -c ' ok') failed=$(echo "$1" | grep -c ' failed') missing=$(echo "$1" | grep -c ' missing') nobuild=$(echo "$1" | grep -c ' nobuild')"; }
rm -rf "$D/red" && cp -a "$D/build" "$D/red"
sed -i 's/if !is_request(msg) || self.held/if self.held/' "$D/red/$B/own.rs"
echo "RED_EDIT is_request_calls=$(grep -c 'is_request(msg)' "$D/red/$B/own.rs")"
find "$D/build" "$D/red" "$D/main" -exec touch {} +
echo "ARM BUILD gates"
cp "$D/gates.sh" "$D/build/gates.sh"
sh "$D/dk.sh" build 'sh gates.sh'
rm "$D/build/gates.sh"
echo "ARM BUILD run15"
v15 "$(sh "$D/dk.sh" build 'sh run15.sh')" BUILD15
echo "ARM RED run15"
v15 "$(sh "$D/dk.sh" red 'sh run15.sh')" RED15
sh "$D/dk.sh" red 'cargo test -q -p pallas-network2 --lib 2>&1 | grep -E "^test result|FAILED|panicked|left:|right:" | head -20'
echo "ARM MAIN run15"
v15 "$(sh "$D/dk.sh" main 'sh run15.sh')" MAIN15
sh "$D/dk.sh" main 'cargo test -q -p pallas-network2 --lib 2>&1 | grep -E "^test result|FAILED" | head -20'
df -h /opt
echo "ALL DONE"
