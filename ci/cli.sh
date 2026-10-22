#!/bin/sh
# Fetches the newest rant CLI release, found as its install.sh finds it, for the end to end tests.
# Usage: ci/cli.sh <platform>, such as linux-x64. Writes cli/rant, or cli/rant.exe on Windows.
set -e
platform=$1
repo="KosmosisDire/rant-cli"
latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest")
version="${latest##*/v}"
case "$platform" in
  win-*) exe=rant.exe ;;
  *) exe=rant ;;
esac
suffix="${exe#rant}"
mkdir -p cli
curl -fsSL -o "cli/$exe" "https://github.com/$repo/releases/download/v$version/rant-$version-$platform$suffix"
chmod +x "cli/$exe"
"./cli/$exe" --version
