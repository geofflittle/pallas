#!/bin/sh
# usage: iter.sh <worktree> <wip branch> <label> [jobs]   amends the wip commit, ships it and runs gate jobs on the box
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/final-fixes
WT=$1; BR=$2; LBL=$3; JOBS=${4:-"clippy test"}
cd "$WT" || exit 2
/usr/bin/git add -A
/usr/bin/git commit -q --amend --no-edit || exit 2
/usr/bin/git branch -f "$BR" HEAD
SHA=$(/usr/bin/git rev-parse HEAD)
"$S/ship.sh" "$BR" || exit 2
ssh musashi-aws "/opt/build/final-fixes-1017/gate.sh $SHA $LBL '$JOBS'"
