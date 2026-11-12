#!/bin/sh
# Builds the generator of oracle/ against the cardano-ledger commit of ledger.rev in the image of images/conformance-gen-hs into OUTDIR under the name generator-name.sh prints, when OUTDIR has no file of that name, and prints the path.
# Usage: [JOBS=n] build-generator.sh <work dir> <out dir> [cabal dir]
# The work dir holds the cardano-ledger checkout across builds, and the cabal dir, by default the work dir's cabal-dir, the cabal store and package index.
set -eu
G=$(cd "$(dirname "$0")" && pwd)
WORK=$(mkdir -p "$1" && cd "$1" && pwd)
OUTDIR=$(mkdir -p "$2" && cd "$2" && pwd)
CABAL=$(mkdir -p "${3:-$WORK/cabal-dir}" && cd "${3:-$WORK/cabal-dir}" && pwd)
name=$(sh "$G/generator-name.sh")
if [ -x "$OUTDIR/$name" ]; then
  echo "$OUTDIR/$name"
  exit 0
fi
image=$(sh "$G/build-image.sh" conformance-gen-hs)
rev=$(cat "$G/ledger.rev")
ledger=$WORK/cardano-ledger
if [ ! -d "$ledger/.git" ]; then
  git init -q "$ledger"
fi
git -C "$ledger" fetch -q --depth 1 https://github.com/IntersectMBO/cardano-ledger "$rev"
git -C "$ledger" checkout -q -f "$rev"
rm -rf "$WORK/oracle"
cp -R "$G/oracle" "$WORK/oracle"
echo 'packages: /work/oracle' >"$ledger/cabal.project.local"
rm -f "$WORK/conformance-gen.built"
docker run --rm --log-driver none --memory 4g --memory-swap 4g --user "$(id -u):$(id -g)" -v "$WORK:/work" -v "$CABAL:/work/cabal-dir" -e CABAL_DIR=/work/cabal-dir -e HOME=/work -w /work/cardano-ledger "$image" sh -c "cabal update && cabal build -j${JOBS:-2} conformance-gen && cp \"\$(cabal list-bin conformance-gen)\" /work/conformance-gen.built" >&2
mv "$WORK/conformance-gen.built" "$OUTDIR/$name"
echo "$OUTDIR/$name"
