#!/bin/sh
# job.sh NAME cmd...  runs cmd in the CI image against $A/$W, logs it, prints exit code and summary lines
A=/opt/build/cvfix-1015
N=$1; shift
mkdir -p $A/logs
JOBNAME=$N $A/run.sh "$@" > $A/logs/$N.log 2>&1
C=$?
echo "job $N exit=$C compiled=$(/bin/grep -cE '^ +(Compiling|Checking) ' $A/logs/$N.log)"
/bin/grep -E '^test result|^error|^warning|FAILED|panicked|^failures:|^    [a-z_:0-9]+$|Running |Diff in' $A/logs/$N.log | head -${HEADN:-60}
exit $C
