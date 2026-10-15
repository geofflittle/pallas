#!/bin/sh
# Run one cargo command on the box, log it, and print the log's verdict lines.
# Usage: box.sh <log name> <cargo args...>
log="/opt/build/u5c-v1/$1.log"
shift
/usr/bin/ssh musashi-aws "/opt/build/u5c-v1/run.sh cargo $* > $log 2>&1; echo exit=\$?; grep -E '^(error|warning: unused|test result|Finished|failures:|    [a-z_:]+$)|panicked|^error\[' $log | head -60"
