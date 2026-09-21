# Planted defects for the housekeeping duplicate send arms

Each patch applies to the fix commit, not to this branch. Each one keeps every
other assertion green, so the assertion named below is the only one that
refuses, which is what makes it evidence for that assertion rather than for a
count that happened to move.

Run one arm:

    git checkout fix/housekeeping-duplicate-send
    git apply arms/<patch>
    cargo test -p pallas-network2 --lib -- --test-threads=1
    git checkout -- pallas-network2

Clean tree on that commit:

    test result: ok. 94 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

## d1-peersharing-guard-removed.patch

Drops the unconfirmed check from the peer sharing sender only.

    two_passes_before_a_confirmation_send_one_request_each
    assertion `left == right` failed: peer sharing should have one request on the wire, got 2
      left: 2
     right: 1
    test result: FAILED. 93 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

## d2-leiosnotify-guard-removed.patch

Drops the unconfirmed check from the leios notify sender only.

    two_passes_before_a_confirmation_send_one_request_each
    assertion `left == right` failed: leios notify should have one request on the wire, got 2
      left: 2
     right: 1
    test result: FAILED. 93 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

## d3-unconfirmed-mark-never-cleared.patch

Never clears the mark, so the guard becomes a guard on sending at all. The
count arm stays green under this one, which is the point of the must not case.

    an_answered_request_is_asked_again_on_the_next_pass
    assertion `left == right` failed: an answered keepalive should be asked again, got 0 in []
      left: 0
     right: 1
    test result: FAILED. 93 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

## d4-inbound-violation-not-recorded.patch

Stops recording a violation for a message the peer sent. Every count assertion
stays green, so only the reason assertions move.

    a_keepalive_response_nothing_asked_for_is_a_violation
    an unrequested keepalive response should mark the peer
    a_notification_nothing_asked_for_is_a_violation
    an offer with no request outstanding should mark the peer
    violation_bans_and_disconnects
    promotion should ban the violating peer
    test result: FAILED. 91 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
