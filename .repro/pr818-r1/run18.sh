#!/bin/sh
# run18.sh: each slot test alone with --exact, one verdict line per test. Runs inside dk.sh.
I=behavior::initiator::tests::
R=behavior::responder::tests::
for t in ${I}blockfetch_two_passes_before_sent_send_one ${I}leiosfetch_two_passes_before_sent_send_one \
  ${I}chainsync_intersect_two_passes_before_sent_send_one ${I}keepalive_two_passes_before_sent_send_one \
  ${I}leiosnotify_two_passes_before_sent_send_one ${I}peersharing_two_passes_before_sent_send_one \
  ${I}raw_send_before_own_sent_keeps_own_range ${I}disconnect_before_sent_requeues_range \
  ${I}continue_sync_before_sent_sends_one_then_owed ${R}tx_ids_two_passes_before_sent_send_one \
  ${I}reconnect_after_disconnect_sends_again ${I}keepalive_sent_and_answered_sends_again \
  ${I}leiosnotify_sent_and_answered_sends_again ${I}leiosfetch_sent_and_answered_sends_next \
  ${I}own_keepalive_response_sends_nothing ${I}keepalive_sent_and_unanswered_sends_nothing \
  ${I}inbound_before_sent_keeps_own_keepalive ${I}reconnect_before_owed_next_owes_nothing; do
  out=$(cargo test -q -p pallas-network2 --lib -- --exact "$t" 2>&1)
  if echo "$out" | grep -q "test result: ok. 1 passed; 0 failed"; then v=ok
  elif echo "$out" | grep -q "test result: FAILED. 0 passed; 1 failed"; then v=failed
  elif echo "$out" | grep -q "test result: ok. 0 passed; 0 failed"; then v=missing
  else v=nobuild; fi
  msg=$(echo "$out" | grep -E "panicked|left:|right:|assertion|^error" | tr '\n' ' ' | cut -c1-240)
  echo "${t##*::} $v $msg"
done
