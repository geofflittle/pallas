#!/bin/sh
# arm.sh NAME  copies tree $B (default tip) to armtree, applies arms/NAME.patch if present, touches, runs the validate tests, prints failing tests
A=/opt/build/cvsimplify-build-1016
N=$1
B=${B:-tip}
mkdir -p $A/logs
rsync -a --delete $A/$B/ $A/armtree/
if [ -f $A/arms/$N.patch ]; then
  (cd $A/armtree && patch -p1 --no-backup-if-mismatch < $A/arms/$N.patch) > $A/logs/arm-$N.patch.log 2>&1 || { echo "arm $N patch FAILED"; cat $A/logs/arm-$N.patch.log; exit 9; }
fi
find $A/armtree -exec touch {} +
W=armtree JOBNAME=arm-$N $A/run.sh cargo test -p pallas-validate --no-fail-fast ${FEATURES:+--features $FEATURES} > $A/logs/arm-$N.log 2>&1
C=$?
echo "arm $N base=$B exit=$C compiled=$(/bin/grep -cE '^ +Compiling ' $A/logs/arm-$N.log)"
/bin/grep -E '^error(\[|:)' $A/logs/arm-$N.log | /bin/grep -v 'test failed' | /bin/grep -v 'targets\? failed' | head -5
/bin/grep -E '^test .* FAILED$' $A/logs/arm-$N.log | sed 's/^test //; s/ \.\.\. FAILED$//' | sed 's/.*:://' | sort | tr '\n' ' '; echo
/bin/grep -E '^test result' $A/logs/arm-$N.log | awk '{p+=$4; f+=$6} END {print "totals passed=" p " failed=" f}'
