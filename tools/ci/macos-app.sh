#!/usr/bin/env bash
# Wraps an existing macOS binary into rogg.app and packs it as $DIST_DIR/rogg-macos-universal-app.tar.gz (CI spec §9).
# The Rust build is untouched: this only assembles and signs the bundle around the binary. Prints the archive path.
# ROGG_BUILD_NUMBER becomes CFBundleVersion (0 when unset). Usage: tools/ci/macos-app.sh <binary>
set -euo pipefail
die() { echo "macos-app.sh: $*" >&2; exit 1; }

[[ $# -eq 1 ]] || die "usage: macos-app.sh <binary>"
binary=$1
root=$(cd "$(dirname "$0")/../.." && pwd)
fonts=$root/crates/rogg-ui/assets/fonts
docs=("$root/README.md" "$fonts/Inter-OFL.txt" "$fonts/JetBrainsMono-OFL.txt")
icon=$root/crates/rogg-app/assets/icon/rogg.icns
for f in "$binary" "${docs[@]}" "$icon"; do [[ -f $f ]] || die "missing $f"; done

version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$root/Cargo.toml" | head -1)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "no workspace version in Cargo.toml"
build=${ROGG_BUILD_NUMBER:-0}
[[ $build =~ ^(0|[1-9][0-9]*)$ ]] || die "ROGG_BUILD_NUMBER must be a plain number, got '$build'"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app=$stage/rogg.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/rogg"
chmod 755 "$app/Contents/MacOS/rogg"
cp "${docs[@]}" "$icon" "$app/Contents/Resources/"
printf 'APPL????' > "$app/Contents/PkgInfo"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>rogg</string>
  <key>CFBundleDisplayName</key><string>rogg</string>
  <key>CFBundleIdentifier</key><string>io.rogg-log.rogg</string>
  <key>CFBundleExecutable</key><string>rogg</string>
  <key>CFBundleIconFile</key><string>rogg</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$build</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST

# Ad-hoc signing seals Info.plist and the resources with the binary. Linux CI runners (the script's tests) have no
# codesign; the bundle is then left unsigned.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$app"
  codesign --verify --deep --strict "$app"
else
  echo "macos-app.sh: codesign not found; leaving rogg.app unsigned" >&2
fi

dist=${DIST_DIR:-$root/dist}
mkdir -p "$dist"
dist=$(cd "$dist" && pwd)
out=$dist/rogg-macos-universal-app.tar.gz
# COPYFILE_DISABLE stops macOS tar from adding AppleDouble ._* files.
(cd "$stage" && COPYFILE_DISABLE=1 tar -czf "$out" rogg.app)
echo "$out"
