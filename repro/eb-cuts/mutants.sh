#!/bin/sh
# usage: mutants.sh <pristine dir name> <mutant...>
# Copies leios.rs from the pristine tree into mut-<pristine>, applies one break,
# runs the leios tests, lists the failing tests and the panic lines, restores.
D=/opt/build/eb-cuts
P=$1
shift
M=mut-$P
[ -d $D/$M ] || cp -a $D/$P $D/$M
for m in "$@"; do
  cp $D/$P/pallas-traverse/src/leios.rs $D/$M/pallas-traverse/src/leios.rs
  python3 $D/mutate.py $D/$M $m || exit 1
  touch $D/$M/pallas-traverse/src/leios.rs
  $D/run.sh $M "$P-$m" cargo test -p pallas-traverse --features unstable --lib leios > /dev/null
  L=$D/logs/$P-$m.log
  echo "== $P $m $(tail -1 $L | head -1) $(/bin/grep '^test result' $L)"
  /bin/grep -E '^test .* FAILED$' $L | sed 's/^test leios::tests::/   /'
  /bin/grep -E "panicked at|^error" $L | sed 's/^/     /' | head -6
  /bin/grep -A2 "panicked at" $L | /bin/grep -v "panicked at\|^--" | sed 's/^/       /' | head -6
done
cp $D/$P/pallas-traverse/src/leios.rs $D/$M/pallas-traverse/src/leios.rs
cmp $D/$P/pallas-traverse/src/leios.rs $D/$M/pallas-traverse/src/leios.rs && echo "restored $M"
