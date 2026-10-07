#!/bin/sh
# mkmain.sh <build worktree> <out dir>: main 0ea294c1 production code with the build's test modules and testing.rs.
# The new own_keepalive_response_sends_nothing test is cut, since it calls send_own, which main lacks.
set -eu
W=$1
O=$2
R=/Users/geofflittle/code/pallas
B=pallas-network2/src/behavior
mkdir -p "$O"
splice() {
  # $1 path, $2 out name: main's lines before its first cfg(test) line, then the build's from its cfg(test) line.
  m=$(/usr/bin/git -C "$R" show "0ea294c1:$1" | /usr/bin/grep -n '^#\[cfg(test)\]$' | head -1 | cut -d: -f1)
  b=$(/usr/bin/grep -n '^#\[cfg(test)\]$' "$W/$1" | head -1 | cut -d: -f1)
  { /usr/bin/git -C "$R" show "0ea294c1:$1" | head -n $((m - 1)); tail -n +"$b" "$W/$1"; } > "$O/$2"
}
splice $B/initiator/mod.rs initiator_mod.rs
splice $B/responder/mod.rs responder_mod.rs
awk '/^    #\[test\]$/ { pend = $0; next }
     /^    fn own_keepalive_response_sends_nothing\(\)/ { skip = 1; pend = ""; next }
     skip { if ($0 ~ /^    }$/) skip = 0; next }
     { if (pend != "") { print pend; pend = "" } print }' "$O/initiator_mod.rs" > "$O/initiator_mod.rs.tmp"
mv "$O/initiator_mod.rs.tmp" "$O/initiator_mod.rs"
cp "$W/pallas-network2/src/testing.rs" "$O/testing.rs"
echo "MKMAIN own_test_lines=$(/usr/bin/grep -c own_keepalive_response_sends_nothing "$O/initiator_mod.rs") send_own_calls=$(/usr/bin/grep -c send_own "$O/initiator_mod.rs")"
