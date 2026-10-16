#!/bin/sh
# arms-all.sh  runs arm.sh on the baseline and on every patch in arms/ against tree $B (default tip), verdicts in logs/arms-$B.out
A=/opt/build/v3dep-1016
B=${B:-tip}
: > $A/logs/arms-$B.out
for N in baseline $(ls $A/arms | sed 's/\.patch$//' | sort); do
  B=$B $A/arm.sh $N >> $A/logs/arms-$B.out 2>&1
done
echo "arms-all $B done"
