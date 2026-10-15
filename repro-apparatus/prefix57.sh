#!/bin/sh
# Test pallas-utxorpc at the tree on the box with the spec crate patched to rev 7b5ebd9.
# Usage: prefix57.sh <log name> <extra cargo args>
log="/opt/build/u5c-v1/$1.log"
shift
/usr/bin/ssh musashi-aws "/opt/build/u5c-v1/run.sh cargo test -p pallas-utxorpc $* --config 'patch.crates-io.utxorpc-spec.git=\"https://github.com/SundaeSwap-finance/utxorpc-spec\"' --config 'patch.crates-io.utxorpc-spec.rev=\"7b5ebd9d11b549dbe18c7aa88b20e740d25da9e1\"' > $log 2>&1; echo exit=\$?; grep -E 'test result|^error' $log; grep -A2 'name = \"utxorpc-spec\"' /opt/build/u5c-v1/pallas/Cargo.lock"
