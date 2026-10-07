#!/bin/sh
# setup.sh, box side, D set, the tars and patch already in D. Makes four trees, each with its own target-<tree> at first use.
# build: the amended worktree. mut: the same as a git repo for mutants. red: 2e7bfd92 plus the new tests only. main: 0ea294c1 with the build's test modules.
set -eu
cd "$D"
mkdir build red main
tar -C build -xf build.tar
tar -C red -xf base.tar
tar -C main -xf main.tar
for t in build red main; do tar -C "$t" -xf bp.tar; done
cp run16.sh build/ && cp run16.sh main/
cp main16/initiator_mod.rs main/pallas-network2/src/behavior/initiator/mod.rs
cp main16/responder_mod.rs main/pallas-network2/src/behavior/responder/mod.rs
cp main16/testing.rs main/pallas-network2/src/testing.rs
cp -a build mut
rm mut/run16.sh build/run16.sh
cp run16.sh build/run16.sh
for t in mut red; do
  (cd "$t" && git init -q && git add -A && git -c user.name=r -c user.email=r@localhost commit -qm base)
done
(cd red && git apply "$D/tests-only.patch" && git -c user.name=r -c user.email=r@localhost commit -qam tests)
find build red main mut -exec touch {} +
echo "SETUP red_tests=$(grep -c 'fn keepalive_sent_and_unanswered_sends_nothing' red/pallas-network2/src/behavior/initiator/mod.rs) red_own_next_owed=$(grep -c 'self.own.next_owed' red/pallas-network2/src/behavior/own.rs) build_next_owed_field=$(grep -c 'pub(crate) next_owed' build/pallas-network2/src/behavior/initiator/mod.rs) bp_files=$(ls build/cardano-blueprint | wc -l)"
df -h /opt
