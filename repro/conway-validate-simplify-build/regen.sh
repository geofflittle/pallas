#!/bin/sh
# regen.sh SRC_PATCH NEW_NAME  applies an old arm to a copy of the tip tree and writes the exact diff against tip as arms-new/NEW_NAME.patch
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad
WT=$S/cvsimplify-wt
OUT=$S/cvsimplify-runner/arms-new
T=$(mktemp -d)
mkdir -p $T/a $T/b
for f in pallas-validate/src/phase1/conway.rs pallas-validate/src/phase1/conway/certificate_witnesses.rs pallas-validate/src/phase1/conway/registration_deposits.rs pallas-validate/src/utils.rs pallas-validate/tests/conway.rs; do
  mkdir -p $T/a/$(dirname $f) $T/b/$(dirname $f)
  cp $WT/$f $T/a/$f
  cp $WT/$f $T/b/$f
done
(cd $T/b && patch -p1 -s < "$1") || { echo "$2 apply FAILED"; rm -rf $T; exit 9; }
find $T/b -name '*.orig' -delete
(cd $T && diff -ruN a b | /usr/bin/grep -v '^diff -ruN') > $OUT/$2.patch
rm -rf $T
echo "$2 lines=$(wc -l < $OUT/$2.patch)"
