#!/bin/sh
# setup.sh, box side, D=/opt/build/pr819-review with rev.tar, parent.tar, probe819.rs in it.
set -eu
cd "$D"
mkdir -p rev parent
tar -C rev -xf rev.tar
tar -C parent -xf parent.tar
for t in rev parent; do
  I=$t/pallas-network2/src/behavior/initiator
  cp probe819.rs $I/probe819.rs
  # replace the final closing brace of the tests module with an include and the brace
  sed -i '$ d' $I/mod.rs
  printf '    include!("probe819.rs");\n}\n' >> $I/mod.rs
done
(cd rev && git init -q && git add -A && git -c user.name=r -c user.email=r@localhost commit -qm base)
find rev parent -exec touch {} +
echo "SETUP include_rev=$(grep -c 'include!("probe819.rs")' rev/pallas-network2/src/behavior/initiator/mod.rs) include_parent=$(grep -c 'include!("probe819.rs")' parent/pallas-network2/src/behavior/initiator/mod.rs)"
df -h /opt
