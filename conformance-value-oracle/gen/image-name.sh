#!/bin/sh
# Prints the name of the image built from the dir <images>/<name>, the name tagged with the hash of the dir's files, where <images> is the dir images beside this script unless given.
# Usage: image-name.sh <name> [<images dir>]
set -eu
cd "${2:-$(dirname "$0")/images}/$1"
hash=$(find . -type f -exec sha256sum {} + | LC_ALL=C sort -k 2 | sha256sum | cut -c1-12)
echo "$1:$hash"
