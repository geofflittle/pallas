#!/bin/sh
# red2.sh, box side, D set. Tree D/mut is a git repo of the amended tree.
# Runs the two tests added for the M5 and M18 survivors on the base and under each mutant.
# Expected: base both ok, M5 fails inbound_before_sent_keeps_own_keepalive with left (1, 1), M18 fails reconnect_before_owed_next_owes_nothing with left 0.
cd "$D/mut" || exit 1
B=pallas-network2/src/behavior
T='inbound_before_sent_keeps_own_keepalive reconnect_before_owed_next_owes_nothing'
rarm() {
  name=$1; file=$2; expr=$3
  git checkout -q -- .
  [ -n "$expr" ] && perl -0pi -e "$expr" "$file"
  echo "RED2 $name changed_lines=$(git diff --numstat | awk '{s+=$1+$2} END {print s+0}')"
  sh "$D/dk.sh" mut "cargo test -q -p pallas-network2 --lib -- $T 2>&1" \
    | grep -E '^test result|^    behavior::|panicked|left:|right:|left ==|^error' | sed "s/^/RED2 $name | /"
}
rarm base "" ""
rarm M5_inbound_clears_channel $B/initiator/mod.rs 's/(state\.apply_msg\(msg\);\n\n            all_visitors!\(self, pid, state, visit_inbound_msg\);)/state.own = Default::default();\n            $1/'
rarm M18_initiator_reset_keeps_owed $B/initiator/mod.rs 's/        self\.next_owed = false;\n//'
git checkout -q -- .
echo RED2 DONE
