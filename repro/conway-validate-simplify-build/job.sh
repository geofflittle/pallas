#!/bin/sh
# job.sh NAME cmd...  runs cmd against tree $W, logs it, prints exit code, compiled crate count and summary lines
A=/opt/build/cvsimplify-build-1016
N=$1; shift
mkdir -p $A/logs
JOBNAME=$N $A/run.sh "$@" > $A/logs/$N.log 2>&1
C=$?
echo "job $N exit=$C compiled=$(/bin/grep -cE '^ +(Compiling|Checking) ' $A/logs/$N.log)"
/bin/grep -E '^test result|^error|^warning|FAILED|panicked|Diff in' $A/logs/$N.log | head -${HEADN:-40}
/bin/grep -E '^test result' $A/logs/$N.log | awk '{p+=$4; f+=$6} END {if (NR) print "totals passed=" p " failed=" f}'
exit $C
