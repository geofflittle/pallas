#!/bin/sh
# red.sh, box side, D set. Tree D/red is a git repo at 2e7bfd92 plus tests-only.patch (the new tests, no production edit).
# Runs the two new tests on that base and under M12, M9 and M17 with the review's own edits.
# Expected: base both ok, M12 fails keepalive_sent_and_unanswered_sends_nothing, M9 and M17 fail continue_sync_before_sent_sends_one_then_owed.
cd "$D/red" || exit 1
B=pallas-network2/src/behavior
T='keepalive_sent_and_unanswered_sends_nothing continue_sync_before_sent_sends_one_then_owed'
rarm() {
  name=$1; file=$2; expr=$3
  git checkout -q -- .
  [ -n "$expr" ] && perl -0pi -e "$expr" "$file"
  echo "RED $name changed_lines=$(git diff --numstat | awk '{s+=$1+$2} END {print s+0}')"
  sh "$D/dk.sh" red "cargo test -q -p pallas-network2 --lib -- $T 2>&1" \
    | grep -E '^test result|^    behavior::|panicked|left:|right:|^error' | sed "s/^/RED $name | /"
}
rarm base "" ""
rarm M12_hold_ignores_accepted $B/own.rs 's/ \|\| !accepted//'
rarm M9_owed_never_cleared $B/own.rs 's/self\.own\.next_owed = false;\n//'
rarm M17_owed_set_always $B/initiator/chainsync.rs 's/if !state\.send_own\(pid, next, outbound\) \{\n                state\.owe_next\(\);\n            \}/state.send_own(pid, next, outbound);\n            state.owe_next();/'
git checkout -q -- .
echo RED DONE
