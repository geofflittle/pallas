#!/bin/sh
# Checks that each image and the generator are named for the files they are built from, that vrun.sh runs the image its files name, and that build-image.sh builds an image only when docker has none of its name, with a docker that logs each call.
# Usage: image_test.sh <empty work dir>
set -u
W=$(cd "$1" && pwd)
G=$(cd "$(dirname "$0")" && pwd)
failed=0

expect() {
  if [ "$2" = "$3" ]; then
    echo "IMAGE-TEST $1 PASS"
  else
    echo "IMAGE-TEST $1 FAIL got=\"$2\" want=\"$3\""
    failed=1
  fi
}

# A copy at $W/$1 of the harness scripts, the oracle sources, ledger.rev and the image dirs, with a docker in its bin that logs each call to docker.log and reports an image present when the file present exists.
copy() {
  C=$W/$1
  rm -rf "$C"
  mkdir -p "$C/bin"
  cp "$G"/*.sh "$G/ledger.rev" "$C/"
  cp -R "$G/oracle" "$G/images" "$C/"
  printf '%s\n' '#!/bin/sh' "echo \"\$*\" >>'$C/docker.log'" \
    "if [ \"\$1 \$2\" = 'image inspect' ]; then [ -e '$C/present' ]; fi" >"$C/bin/docker"
  chmod +x "$C/bin/docker"
}

# The image vrun.sh of the copy $1 runs the generator in, for one value of one kind.
vrun_image() {
  : >"$1/docker.log"
  printf '#!/bin/sh\ncat >/dev/null\n' >"$1/probe"
  chmod +x "$1/probe"
  PATH=$1/bin:$PATH OUT=$1/out PROBE=$1/probe sh "$1/vrun.sh" 1 1 tx cddl >/dev/null 2>&1
  sed -n 's|^run .* -v [^ ]*:/work \([^ ]*\) /work/bin/conformance-gen gen .*|\1|p' "$1/docker.log"
}

# The docker calls of build-image.sh in the copy $1 for the image $2, its dir written DIR and its name $3 written NAME.
build_calls() {
  : >"$1/docker.log"
  PATH=$1/bin:$PATH sh "$1/build-image.sh" "$2" >/dev/null 2>&1 || echo "build-image exit=$?"
  sed "s|$1/images/$2|DIR|g; s|$3|NAME|g" "$1/docker.log" | paste -sd ';' -
}

copy a
image=$(sh "$W/a/image-name.sh" conformance-gen-hs)
generator=$(sh "$W/a/generator-name.sh")
# Must name each image with the hash of its files, and the generator with the hash of its sources, the ledger commit and its image's hash.
expect image_name "$(echo "$image" | sed 's/:[0-9a-f]\{12\}$/:HASH/')" conformance-gen-hs:HASH
expect ci_image_name "$(sh "$W/a/image-name.sh" pallas-ci | sed 's/:[0-9a-f]\{12\}$/:HASH/')" pallas-ci:HASH
expect generator_name "$(echo "$generator" | sed 's/-[0-9a-f]\{12\}-[0-9a-f]\{12\}-\([0-9a-f]\{12\}\)$/-HASH-LEDGER-\1/')" \
  "conformance-gen-HASH-LEDGER-${image#*:}"
# Must run the generator in the image its files name.
ran=$(vrun_image "$W/a")
expect vrun_image "$ran" "$image"

# Must not fire: the same files in another dir give the same names.
copy same
expect same_image "$(sh "$W/same/image-name.sh" conformance-gen-hs)" "$image"
expect same_generator "$(sh "$W/same/generator-name.sh")" "$generator"

# Must fire: a changed Dockerfile names another image, which vrun.sh runs, and another generator.
copy dockerfile
echo 'RUN true' >>"$W/dockerfile/images/conformance-gen-hs/Dockerfile"
changed=$(vrun_image "$W/dockerfile")
expect dockerfile_image "$(if [ -n "$changed" ] && [ "$changed" != "$ran" ]; then echo differs; else echo "same $changed"; fi)" differs
expect dockerfile_generator "$(if [ "$(sh "$W/dockerfile/generator-name.sh")" != "$generator" ]; then echo differs; else echo same; fi)" differs
# Must fire: a changed oracle source, ledger commit and build script each name another generator.
copy source
echo '-- changed' >>"$W/source/oracle/Main.hs"
expect source_generator "$(if [ "$(sh "$W/source/generator-name.sh")" != "$generator" ]; then echo differs; else echo same; fi)" differs
copy ledger
echo 0000000000000000000000000000000000000000 >"$W/ledger/ledger.rev"
expect ledger_generator "$(if [ "$(sh "$W/ledger/generator-name.sh")" != "$generator" ]; then echo differs; else echo same; fi)" differs
copy script
echo '# changed' >>"$W/script/build-generator.sh"
expect script_generator "$(if [ "$(sh "$W/script/generator-name.sh")" != "$generator" ]; then echo differs; else echo same; fi)" differs

# Must build the image under its name when docker has none, and must not build when docker has it.
expect image_built "$(build_calls "$W/a" conformance-gen-hs "$image")" \
  "image inspect NAME;build --memory 4g --memory-swap 4g -t NAME DIR"
touch "$W/a/present"
expect image_present "$(build_calls "$W/a" conformance-gen-hs "$image")" "image inspect NAME"

# Must name and build an image of a dir outside the harness by the same rule, so another repo's image dir is named by this script.
mkdir -p "$W/outside/images"
cp -R "$G/images/conformance-gen-hs" "$W/outside/images/"
expect outside_name "$(sh "$W/a/image-name.sh" conformance-gen-hs "$W/outside/images")" "$image"
rm -f "$W/a/present"
: >"$W/a/docker.log"
outside=$(PATH=$W/a/bin:$PATH sh "$W/a/build-image.sh" conformance-gen-hs "$W/outside/images" 2>/dev/null)
expect outside_built "$outside $(sed "s|$W/outside/images|OUTSIDE|g; s|$image|NAME|g" "$W/a/docker.log" | paste -sd ';' -)" \
  "$image image inspect NAME;build --memory 4g --memory-swap 4g -t NAME OUTSIDE/conformance-gen-hs"
# Must fire: a changed file in the outside dir names another image.
echo 'RUN true' >>"$W/outside/images/conformance-gen-hs/Dockerfile"
expect outside_changed "$(if [ "$(sh "$W/a/image-name.sh" conformance-gen-hs "$W/outside/images")" != "$image" ]; then echo differs; else echo same; fi)" differs

if [ "$failed" -eq 0 ]; then
  echo "IMAGE-TEST RESULT PASS"
else
  echo "IMAGE-TEST RESULT FAIL"
  exit 1
fi
