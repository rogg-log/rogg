#!/usr/bin/env bash
# Writes SHA256SUMS and manifest.json for the rogg-* archives in <dir> (CI spec §5). Consumers (install.sh, the
# updater) trust only these checksums. Usage: tools/ci/manifest.sh <dir> <build> <commit> <channel>
#
# FROZEN CONTRACT: every installed rogg reads this manifest to update itself (crates/rogg-core/src/update/mod.rs), so
# changing any of the following breaks self-update for every copy already installed:
#   - manifest.json at releases/latest/download/ (publish.sh marks each build latest), with the fields
#     version (string), build (JSON number, not a string), commit (string), date (string), and
#     assets[] of { name, os, arch, sha256 (hex), size (JSON number) }. Other fields (channel, kind) may be added;
#     none of these may be renamed, retyped or dropped.
#   - the release tag v<version>-build.<n>; assets are downloaded from releases/download/<tag>/<name>.
#   - the five asset names: rogg-macos-universal-app.tar.gz, rogg-macos-universal.tar.gz, rogg-linux-x86_64.tar.gz,
#     rogg-linux-arm64.tar.gz, rogg-windows-x86_64.zip.
#   - the archive roots: rogg (macOS and Linux binaries), rogg.exe (Windows), rogg.app/Contents/MacOS/rogg (the app).
set -euo pipefail
die() { echo "manifest.sh: $*" >&2; exit 1; }

[[ $# -eq 4 ]] || die "usage: manifest.sh <dir> <build> <commit> <channel>"
dir=$1 build=$2 commit=$3 channel=$4
[[ -d $dir ]] || die "no directory $dir"
[[ $build =~ ^(0|[1-9][0-9]*)$ ]] || die "build must be a plain number, got '$build'"
[[ $commit =~ ^[0-9a-f]{7,40}$ ]] || die "commit must be a hex sha, got '$commit'"
[[ $channel =~ ^[a-z0-9-]+$ ]] || die "bad channel '$channel'"
version=$("$(dirname "$0")/version.sh")

sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

cd "$dir"
shopt -s nullglob
found=(rogg-*.tar.gz rogg-*.zip)
(( ${#found[@]} > 0 )) || die "no rogg-* archives in $dir"
archives=()
while IFS= read -r a; do archives+=("$a"); done < <(printf '%s\n' "${found[@]}" | LC_ALL=C sort)

sums=""
entries=""
for a in "${archives[@]}"; do
  [[ $a =~ ^rogg-(macos|linux|windows)-([a-z0-9_]+)(-app)?\.(tar\.gz|zip)$ ]] || die "unexpected archive name '$a'"
  os=${BASH_REMATCH[1]} arch=${BASH_REMATCH[2]} suffix=${BASH_REMATCH[3]} ext=${BASH_REMATCH[4]}
  if [[ $os == windows ]]; then [[ $ext == zip ]] || die "$a: Windows archives are .zip"
  else [[ $ext == tar.gz ]] || die "$a: macOS and Linux archives are .tar.gz"; fi
  kind=binary
  if [[ -n $suffix ]]; then
    [[ $os == macos ]] || die "$a: only macOS has an app bundle"
    kind=app
  fi
  hash=$(sha256 "$a")
  size=$(wc -c < "$a" | tr -d ' ')
  sums+="$hash  $a"$'\n'
  [[ -n $entries ]] && entries+=$',\n'
  entries+="    { \"name\": \"$a\", \"os\": \"$os\", \"arch\": \"$arch\", \"kind\": \"$kind\", \"sha256\": \"$hash\", \"size\": $size }"
done

printf '%s' "$sums" > SHA256SUMS
date=$(date -u +%Y-%m-%dT%H:%M:%SZ)
{
  printf '{\n  "channel": "%s",\n  "version": "%s",\n  "build": %s,\n  "commit": "%s",\n  "date": "%s",\n  "assets": [\n' \
    "$channel" "$version" "$build" "$commit" "$date"
  printf '%s\n' "$entries"
  printf '  ]\n}\n'
} > manifest.json
