#!/bin/sh
# Applies each comparer break to a copy of gen and runs the check that must catch it.
# Expected: every check prints a nonzero wrong count and every line ends in exit 1.
# usage: sh breaks/run-breaks.sh
set -u
B=$(cd "$(dirname "$0")" && pwd)
G=$B/../gen
run() {
  T=$(mktemp -d)
  cp "$G"/*.py "$T"/
  if patch -s "$T/$2" < "$B/$1"; then
    (cd "$T" && python3 -I "$3")
    echo "$1: exit $?"
  else
    echo "$1: patch does not apply"
  fi
  rm -rf "$T"
}
run rows-value.py.patch value.py check_rows.py
run rows-empty-value.py.patch value.py check_rows.py
run sets-schemacmp.py.patch schemacmp.py check_sets.py
run align-schemacmp.py.patch schemacmp.py check_align.py
run tag24-valuecmp.py.patch valuecmp.py check_listing.py
