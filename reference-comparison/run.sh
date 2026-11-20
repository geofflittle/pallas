#!/bin/sh
# Overlays the tests of LAYER on the pallas tree HEAD with the comparison crate from SOURCE and runs them, printing the layer's table, or checks the LAYER records in DIR with the comparison crate's check-records.
# usage: run.sh HEAD SOURCE LAYER (SOURCE as for overlay.sh, LAYER is decode or validation)
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
head=$(cd "$1" && pwd)
rustflags=
case "$3" in
  decode) crate=pallas-primitives test=reference_decode features=unstable toolchain=stable ;;
  validation)
    crate=pallas-validate test=reference_validation features=unstable,phase2
    toolchain=nightly-2026-09-04 rustflags="--cfg pallas_dijkstra_evaluator" ;;
  *) echo "LAYER must be decode or validation" >&2; exit 2 ;;
esac
sh "$here/overlay.sh" "$head" "$2" "$crate"
cd "$head"
RUSTFLAGS=$rustflags cargo "+$toolchain" test -p "$crate" --features "$features" --test "$test" -- --nocapture
