#!/usr/bin/env bash
# Packs one platform's build into $DIST_DIR/rogg-<os>-<arch>.<ext> (CI spec §4): the binary, README.md and the font
# licences, at the archive root. Prints the archive path. Usage: tools/ci/package.sh <os> <arch> <binary>
set -euo pipefail
die() { echo "package.sh: $*" >&2; exit 1; }

[[ $# -eq 3 ]] || die "usage: package.sh <os> <arch> <binary>"
os=$1 arch=$2 binary=$3
[[ $os =~ ^(macos|linux|windows)$ ]] || die "unknown os '$os' (macos, linux or windows)"
[[ $arch =~ ^[a-z0-9_]+$ ]] || die "bad arch '$arch'"

root=$(cd "$(dirname "$0")/../.." && pwd)
fonts=$root/crates/rogg-ui/assets/fonts
docs=("$root/README.md" "$fonts/Inter-OFL.txt" "$fonts/JetBrainsMono-OFL.txt")
for f in "$binary" "${docs[@]}"; do [[ -f $f ]] || die "missing $f"; done

exe=rogg
[[ $os == windows ]] && exe=rogg.exe
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$binary" "$stage/$exe"
chmod 755 "$stage/$exe"
cp "${docs[@]}" "$stage/"

dist=${DIST_DIR:-$root/dist}
mkdir -p "$dist"
dist=$(cd "$dist" && pwd)
name=rogg-$os-$arch
if [[ $os == windows ]]; then
  out=$dist/$name.zip
  rm -f "$out"
  if command -v zip >/dev/null 2>&1; then
    (cd "$stage" && zip -q -X "$out" *)
  elif command -v 7z >/dev/null 2>&1; then
    target=$out
    command -v cygpath >/dev/null 2>&1 && target=$(cygpath -w "$out")
    (cd "$stage" && 7z a -tzip -bd "$target" * >/dev/null)
  else
    die "need zip or 7z to make $out"
  fi
else
  out=$dist/$name.tar.gz
  # COPYFILE_DISABLE stops macOS tar from adding AppleDouble ._* files.
  (cd "$stage" && COPYFILE_DISABLE=1 tar -czf "$out" -- *)
fi
echo "$out"
