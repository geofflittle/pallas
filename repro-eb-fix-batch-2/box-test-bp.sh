#!/bin/sh
# box-test-bp.sh <git ref> <label> <cargo args...>
#
# As box-test.sh, and also puts the cardano-blueprint submodule at its pinned
# commit 188183b into the tree, since git archive leaves the submodule empty
# and pallas-network2 tests read files from it.
W=/Users/geofflittle/code/pallas-wt-eb
B=/opt/build/pallas-eb
BP=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/blueprint-188183b
ref=$1
label=$2
shift 2
sha=$(/usr/bin/git -C "$W" rev-parse "$ref") || exit 2
pin=$(/usr/bin/git -C "$W" ls-tree "$sha" cardano-blueprint | awk '{print $3}')
have=$(/usr/bin/git -C "$BP" rev-parse HEAD)
[ "$pin" = "$have" ] || { echo "blueprint pin $pin, have $have"; exit 2; }
echo "== $ref $sha label=$label blueprint=$have"
ssh musashi-aws "rm -rf $B/src-$label && mkdir -p $B/src-$label" || exit 2
/usr/bin/git -C "$W" archive "$sha" | ssh musashi-aws "tar -x -C $B/src-$label" || exit 2
/usr/bin/git -C "$BP" archive HEAD | ssh musashi-aws "tar -x -C $B/src-$label/cardano-blueprint && echo $sha > $B/src-$label/HEAD_SHA && find $B/src-$label -exec touch {} + && echo synced" || exit 2
ssh musashi-aws "SRC=$B/src-$label $B/job.sh $label $*" < /dev/null
ssh musashi-aws "$B/wait.sh $label" < /dev/null
ssh musashi-aws "/usr/bin/grep -E '^test .* FAILED|^test result|panicked at|^error|^warning: unused|Diff in' $B/logs/$label.log | head -40"
