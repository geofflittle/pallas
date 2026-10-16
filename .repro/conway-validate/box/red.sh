#!/bin/sh
# red.sh NAME FILTER RELPATH:LOCALFILE...  runs FILTER on the box with each RELPATH replaced by LOCALFILE, then restores the worktree copy
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad
B=root@u5c.musashi.sundae.fi; A=/opt/build/cvfix-1015
N=$1; F=$2; shift 2
for pair in "$@"; do scp -q "${pair#*:}" $B:$A/w/"${pair%%:*}"; done
ssh $B "HEADN=80 $A/job.sh $N cargo test -p pallas-validate $FEATURES $F; /bin/grep -E 'left:|right:' $A/logs/$N.log | head -40"
for pair in "$@"; do scp -q $S/wt-cvfix/"${pair%%:*}" $B:$A/w/"${pair%%:*}"; ssh $B "touch $A/w/${pair%%:*}"; done
