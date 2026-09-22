#!/bin/sh
# wait.sh <label>: prints progress every 30 seconds until the job's exit file
# appears, then prints the exit code. Kills the job's container if free disk
# drops under 5 GiB.
label=$1
log=/opt/build/pallas-eb/logs/"$label".log
while [ ! -f /opt/build/pallas-eb/logs/"$label".exit ]; do
  free=$(df --output=avail -k / | tail -1)
  echo "$(date -u +%H:%M:%S) free_kib=$free lines=$(wc -l < "$log") last=$(tail -1 "$log" | cut -c1-120)"
  if [ "$free" -lt 5242880 ]; then
    echo "free disk under 5 GiB, stopping"
    docker ps -q --filter ancestor=pallas-ci:1 --filter volume=/opt/build/pallas-eb/target | xargs -r docker kill
  fi
  sleep 30
done
echo "exit=$(cat /opt/build/pallas-eb/logs/"$label".exit)"
