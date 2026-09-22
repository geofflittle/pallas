#!/bin/sh
# box-test.sh <git ref> <label> <cargo args...>
#
# Puts the tree of <git ref> from the endorser block worktree on the box under
# /opt/build/pallas-eb/src-<label>, touches it so cargo cannot replay stale
# artifacts, runs the cargo command there with the private target of
# /opt/build/pallas-eb, prints progress every 30 seconds, and ends with the
# failed tests, the panic lines and every count line.
W=/Users/geofflittle/code/pallas-wt-eb
B=/opt/build/pallas-eb
ref=$1
label=$2
shift 2
sha=$(/usr/bin/git -C "$W" rev-parse "$ref") || exit 2
echo "== $ref $sha label=$label"
ssh musashi-aws "rm -rf $B/src-$label && mkdir -p $B/src-$label" || exit 2
/usr/bin/git -C "$W" archive "$sha" | ssh musashi-aws "tar -x -C $B/src-$label && echo $sha > $B/src-$label/HEAD_SHA && find $B/src-$label -exec touch {} + && echo synced" || exit 2
ssh musashi-aws "SRC=$B/src-$label $B/job.sh $label $*" < /dev/null
ssh musashi-aws "$B/wait.sh $label" < /dev/null
ssh musashi-aws "/usr/bin/grep -E '^test .* FAILED|^test result|panicked at|^  left|^ right|wrong refusal|must be refused|^error|^warning: unused|Diff in' $B/logs/$label.log | head -40"
