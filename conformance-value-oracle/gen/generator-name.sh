#!/bin/sh
# Prints the name of the generator build-generator.sh builds from the files of oracle/ and that script, the cardano-ledger commit of ledger.rev and the image of images/conformance-gen-hs, the name holding the hash of each.
# Usage: generator-name.sh
set -eu
G=$(cd "$(dirname "$0")" && pwd)
oracle=$(cd "$G" && find oracle build-generator.sh -type f -exec sha256sum {} + | LC_ALL=C sort -k 2 | sha256sum | cut -c1-12)
ledger=$(cut -c1-12 "$G/ledger.rev")
image=$(sh "$G/image-name.sh" conformance-gen-hs)
echo "conformance-gen-$oracle-$ledger-${image#*:}"
