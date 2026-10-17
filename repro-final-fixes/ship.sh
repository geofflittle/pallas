#!/bin/sh
# usage: ship.sh <branch>...   bundles local branches over the two old heads, ships them to the box repo and compares hashes
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/final-fixes
B=$S/ship.bundle
cd /Users/geofflittle/code/pallas || exit 2
/usr/bin/git bundle create -q "$B" "$@" ^837e24cd ^7c36190a || exit 2
local_sum=$(shasum "$B" | cut -d' ' -f1)
scp -q "$B" musashi-aws:/opt/build/final-fixes-1017/ship.bundle || exit 2
remote_sum=$(ssh musashi-aws 'sha1sum /opt/build/final-fixes-1017/ship.bundle' | cut -d' ' -f1)
[ "$local_sum" = "$remote_sum" ] || { echo "hash mismatch $local_sum $remote_sum"; exit 2; }
ssh musashi-aws 'cd /opt/build/final-fixes-1017 && git -C repo fetch -q -f ../ship.bundle "refs/heads/*:refs/heads/*"' || exit 2
for b in "$@"; do
  l=$(/usr/bin/git rev-parse "$b"); r=$(ssh musashi-aws "git -C /opt/build/final-fixes-1017/repo rev-parse $b")
  echo "$b local=$l box=$r bundle=$local_sum"
done
