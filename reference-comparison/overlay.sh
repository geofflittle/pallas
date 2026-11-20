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
# Copies SRC to DEST unless DEST holds the same bytes, so an unchanged file keeps its time and cargo rebuilds nothing for it.
put() {
  cmp -s "$1" "$2" || { mkdir -p "$(dirname "$2")" && cp "$1" "$2"; }
}
# Runs cargo add in CRATE with the other arguments, giving the manifest and the lock file back their times when their bytes are unchanged.
add() {
  pkg=$1
  shift
  saved=$(mktemp -d)
  for f in "$pkg/Cargo.toml" Cargo.lock; do
    if [ -f "$head/$f" ]; then mkdir -p "$saved/$(dirname "$f")" && cp -p "$head/$f" "$saved/$f"; fi
  done
  (cd "$head/$pkg" && cargo add --dev --quiet "$@")
  for f in "$pkg/Cargo.toml" Cargo.lock; do
    if cmp -s "$head/$f" "$saved/$f"; then touch -r "$saved/$f" "$head/$f"; fi
  done
  rm -r "$saved"
}
pin=$(head -n 1 "$head/pallas-primitives/src/dijkstra/defs.cddl" | grep -o '[0-9a-f]\{40\}')
[ -d "$here/records/$pin" ] || { echo "no records at the ledger commit $pin that HEAD's defs.cddl names" >&2; exit 2; }
for f in "$here/records/$pin"/*.jsonl; do
  put "$f" "$head/test_data/reference-comparison/${f##*/}"
done
for crate in "$@"; do
  [ -d "$here/overlay/$crate/tests" ] || { echo "no overlay for $crate" >&2; exit 2; }
  for f in $(cd "$here/overlay/$crate/tests" && find . -type f); do
    put "$here/overlay/$crate/tests/$f" "$head/$crate/tests/$f"
  done
  add "$crate" ledger-oracle-judge $from
  if [ -f "$here/overlay/$crate/dev-dependencies" ]; then
    add "$crate" $(cat "$here/overlay/$crate/dev-dependencies")
  fi
done
