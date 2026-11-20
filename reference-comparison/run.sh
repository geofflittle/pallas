#!/bin/sh
# Overlays the tests of LAYER on the pallas tree HEAD with the comparison crate from SOURCE and runs them, printing the layer's table, or checks the LAYER records in DIR with the comparison crate's check-records.
# usage: run.sh HEAD SOURCE LAYER [KIND...] (SOURCE as for overlay.sh, LAYER is validation, each KIND a class or kind of change the run is limited to)
#        run.sh records SOURCE DIR LAYER [KIND...] (DIR is a records directory named by its reference pin, each KIND a kind or class the records must name)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ "${1-}" = records ]; then
  [ $# -ge 4 ] || { echo "usage: run.sh records SOURCE DIR LAYER [KIND...]" >&2; exit 2; }
  source=$2 dir=$3
  shift 3
  [ -d "$dir" ] || { echo "$dir is not a directory" >&2; exit 2; }
  case "$source" in
    path=*) exec cargo run --quiet --manifest-path "${source#path=}/Cargo.toml" --bin check-records -- "$dir" "$@" ;;
    git=*@*)
      url=${source#git=}
      bin=$(mktemp -d)
      cargo install --quiet --root "$bin" --git "${url%@*}" --rev "${url##*@}" --bin check-records ledger-oracle-judge
      if "$bin/bin/check-records" "$dir" "$@"; then st=0; else st=$?; fi
      rm -r "$bin"
      exit "$st" ;;
    *) echo "SOURCE must be path=DIR or git=URL@REV" >&2; exit 2 ;;
  esac
fi
[ $# -ge 3 ] || { echo "usage: run.sh HEAD SOURCE LAYER [KIND...]" >&2; exit 2; }
head=$(cd "$1" && pwd)
source=$2 layer=$3
shift 3
kinds=$(IFS=,; echo "$*")
rustflags=
case "$layer" in
  validation)
    crate=pallas-validate test=reference_validation features=unstable,phase2
    toolchain=nightly-2026-09-04 rustflags="--cfg pallas_dijkstra_evaluator" ;;
  *) echo "LAYER must be validation" >&2; exit 2 ;;
esac
sh "$here/overlay.sh" "$head" "$source" "$crate"
cd "$head"
RUSTFLAGS=$rustflags REFERENCE_COMPARISON_KINDS=$kinds cargo "+$toolchain" test -p "$crate" --features "$features" --test "$test" -- --nocapture
