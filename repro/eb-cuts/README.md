# Breaks for the endorser block cuts

The tree of this commit is b50a967 with the five cuts applied (the state left uncommitted in the build worktree). The scripts here are the breaks run against it.

Rerun on musashi-aws: copy this tree to `/opt/build/eb-cuts/src` without `repro/`, copy the tree of b50a967 to `/opt/build/eb-cuts/base`, put `cardano-blueprint` at 188183b37081 into both, copy the four scripts to `/opt/build/eb-cuts/`, `find` and `touch` both trees, then:

```
/opt/build/eb-cuts/mutants.sh src M11_long_delivery_ok M13_long_wrong_variant M12_count_fields_swapped M5_certifies_nothing_off
/opt/build/eb-cuts/mutants.sh src M1_body_hash_off C1_hash_reencoding C2_message_drops_announced M7_hash_envelope M8_decode_envelope
/opt/build/eb-cuts/mutants.sh base C1_hash_reencoding C2_message_drops_announced M1_body_hash_off
```

Expected verdict lines:

```
== src M11_long_delivery_ok EXIT 101 test result: FAILED. 23 passed; 1 failed   (a_long_delivery_is_refused, "a long delivery must be refused")
== src M13_long_wrong_variant EXIT 101 test result: FAILED. 23 passed; 1 failed (a_long_delivery_is_refused, "wrong refusal")
== src M12_count_fields_swapped EXIT 101 test result: FAILED. 22 passed; 2 failed (the long and short delivery tests, field asserts)
== src M5_certifies_nothing_off EXIT 101 test result: FAILED. 23 passed; 1 failed (the rewritten header test)
== src M1_body_hash_off EXIT 101 test result: FAILED. 22 passed; 2 failed
== src C1_hash_reencoding EXIT 101 test result: FAILED. 22 passed; 2 failed
== src C2_message_drops_announced EXIT 101 test result: FAILED. 23 passed; 1 failed
== src M7_hash_envelope EXIT 101 test result: FAILED. 18 passed; 6 failed
== src M8_decode_envelope EXIT 101 test result: FAILED. 18 passed; 6 failed
== base C1_hash_reencoding EXIT 101 test result: FAILED. 27 passed; 2 failed
== base C2_message_drops_announced EXIT 101 test result: FAILED. 28 passed; 1 failed
== base M1_body_hash_off EXIT 101 test result: FAILED. 25 passed; 4 failed
```

`hashes_break.py src` then `cargo test -p pallas-traverse --features unstable --lib hashes` gives `FAILED. 13 passed; 2 failed` with `left: 15 right: 16` and `left: 16 right: 17`.
