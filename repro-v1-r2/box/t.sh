#!/bin/sh
# usage: t.sh <name> <cargo test args...>  runs a cargo test in src-<name>, keeps the log, prints failures and the count lines
D=/opt/build/v1-r2-1022
N=$1; shift
L=$D/logs; mkdir -p $L
LOG=$L/t-$N-$(date +%s).log
echo "head $(git -C $D/src-$N rev-parse --short HEAD) args: $*"
$D/run.sh $N cargo test "$@" > $LOG 2>&1
echo "exit=$? log=$LOG"
grep -E '^error(\[E[0-9]+\])?:|^test result|^test .* FAILED$|panicked at|^  left|^ right|^<|^>|assertion' $LOG | head -40
