#!/bin/sh
# seq.sh TODO  adds an amend with the prepared message after each target pick in a rebase todo
M=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/msg
awk -v m="$M" '
{ print }
$1 == "pick" && $2 ~ /^a0c62565/ { print "exec git commit -q --amend -F " m "/1-balance.txt" }
$1 == "pick" && $2 ~ /^cb7c7d00/ { print "exec git commit -q --amend -F " m "/2-witnesses.txt" }
$1 == "pick" && $2 ~ /^bf9805dd/ { print "exec git commit -q --amend -F " m "/3-collateral.txt" }
$1 == "pick" && $2 ~ /^afa046d6/ { print "exec git commit -q --amend -F " m "/6-v3-deposit.txt" }
' "$1" > "$1.new" && mv "$1.new" "$1"
