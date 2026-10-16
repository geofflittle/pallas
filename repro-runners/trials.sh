#!/bin/sh
# trials.sh  merges the four main branches onto upstream main in every order where the certificate branch lands before the input eras branch, one merge commit per landing, and prints each order's verdict and final tree
G="/usr/bin/git -C /Users/geofflittle/code/pallas"
BASE=e6c38ac5
D=fix/validate-conway-deposits
C=fix/validate-conway-collateral
W=fix/validate-conway-certificate-witnesses
I=fix/validate-conway-input-eras
for order in "D C W I" "D W C I" "D W I C" "C D W I" "C W D I" "C W I D" "W D C I" "W D I C" "W C D I" "W C I D" "W I D C" "W I C D" "D C I" "C I D" "I D C" "I C D"; do
  head=$($G rev-parse $BASE)
  verdict=clean
  for b in $order; do
    eval ref=\$$b
    out=$($G merge-tree --write-tree --name-only $head $ref)
    rc=$?
    tree=$(echo "$out" | head -1)
    if [ $rc -ne 0 ]; then
      verdict="conflict at $b: $(echo "$out" | sed -n '2,20p' | tr '\n' ' ')"
      break
    fi
    head=$($G commit-tree $tree -p $head -p $ref -m "trial merge $b")
  done
  echo "order [$order] $verdict tree=$($G rev-parse $head^{tree}) head=$head"
done
