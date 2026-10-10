#!/usr/bin/env bash
# Writes SHA256SUMS and manifest.json for the rogg-* archives in <dir> (CI spec §5). Consumers (install.sh, the
# updater) trust only these checksums. Usage: tools/ci/manifest.sh <dir> <build> <commit> <channel>
set -euo pipefail
die() { echo "manifest.sh: $*" >&2; exit 1; }

[[ $# -eq 4 ]] || die "usage: manifest.sh <dir> <build> <commit> <channel>"
dir=$1 build=$2 commit=$3 channel=$4
[[ -d $dir ]] || die "no directory $dir"
[[ $build =~ ^(0|[1-9][0-9]*)$ ]] || die "build must be a plain number, got '$build'"
[[ $commit =~ ^[0-9a-f]{7,40}$ ]] || die "commit must be a hex sha, got '$commit'"
[[ $channel =~ ^[a-z0-9-]+$ ]] || die "bad channel '$channel'"

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
  printf '{\n  "channel": "%s",\n  "build": %s,\n  "commit": "%s",\n  "date": "%s",\n  "assets": [\n' "$channel" "$build" "$commit" "$date"
  printf '%s\n' "$entries"
  printf '  ]\n}\n'
} > manifest.json
