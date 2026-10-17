#!/bin/sh
# rebase sequence editor: keeps the autosquash order and rewrites the commit 3 message after its fixups
M=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/exfix/msg/c3.txt
T="$1"
awk -v m="$M" '
  /^pick c8b558ce/ { print "exec /usr/bin/git commit --amend -q -F " m }
  { print }
' "$T" > "$T.new" && mv "$T.new" "$T"
cat "$T" | grep -v '^#' | grep -v '^$'
