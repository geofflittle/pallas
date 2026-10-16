#!/bin/sh
# red2.sh NAME FILTER RELPATH:LOCALFILE...  runs FILTER on the box tree $A/$W with each RELPATH replaced by LOCALFILE, then restores RELPATH from the local tree $TREE
B=root@u5c.musashi.sundae.fi; A=/opt/build/cvfix-1015
W=${W:?set W}; TREE=${TREE:?set TREE}
N=$1; F=$2; shift 2
for pair in "$@"; do scp -q "${pair#*:}" $B:$A/$W/"${pair%%:*}"; ssh -n $B "touch $A/$W/${pair%%:*}"; done
ssh -n $B "HEADN=80 W=$W $A/job.sh $N cargo test -p pallas-validate $FEATURES $F; /bin/grep -E 'left:|right:' $A/logs/$N.log | head -40"
for pair in "$@"; do scp -q $TREE/"${pair%%:*}" $B:$A/$W/"${pair%%:*}"; ssh -n $B "touch $A/$W/${pair%%:*}"; done
