#!/bin/sh
# mutants.sh, run on the box with D set to the run dir. The tree D/mut is a git repo of the amended tree.
# The review's 17 arms, with M9 edited for next_owed now on InitiatorState.
# Each arm: apply one perl edit, check the edit took (count of changed lines), run the network2 lib tests, revert.
cd "$D/mut" || exit 1
B=pallas-network2/src/behavior

arm() {
  name=$1; file=$2; expr=$3
  git checkout -q -- .
  perl -0pi -e "$expr" "$file"
  n=$(git diff --numstat | awk '{s+=$1+$2} END {print s+0}')
  if [ "$n" = 0 ]; then echo "ARM $name EDIT_DID_NOT_APPLY"; return; fi
  out=$(sh "$D/dk.sh" mut "cargo test -q -p pallas-network2 --lib 2>&1")
  res=$(printf '%s\n' "$out" | grep -E '^test result' | tail -1)
  fails=$(printf '%s\n' "$out" | grep -E '^    behavior::|^    testing::|^    [a-z_:]+$' | tr -d ' ' | tr '\n' ' ')
  err=$(printf '%s\n' "$out" | grep -cE '^error(\[|:)')
  echo "ARM $name changed_lines=$n compile_errors=$err | $res | FAILED: $fails"
}

arm M0_baseline $B/own.rs 's/\A/\/\/ baseline\n/'
arm M1_no_is_request $B/own.rs 's/if !is_request\(msg\) \|\| /if /'
arm M2_one_key_all_channels $B/own.rs 's/\.get\(&msg\.channel\(\)\)/.get(&0)/; s/contains_key\(&msg\.channel\(\)\)/contains_key(&0)/; s/insert\(msg\.channel\(\)/insert(0/; s/held\.remove\(&msg\.channel\(\)\)/held.remove(&0)/g; s/held\.remove\(&proto::blockfetch::CHANNEL_ID\)/held.remove(&0)/'
arm M3_clear_by_channel_no_cbor $B/own.rs 's/\.is_some_and\(\|held\| held\.payload\(\) == msg\.payload\(\)\)/.is_some()/'
arm M4_clear_without_accepts $B/own.rs 's/self\.own\.holds\(msg\) && self\.accepts\(msg\)/self.own.holds(msg)/g'
arm M5_inbound_clears_channel $B/initiator/mod.rs 's/(state\.apply_msg\(msg\);\n\n            all_visitors!\(self, pid, state, visit_inbound_msg\);)/state.own = Default::default();\n            $1/'
arm M6_reset_before_visitors $B/initiator/mod.rs 's/(state\.connection = ConnectionState::Disconnected;\n)\n(            all_visitors!\(self, pid, state, visit_disconnected\);\n)\n            state\.reset\(\);\n/$1            state.reset();\n$2/'
arm M7_no_requeue $B/initiator/blockfetch.rs 's/self\.requests\.push_front\(range\);/let _ = range;/'
arm M8_no_owe $B/initiator/chainsync.rs 's/state\.owe_next\(\);/{}/'
arm M9_owed_never_cleared $B/own.rs 's/self\.next_owed = false;\n//'
arm M10_no_owed_flush $B/initiator/chainsync.rs 's/state\.send_owed_next\(pid, outbound\);\n//'
arm M11_initiator_reset_keeps_own $B/initiator/mod.rs 's/self\.own = OwnSends::default\(\);\n//'
arm M12_hold_ignores_accepted $B/own.rs 's/ \|\| !accepted//'
arm M13_initiator_no_clear $B/initiator/mod.rs 's/state\.clear_own_sent\(msg\);\n//'
arm M14_responder_no_clear $B/responder/mod.rs 's/state\.clear_own_sent\(msg\);\n//'
arm M15_responder_reset_keeps_own $B/responder/mod.rs 's/self\.own = OwnSends::default\(\);\n//'
arm M16_responder_no_hold $B/own.rs 's/(OutboundQueue<ResponderBehavior>,\n    \) -> bool \{\n        let accepted = self\.accepts\(&msg\);\n\n        if )!self\.own\.hold\(&msg, accepted\)/$1false \&\& !accepted/'
arm M17_owed_set_always $B/initiator/chainsync.rs 's/if !state\.send_own\(pid, next, outbound\) \{\n                state\.owe_next\(\);\n            \}/state.send_own(pid, next, outbound);\n            state.owe_next();/'
arm M18_initiator_reset_keeps_owed $B/initiator/mod.rs 's/        self\.next_owed = false;\n//'
git checkout -q -- .
echo "DONE"
