# Final fixes runners

Box layout: copy `box/*` and `probes/` (as `rprobes/`) to `/opt/build/final-fixes-1017`, with a clone of pallas at `repo/` holding the branches below (ship with `ship.sh <branch>...`).

Gates, one commit per call:

    ssh musashi-aws '/opt/build/final-fixes-1017/gate.sh <sha> <label> "fmt clippy clippy-p2 test test-p2"'
    ssh musashi-aws '/opt/build/final-fixes-1017/gate.sh <sha> <label> "wclippy wtest"'

Revert arms and expected verdicts:

    probe.sh 3c0d5f72 rv-f1 default lib every_needed_script_is_found_in_any_position
      expect exit=101, left "Err(ShelleyMA(MissingScriptWitness))" right "Ok(())"
    probe.sh 67462441 rv-f2 default lib key_deposits_refunds_and_withdrawals_balance
      expect exit=101, alonzo and babbage FAILED on the registration case
    probe.sh 54f05ae9 rv-f4 default lib an_undecodable_withdrawal_account_is_refused
      expect exit=101, left: [Ok([]), Ok([])] in babbage and conway
    probe.sh a188e178 rv-f6 default lib a_reference_script_from_an_earlier_era_is_found
      expect exit=101, left: []

Reviewer probes and arms on the heads:

    rprobes/rerun.sh 55a4bfde pr-lower-lib default probes shelley,utils
    rprobes/rerun.sh c42430bd pr-upper-lib default probes shelley,utils,alonzo_balance,babbage_balance
      expect every PROBE line Ok, except withdrawal_scripts=Err(PostAlonzo(InputDecoding)) for undecodable=true
    rprobes/rerun.sh <head> pr-p2 phase2 probes conway
      expect PROBE bad_withdrawal=true PANIC (identical to main)
    rprobes/rerun.sh 55a4bfde arm-a1 default arm a1-shelley-cert-key        expect exit=101
    rprobes/rerun.sh 55a4bfde arm-a2 phase2 arm a2-v3-deposit-boundary      expect exit=101
    rprobes/rerun.sh 55a4bfde arm-a3 default arm a3-alonzo-cert-native      expect exit=101
    rprobes/rerun.sh c42430bd arm-a4 default arm a4-bootstrap-signature     expect exit=101
    rprobes/rerun.sh c42430bd arm-a5 default arm a5-babbage-needed-language expect exit=101
    rprobes/rerun.sh 55a4bfde arm-a6 default arm a6-alonzo-collateral-trigger expect exit=0 (survives)
