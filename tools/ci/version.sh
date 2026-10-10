#!/usr/bin/env bash
# Prints the workspace version from Cargo.toml ([workspace.package] version), e.g. 0.1.0. Used by the CI scripts for
# the manifest, the app bundle and the release tag.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$root/Cargo.toml" | head -1)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version.sh: no workspace version in Cargo.toml" >&2; exit 1; }
echo "$version"
