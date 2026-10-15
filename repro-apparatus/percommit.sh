#!/bin/sh
# Check pallas-utxorpc at each given commit on the box, default and unstable features.
# Usage: percommit.sh <commit>...
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad
for c in "$@"; do
  /usr/bin/git -C "$S/wt-repro" checkout -q --detach "$c" || exit 1
  (cd "$S" && sh syncdir.sh wt-repro >/dev/null) || exit 1
  for f in "" "--features unstable"; do
    log="/opt/build/u5c-v1/pc-$c$(echo $f | tr -d ' -').log"
    /usr/bin/ssh musashi-aws "/opt/build/u5c-v1/run.sh cargo check -p pallas-utxorpc --all-targets $f > $log 2>&1; echo $c [$f] exit=\$? warnings=\$(grep -c '^warning' $log); grep -E '^error' $log | head -5"
  done
done
