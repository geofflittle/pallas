#!/bin/sh
# usage: load.sh <pallas ref>
# Replaces /opt/build/fork-rebuild2/src with the pallas tree at <ref> and its
# cardano-blueprint submodule, then touches every file so cargo rebuilds it.
set -e
pallas=/Users/geofflittle/code/pallas
base=/opt/build/fork-rebuild2
ref=$1
blueprint=$(/usr/bin/git -C $pallas ls-tree "$ref" cardano-blueprint | awk '{print $3}')
ssh musashi-aws "rm -rf $base/src && mkdir -p $base/src/cardano-blueprint"
/usr/bin/git -C $pallas archive "$ref" | ssh musashi-aws "tar -x -C $base/src"
/usr/bin/git -C $pallas/cardano-blueprint archive "$blueprint" | ssh musashi-aws "tar -x -C $base/src/cardano-blueprint"
ssh musashi-aws "find $base/src -exec touch {} + && echo $(/usr/bin/git -C $pallas rev-parse "$ref") > $base/src/.loaded-sha && cat $base/src/.loaded-sha && df -BG /opt/build | tail -1"
