#!/bin/sh
# Run one cargo command on the box against a source dir, logging to a named
# file, refusing below 6 GiB free and echoing every 30s while it runs.
# Usage: boxrun.sh <box src dir> <log name> <cargo args...>
set -eu
dir="$1"
log="$2"
shift 2
ssh musashi-aws "sh -c '
free=\$(df --output=avail -BG / | tail -1 | tr -dc 0-9)
df -h / | tail -1
if [ \"\$free\" -lt 6 ]; then echo STOP: only \${free}G free; exit 99; fi
SRC=$dir timeout 1500 /opt/build/wave2-eb/run.sh cargo $* > /opt/build/wave2-eb/logs/$log.log 2>&1 &
pid=\$!
while kill -0 \$pid 2>/dev/null; do sleep 30; echo waiting $log; done
wait \$pid; rc=\$?
echo exit \$rc
exit \$rc
'"
