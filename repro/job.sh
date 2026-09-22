#!/bin/sh
# job.sh <label> <cargo args...>: runs one cargo command through run.sh in the
# background, logging to logs/<label>.log and writing the exit code to
# logs/<label>.exit when it ends.
label=$1
shift
mkdir -p /opt/build/pallas-eb/logs /opt/build/pallas-eb/target
rm -f /opt/build/pallas-eb/logs/"$label".exit
(
  /opt/build/pallas-eb/run.sh "$@" > /opt/build/pallas-eb/logs/"$label".log 2>&1
  echo $? > /opt/build/pallas-eb/logs/"$label".exit
) &
echo started "$label"
