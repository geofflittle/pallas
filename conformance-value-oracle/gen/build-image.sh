#!/bin/sh
# Builds the image of the dir <images>/<name> under its name when docker has no image of that name, the build held to 4g of memory and swap, and prints the name, where <images> is the dir images beside this script unless given.
# Usage: build-image.sh <name> [<images dir>]
set -eu
G=$(cd "$(dirname "$0")" && pwd)
IMAGES=${2:-$G/images}
name=$(sh "$G/image-name.sh" "$1" "$IMAGES")
if ! docker image inspect "$name" >/dev/null 2>&1; then
  docker build --memory 4g --memory-swap 4g -t "$name" "$IMAGES/$1" >&2
fi
echo "$name"
