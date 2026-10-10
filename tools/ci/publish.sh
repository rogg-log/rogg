#!/usr/bin/env bash
# Publishes <dir> as this build's own release (CI spec §10): tag v<version>-build.<n> on the pushed commit, created as
# a draft with every file attached, then published and marked latest in one step (GitHub creates the tag then), so no
# reader sees a half-uploaded release. Keeps the newest 30 build releases and retires the old rolling main-latest.
# Needs GITHUB_SHA, GITHUB_REF, GITHUB_REPOSITORY and ROGG_BUILD_NUMBER (and GH_TOKEN for gh). Only main publishes.
# Usage: publish.sh <dir>
set -euo pipefail
die() { echo "publish.sh: $*" >&2; exit 1; }
KEEP=30

[[ $# -eq 1 ]] || die "usage: publish.sh <dir>"
dir=$1
sha=${GITHUB_SHA:?GITHUB_SHA is not set}
build=${ROGG_BUILD_NUMBER:?ROGG_BUILD_NUMBER is not set}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set}
[[ ${GITHUB_REF:-} == refs/heads/main ]] || die "only main publishes releases (this is '${GITHUB_REF:-unset}')"
[[ $build =~ ^[1-9][0-9]*$ ]] || die "ROGG_BUILD_NUMBER must be a positive number, got '$build'"
[[ -f $dir/manifest.json && -f $dir/SHA256SUMS ]] || die "$dir has no manifest.json/SHA256SUMS; run manifest.sh first"
shopt -s nullglob
archives=("$dir"/rogg-*.tar.gz "$dir"/rogg-*.zip)
(( ${#archives[@]} > 0 )) || die "no archives in $dir"
export GH_REPO=$repo

version=$("$(dirname "$0")/version.sh")
tag="v$version-build.$build"
build_tag='^v[0-9]+\.[0-9]+\.[0-9]+-build\.([0-9]+)$'

# A re-run of a run that already published changes nothing; a draft left by an interrupted publish is replaced.
if draft=$(gh release view "$tag" --json isDraft --jq .isDraft 2>/dev/null); then
  if [[ $draft == true ]]; then
    gh release delete "$tag" --yes
  else
    echo "publish.sh: $tag is already published; nothing to do"
    exit 0
  fi
fi

# Existing build releases, newest first (by build number, which CI's run number makes increasing).
existing=()
while IFS= read -r t; do
  [[ $t =~ $build_tag && $t != "$tag" ]] && existing+=("${BASH_REMATCH[1]} $t")
done < <(gh release list --limit 1000 --json tagName --jq '.[].tagName')
newest=()
if (( ${#existing[@]} > 0 )); then
  while IFS= read -r line; do newest+=("$line"); done < <(printf '%s\n' "${existing[@]}" | sort -rn)
  latest=${newest[0]%% *}
  (( build > latest )) || die "build $build is not newer than the latest published build $latest; refusing to replace it"
fi

title="rogg $version · build $build · $(date -u +%F) · ${sha:0:7}"
notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
cat > "$notes" <<NOTES
$(git log -1 --format=%s "$sha")

Commit \`$sha\`.

| File | For |
|---|---|
| \`rogg-macos-universal-app.tar.gz\` | macOS 11+, \`rogg.app\` (Apple silicon and Intel) |
| \`rogg-macos-universal.tar.gz\` | macOS 11+, the bare \`rogg\` binary |
| \`rogg-linux-x86_64.tar.gz\` | Linux x86_64, glibc 2.35+ |
| \`rogg-linux-arm64.tar.gz\` | Linux arm64, glibc 2.35+ |
| \`rogg-windows-x86_64.zip\` | Windows x86_64 |

Check downloads against \`SHA256SUMS\`; \`manifest.json\` lists every file with its checksum, size and kind.
NOTES

gh release create "$tag" "${archives[@]}" "$dir/SHA256SUMS" "$dir/manifest.json" \
  --draft --target "$sha" --title "$title" --notes-file "$notes"
gh release edit "$tag" --draft=false --latest

# Keep the newest $KEEP build releases, this one included.
for ((i = KEEP - 1; i < ${#newest[@]}; i++)); do
  gh release delete "${newest[i]#* }" --cleanup-tag --yes
done
# The rolling release used before per-build releases.
if gh release view main-latest --json isDraft --jq .isDraft >/dev/null 2>&1; then
  gh release delete main-latest --cleanup-tag --yes
fi
