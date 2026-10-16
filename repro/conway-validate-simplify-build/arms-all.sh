#!/bin/sh
# arms-all.sh  runs arm.sh on the baseline and on every patch in arms/ against tree tip, one verdict block per arm in logs/arms-all.out, then writes logs/arms-all.done
A=/opt/build/cvsimplify-build-1016
rm -f $A/logs/arms-all.done
: > $A/logs/arms-all.out
for N in baseline $(ls $A/arms | sed 's/\.patch$//' | sort); do
  B=tip $A/arm.sh $N >> $A/logs/arms-all.out 2>&1
done
touch $A/logs/arms-all.done
