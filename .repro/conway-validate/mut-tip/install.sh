#!/bin/sh
# install.sh  copies the fix branch tip to the box as $A/w-mut, overlays the mutant sources, and copies arm.sh.
# Run from the laptop with the tip checked out at $TREE.
S=$(cd "$(dirname "$0")" && pwd)
A=/opt/build/cvfix-1015
B=root@u5c.musashi.sundae.fi
TREE=${TREE:?set TREE to a checkout of the fix branch tip}
rsync -a --delete --exclude .git --exclude target --exclude cardano-blueprint "$TREE/" $B:$A/w-mut/
rsync -a --delete /Users/geofflittle/code/pallas/cardano-blueprint/ $B:$A/w-mut/cardano-blueprint/ --exclude .git
V=$A/w-mut/pallas-validate/src
scp -q "$S/conway.rs" $B:$V/phase1/conway.rs
scp -q "$S/utils.rs" $B:$V/utils.rs
scp -q "$S/script_context.rs" $B:$V/phase2/script_context.rs
scp -q "$S/to_plutus_data.rs" $B:$V/phase2/to_plutus_data.rs
scp -q "$S/tx.rs" $B:$V/phase2/tx.rs
scp -q "$S/arm.sh" $B:$A/arm.sh
ssh -n $B "chmod +x $A/arm.sh && find $A/w-mut -exec touch {} +"
