#!/bin/sh
# usage: muts.sh <base tree> <mutant>...   runs arm.sh per mutant and appends each verdict to logs/mutants-<base>.txt
A=/opt/build/cv-exfix-1016
B=$1; shift
cd $A
for M in "$@"; do
  ./arm.sh "$B" "$M" | tee -a "$A/logs/mutants-$B.txt"
done
