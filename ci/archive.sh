#!/bin/sh
# Packs the release build as the archive rant install unpacks: package.rant, the group, and
# bin/ with the manager.
# Usage: ci/archive.sh <platform>, such as linux-x64. Writes out/rant-manager-<version>-<platform>.tar.gz
set -e
platform=$1
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
name="rant-manager-$version-$platform"
rm -rf pkg && mkdir -p pkg/bin out
for f in target/release/rant-manager target/release/rant-manager.exe; do
  if [ -f "$f" ]; then cp "$f" pkg/bin/; fi
done
test -f pkg/bin/rant-manager || test -f pkg/bin/rant-manager.exe
cp package.rant Manager.run pkg/
tar -czf "out/$name.tar.gz" -C pkg package.rant Manager.run bin
echo "out/$name.tar.gz"
