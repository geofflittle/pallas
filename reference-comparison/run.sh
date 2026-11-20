#!/bin/sh
# Overlays the tests of LAYER on the pallas tree HEAD with the comparison crate from SOURCE and runs them, printing the layer's table.
# usage: run.sh HEAD SOURCE LAYER (SOURCE as for overlay.sh, LAYER is decode or validation)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
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
