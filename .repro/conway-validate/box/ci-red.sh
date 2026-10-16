#!/bin/sh
# ci-red.sh NAME RELPATH LOCALFILE CMD...  runs CMD on the box tree $A/$W with RELPATH replaced by LOCALFILE, then restores RELPATH from the local tree $TREE
B=root@u5c.musashi.sundae.fi; A=/opt/build/cvfix-1015
W=${W:?set W}; TREE=${TREE:?set TREE}
N=$1; P=$2; L=$3; shift 3
scp -q "$L" $B:$A/$W/"$P"; ssh -n $B "touch $A/$W/$P"
ssh -n $B "HEADN=${HEADN:-20} W=$W $A/job.sh $N $*"
scp -q "$TREE/$P" $B:$A/$W/"$P"; ssh -n $B "touch $A/$W/$P"
