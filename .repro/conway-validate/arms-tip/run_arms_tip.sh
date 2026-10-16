#!/bin/sh
# Reruns the blind lens arms against the fix branch tip.
# Needs the box area A with job.sh and run.sh, A/w-arms holding the tip sources
# (sync.sh <tree> w-arms) and this directory at A/arms-tip.
# Expected verdict lines at the tip (2026-10-15, tip db1e9cc0):
#   ARM A1_native_ref_cert Ok(())   A1c and A1s Ok(())
#   ARM A2c_reg_deposit_control Ok(())   and A2u, A2d, A2r, A2w Ok(())
#   ARM P_unreg_v1 a DCert, no PANIC   ARM P_legacy_equals_reg true
#   ARM F_result Ok(()) twice
A=${A:-/opt/build/cvfix-1015}
python3 -I $A/arms-tip/apply_arms_tip.py $A/w-arms $A/arms-tip
find $A/w-arms -exec touch {} +
W=w-arms $A/job.sh arms-p1 cargo test -p pallas-validate --lib arm_ -- --nocapture
W=w-arms $A/job.sh arms-p2 cargo test -p pallas-validate --features phase2 --lib arm_p -- --nocapture
W=w-arms $A/job.sh arms-fix cargo test -p pallas-validate --features phase2 --test conway plutus_v3_script -- --nocapture
/bin/grep -h '^ARM' $A/logs/arms-p1.log $A/logs/arms-p2.log $A/logs/arms-fix.log
