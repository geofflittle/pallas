#!/bin/sh
# Copies the reference comparison tests of each CRATE and the records of the ledger commit HEAD pins into the pallas tree HEAD, and adds the comparison crate from SOURCE and the crates its dev-dependencies file names to each CRATE as dev-dependencies.
# usage: overlay.sh HEAD SOURCE CRATE... (SOURCE is "path=DIR" or "git=URL@REV")
set -eu
here=$(cd "$(dirname "$0")" && pwd)
head=$(cd "$1" && pwd)
source=$2
shift 2
case "$source" in
  path=*) from="--path ${source#path=}" ;;
  git=*@*) url=${source#git=}; from="--git ${url%@*} --rev ${url##*@}" ;;
  *) echo "SOURCE must be path=DIR or git=URL@REV" >&2; exit 2 ;;
esac
pin=$(head -n 1 "$head/pallas-primitives/src/dijkstra/defs.cddl" | grep -o '[0-9a-f]\{40\}')
[ -d "$here/records/$pin" ] || { echo "no records at the ledger commit $pin that HEAD's defs.cddl names" >&2; exit 2; }
mkdir -p "$head/test_data/reference-comparison"
cp "$here/records/$pin"/*.jsonl "$head/test_data/reference-comparison/"
for crate in "$@"; do
  [ -d "$here/overlay/$crate/tests" ] || { echo "no overlay for $crate" >&2; exit 2; }
  cp -R "$here/overlay/$crate/tests/." "$head/$crate/tests/"
  (cd "$head/$crate" && cargo add --dev --quiet ledger-oracle-judge $from)
  if [ -f "$here/overlay/$crate/dev-dependencies" ]; then
    (cd "$head/$crate" && cargo add --dev --quiet $(cat "$here/overlay/$crate/dev-dependencies"))
  fi
done
