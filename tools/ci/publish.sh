#!/usr/bin/env bash
# Publishes <dir> to the rolling prerelease (CI spec §5): creates it if needed, moves its tag to this commit, uploads
# the archives, then SHA256SUMS, then manifest.json last, and deletes assets that are no longer built.
# Needs GITHUB_SHA and ROGG_BUILD_NUMBER (and GH_TOKEN for gh); TAG defaults to main-latest. Usage: publish.sh <dir>
set -euo pipefail
die() { echo "publish.sh: $*" >&2; exit 1; }

[[ $# -eq 1 ]] || die "usage: publish.sh <dir>"
dir=$1
tag=${TAG:-main-latest}
sha=${GITHUB_SHA:?GITHUB_SHA is not set}
build=${ROGG_BUILD_NUMBER:?ROGG_BUILD_NUMBER is not set}
[[ -f $dir/manifest.json && -f $dir/SHA256SUMS ]] || die "$dir has no manifest.json/SHA256SUMS; run manifest.sh first"
shopt -s nullglob
archives=("$dir"/rogg-*.tar.gz "$dir"/rogg-*.zip)
(( ${#archives[@]} > 0 )) || die "no archives in $dir"

title="Build $build · ${sha:0:7}"
notes="$(git log -1 --format=%s "$sha")

Commit $sha. File list and checksums: manifest.json and SHA256SUMS."

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --prerelease --target "$sha" --title "$title" --notes "$notes"
fi
git tag -f "$tag" "$sha"
git push -f origin "refs/tags/$tag"
gh release edit "$tag" --prerelease --title "$title" --notes "$notes"

# Archives first, the manifest last: a manifest is only ever published after the files it describes.
gh release upload "$tag" "${archives[@]}" --clobber
gh release upload "$tag" "$dir/SHA256SUMS" --clobber
gh release upload "$tag" "$dir/manifest.json" --clobber

keep=" SHA256SUMS manifest.json "
for a in "${archives[@]}"; do keep+="$(basename "$a") "; done
gh release view "$tag" --json assets --jq '.assets[].name' | while IFS= read -r asset; do
  [[ $keep == *" $asset "* ]] || gh release delete-asset "$tag" "$asset" --yes
done
