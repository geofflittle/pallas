#!/bin/sh
# usage: mk.sh <name> <ref>  fresh worktree src-<name> at <ref> with the blueprint submodule, every file touched
# a later commit arrives by: git -C repo fetch ../<bundle> "+refs/heads/<branch>:refs/heads/<branch>"
set -e
D=/opt/build/v1-r2-1022
cd $D
N=$1; R=$2
if [ ! -d repo ]; then git clone -q --bare r2.bundle repo; fi
if [ -d src-$N ]; then git -C repo worktree remove --force ../src-$N; fi
git -C repo worktree add -q --detach ../src-$N "$R"
cd src-$N
git -c protocol.file.allow=always submodule update -q --init --reference /opt/build/mapper-panics-1022/blueprint.git
echo "src-$N at $(git rev-parse HEAD) blueprint $(git -C cardano-blueprint rev-parse HEAD)"
find . -type f -not -path "./.git/*" -exec touch {} +
