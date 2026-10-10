#!/usr/bin/env bash
# Tests for tools/ci/{package,manifest,publish}.sh. Runs under macOS /bin/bash 3.2 and on Linux CI runners.
# Usage: bash tools/ci/test.sh
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
ci=$root/tools/ci
sh=${BASH:-bash}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s: %s\n' "$1" "$2"; failures=$((failures + 1)); }

fake_binary() { printf 'fake rogg binary %s\n' "$1" > "$work/$1"; echo "$work/$1"; }

test_tar_archive_lists_exactly_the_four_files() {
  local name=${FUNCNAME[0]} dist=$work/dist-tar out list
  out=$(DIST_DIR=$dist "$sh" "$ci/package.sh" linux x86_64 "$(fake_binary rogg-linux)")
  [[ $out == "$dist/rogg-linux-x86_64.tar.gz" && -f $out ]] || { fail "$name" "printed '$out'"; return; }
  list=$(tar -tzf "$out" | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ')
  [[ $list == "Inter-OFL.txt JetBrainsMono-OFL.txt README.md rogg " ]] || { fail "$name" "listing '$list'"; return; }
  tar -tvzf "$out" | grep -E ' (\./)?rogg$' | grep -q '^-rwx' || { fail "$name" "rogg is not executable"; return; }
  pass "$name"
}

test_windows_archive_is_a_zip_with_rogg_exe() {
  local name=${FUNCNAME[0]} dist=$work/dist-zip out list
  out=$(DIST_DIR=$dist "$sh" "$ci/package.sh" windows x86_64 "$(fake_binary rogg-win)")
  [[ $out == "$dist/rogg-windows-x86_64.zip" && -f $out ]] || { fail "$name" "printed '$out'"; return; }
  list=$(unzip -Z1 "$out" | LC_ALL=C sort | tr '\n' ' ')
  [[ $list == "Inter-OFL.txt JetBrainsMono-OFL.txt README.md rogg.exe " ]] || { fail "$name" "listing '$list'"; return; }
  pass "$name"
}

test_package_refuses_missing_binary_and_unknown_os() {
  local name=${FUNCNAME[0]} dist=$work/dist-bad
  if DIST_DIR=$dist "$sh" "$ci/package.sh" linux x86_64 "$work/nope" >/dev/null 2>&1; then fail "$name" "missing binary accepted"; return; fi
  if DIST_DIR=$dist "$sh" "$ci/package.sh" beos x86_64 "$(fake_binary rogg-beos)" >/dev/null 2>&1; then fail "$name" "unknown os accepted"; return; fi
  if DIST_DIR=$dist "$sh" "$ci/package.sh" linux "x86 64" "$(fake_binary rogg-sp)" >/dev/null 2>&1; then fail "$name" "bad arch accepted"; return; fi
  [[ ! -e $dist ]] || [[ -z $(ls -A "$dist") ]] || { fail "$name" "left files in $dist"; return; }
  pass "$name"
}

make_release_dir() {
  local dir=$work/$1
  mkdir -p "$dir"
  DIST_DIR=$dir "$sh" "$ci/package.sh" macos universal "$(fake_binary rogg-mac)" >/dev/null
  DIST_DIR=$dir "$sh" "$ci/package.sh" linux x86_64 "$(fake_binary rogg-l64)" >/dev/null
  DIST_DIR=$dir "$sh" "$ci/package.sh" linux arm64 "$(fake_binary rogg-la64)" >/dev/null
  DIST_DIR=$dir "$sh" "$ci/package.sh" windows x86_64 "$(fake_binary rogg-w64)" >/dev/null
  echo "$dir"
}

test_manifest_checksums_verify_and_json_describes_the_archives() {
  local name=${FUNCNAME[0]} dir
  dir=$(make_release_dir rel)
  "$sh" "$ci/manifest.sh" "$dir" 42 0123456789abcdef0123456789abcdef01234567 main || { fail "$name" "manifest.sh failed"; return; }
  if command -v sha256sum >/dev/null 2>&1; then (cd "$dir" && sha256sum -c SHA256SUMS >/dev/null) || { fail "$name" "SHA256SUMS does not verify"; return; }
  else (cd "$dir" && shasum -a 256 -c SHA256SUMS >/dev/null) || { fail "$name" "SHA256SUMS does not verify"; return; }; fi
  [[ $(wc -l < "$dir/SHA256SUMS" | tr -d ' ') == 4 ]] || { fail "$name" "SHA256SUMS lines"; return; }
  python3 - "$dir" "$root" <<'PY' || { fail "$name" "manifest.json content"; return; }
import hashlib, json, os, sys
d = sys.argv[1]
m = json.load(open(os.path.join(d, "manifest.json")))
assert m["channel"] == "main" and m["build"] == 42 and isinstance(m["build"], int)
import re
assert m["version"] == re.search(r'^version = "([^"]+)"', open(os.path.join(sys.argv[2], "Cargo.toml")).read(), re.M).group(1), m.get("version")
assert m["commit"] == "0123456789abcdef0123456789abcdef01234567"
assert m["date"].endswith("Z") and "T" in m["date"]
names = [a["name"] for a in m["assets"]]
assert names == sorted(names) == ["rogg-linux-arm64.tar.gz", "rogg-linux-x86_64.tar.gz", "rogg-macos-universal.tar.gz", "rogg-windows-x86_64.zip"], names
for a in m["assets"]:
    p = os.path.join(d, a["name"])
    assert a["sha256"] == hashlib.sha256(open(p, "rb").read()).hexdigest(), a
    assert a["size"] == os.path.getsize(p), a
    assert (a["os"], a["arch"]) in {("linux", "arm64"), ("linux", "x86_64"), ("macos", "universal"), ("windows", "x86_64")}, a
PY
  "$sh" "$ci/manifest.sh" "$dir" 43 0123456789abcdef0123456789abcdef01234567 main || { fail "$name" "rerun failed"; return; }
  python3 -c "import json,sys; m=json.load(open(sys.argv[1])); assert len(m['assets'])==4 and m['build']==43" "$dir/manifest.json" \
    || { fail "$name" "rerun picked up SHA256SUMS or manifest.json as assets"; return; }
  pass "$name"
}

test_manifest_rejects_bad_build_numbers() {
  local name=${FUNCNAME[0]} dir b
  dir=$(make_release_dir rel-bad-build)
  for b in 007 12a "" -1; do
    if "$sh" "$ci/manifest.sh" "$dir" "$b" abc1234 main >/dev/null 2>&1; then fail "$name" "accepted build '$b'"; return; fi
  done
  if "$sh" "$ci/manifest.sh" "$dir" 1 "not-a-sha" main >/dev/null 2>&1; then fail "$name" "accepted a bad commit"; return; fi
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 'ma"in' >/dev/null 2>&1; then fail "$name" "accepted a bad channel"; return; fi
  pass "$name"
}

test_manifest_rejects_unexpected_archives() {
  local name=${FUNCNAME[0]} dir
  dir=$work/rel-odd; mkdir -p "$dir"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted an empty directory"; return; fi
  printf x > "$dir/rogg-linux.tar.gz"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted a name without arch"; return; fi
  rm "$dir/rogg-linux.tar.gz"; printf x > "$dir/rogg-windows-x86_64.tar.gz"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted a Windows tar.gz"; return; fi
  rm "$dir/rogg-windows-x86_64.tar.gz"; printf x > "$dir/rogg-linux-x86_64.zip"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted a Linux zip"; return; fi
  pass "$name"
}

# A real Mach-O where codesign exists (it cannot sign a text file as a main executable), a stand-in elsewhere.
app_binary() {
  if command -v codesign >/dev/null 2>&1 && [[ -f /usr/bin/true ]]; then cp /usr/bin/true "$work/rogg-app-bin"; echo "$work/rogg-app-bin"
  else fake_binary rogg-app-bin; fi
}

test_mac_app_archive_holds_a_signed_bundle_with_its_info_plist() {
  local name=${FUNCNAME[0]} dist=$work/dist-app out list expected x
  out=$(DIST_DIR=$dist ROGG_BUILD_NUMBER=42 "$sh" "$ci/macos-app.sh" "$(app_binary)" 2>/dev/null) || { fail "$name" "macos-app.sh failed"; return; }
  [[ $out == "$dist/rogg-macos-universal-app.tar.gz" && -f $out ]] || { fail "$name" "printed '$out'"; return; }
  list=$(tar -tzf "$out" | sed 's|^\./||' | grep -v '/$' | LC_ALL=C sort | tr '\n' ' ')
  expected="rogg.app/Contents/Info.plist rogg.app/Contents/MacOS/rogg rogg.app/Contents/PkgInfo rogg.app/Contents/Resources/Inter-OFL.txt rogg.app/Contents/Resources/JetBrainsMono-OFL.txt rogg.app/Contents/Resources/README.md rogg.app/Contents/Resources/rogg.icns "
  command -v codesign >/dev/null 2>&1 && expected="rogg.app/Contents/Info.plist rogg.app/Contents/MacOS/rogg rogg.app/Contents/PkgInfo rogg.app/Contents/Resources/Inter-OFL.txt rogg.app/Contents/Resources/JetBrainsMono-OFL.txt rogg.app/Contents/Resources/README.md rogg.app/Contents/Resources/rogg.icns rogg.app/Contents/_CodeSignature/CodeResources "
  [[ $list == "$expected" ]] || { fail "$name" "listing '$list'"; return; }
  x=$work/app-x; mkdir -p "$x"; tar -xzf "$out" -C "$x"
  [[ -x $x/rogg.app/Contents/MacOS/rogg ]] || { fail "$name" "the binary is not executable"; return; }
  [[ $(cat "$x/rogg.app/Contents/PkgInfo") == 'APPL????' ]] || { fail "$name" "PkgInfo"; return; }
  [[ $(head -c 4 "$x/rogg.app/Contents/Resources/rogg.icns") == icns ]] || { fail "$name" "rogg.icns is not an icon file"; return; }
  python3 - "$x/rogg.app/Contents/Info.plist" "$root/Cargo.toml" <<'PY' || { fail "$name" "Info.plist values"; return; }
import plistlib, re, sys
p = plistlib.load(open(sys.argv[1], "rb"))
version = re.search(r'^version = "([^"]+)"', open(sys.argv[2]).read(), re.M).group(1)
want = {"CFBundleName": "rogg", "CFBundleDisplayName": "rogg", "CFBundleIdentifier": "io.rogg-log.rogg",
        "CFBundleExecutable": "rogg", "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version,
        "CFBundleVersion": "42", "LSMinimumSystemVersion": "11.0", "NSHighResolutionCapable": True,
        "LSApplicationCategoryType": "public.app-category.developer-tools", "CFBundleIconFile": "rogg"}
for k, v in want.items():
    assert p.get(k) == v, (k, p.get(k), v)
PY
  if command -v codesign >/dev/null 2>&1; then
    codesign --verify --deep --strict "$x/rogg.app" 2>/dev/null || { fail "$name" "the bundle signature does not verify"; return; }
  fi
  pass "$name"
}

test_mac_app_build_number_defaults_to_zero_and_rejects_junk() {
  local name=${FUNCNAME[0]} dist=$work/dist-app0 out x
  out=$(env -u ROGG_BUILD_NUMBER DIST_DIR=$dist "$sh" "$ci/macos-app.sh" "$(app_binary)" 2>/dev/null) || { fail "$name" "macos-app.sh failed"; return; }
  x=$work/app0-x; mkdir -p "$x"; tar -xzf "$out" -C "$x"
  python3 -c "import plistlib,sys; assert plistlib.load(open(sys.argv[1],'rb'))['CFBundleVersion']=='0'" "$x/rogg.app/Contents/Info.plist" \
    || { fail "$name" "a local build is not version 0"; return; }
  if DIST_DIR=$work/dist-appbad ROGG_BUILD_NUMBER=007 "$sh" "$ci/macos-app.sh" "$(app_binary)" >/dev/null 2>&1; then fail "$name" "accepted build 007"; return; fi
  if DIST_DIR=$work/dist-appbad "$sh" "$ci/macos-app.sh" "$work/nope" >/dev/null 2>&1; then fail "$name" "accepted a missing binary"; return; fi
  pass "$name"
}

test_manifest_marks_the_app_and_the_binaries() {
  local name=${FUNCNAME[0]} dir
  dir=$(make_release_dir rel-kind)
  DIST_DIR=$dir "$sh" "$ci/macos-app.sh" "$(app_binary)" >/dev/null 2>&1 || { fail "$name" "macos-app.sh failed"; return; }
  "$sh" "$ci/manifest.sh" "$dir" 9 abc1234 main || { fail "$name" "manifest.sh failed"; return; }
  python3 - "$dir/manifest.json" <<'PY' || { fail "$name" "kinds"; return; }
import json, sys
m = json.load(open(sys.argv[1]))
kinds = {a["name"]: (a["os"], a["arch"], a["kind"]) for a in m["assets"]}
assert kinds == {
    "rogg-linux-arm64.tar.gz": ("linux", "arm64", "binary"),
    "rogg-linux-x86_64.tar.gz": ("linux", "x86_64", "binary"),
    "rogg-macos-universal-app.tar.gz": ("macos", "universal", "app"),
    "rogg-macos-universal.tar.gz": ("macos", "universal", "binary"),
    "rogg-windows-x86_64.zip": ("windows", "x86_64", "binary"),
}, kinds
PY
  pass "$name"
}

test_manifest_rejects_app_archives_off_macos() {
  local name=${FUNCNAME[0]} dir
  dir=$work/rel-app-odd; mkdir -p "$dir"
  printf x > "$dir/rogg-linux-x86_64-app.tar.gz"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted a Linux app"; return; fi
  rm "$dir/rogg-linux-x86_64-app.tar.gz"; printf x > "$dir/rogg-macos-universal-app.zip"
  if "$sh" "$ci/manifest.sh" "$dir" 1 abc1234 main >/dev/null 2>&1; then fail "$name" "accepted an app .zip"; return; fi
  pass "$name"
}

# A fake `gh` and `git` that log their arguments. `gh release list` prints GH_TAGS (space-separated); `gh release view
# <tag>` reports isDraft for tags in GH_DRAFTS, a published release for other tags in GH_TAGS, and fails otherwise.
# --notes-file contents are copied to $STUB_LOG.notes.
make_stubs() {
  local bin=$work/bin
  mkdir -p "$bin"
  cat > "$bin/gh" <<'STUB'
#!/usr/bin/env bash
echo "gh $*" >> "$STUB_LOG"
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do [[ ${args[i]} == --notes-file ]] && cp "${args[i+1]}" "$STUB_LOG.notes"; done
if [[ $1 == release && $2 == list ]]; then for t in ${GH_TAGS:-}; do echo "$t"; done; exit 0; fi
if [[ $1 == release && $2 == view ]]; then
  for t in ${GH_DRAFTS:-}; do [[ $t == "$3" ]] && { echo true; exit 0; }; done
  for t in ${GH_TAGS:-}; do [[ $t == "$3" ]] && { echo false; exit 0; }; done
  exit 1
fi
exit 0
STUB
  cat > "$bin/git" <<'STUB'
#!/usr/bin/env bash
echo "git $*" >> "$STUB_LOG"
if [[ $1 == log ]]; then echo "feat: the commit subject"; fi
exit 0
STUB
  chmod +x "$bin/gh" "$bin/git"
  echo "$bin"
}

line_of() { grep -n -- "$2" "$1" | head -1 | cut -d: -f1; }
SHA=0123456789abcdef0123456789abcdef01234567

# publish.sh against the stubs: publish_with <log> <build> [VAR=value...]
publish_with() {
  local log=$1 build=$2 dir bin
  shift 2
  dir=$(make_release_dir "rel-pub-$build-$RANDOM")
  "$sh" "$ci/manifest.sh" "$dir" "$build" "$SHA" main >/dev/null
  bin=$(make_stubs); : > "$log"
  env STUB_LOG="$log" PATH="$bin:$PATH" GITHUB_SHA="$SHA" ROGG_BUILD_NUMBER="$build" GITHUB_REF=refs/heads/main \
    GITHUB_REPOSITORY=rogg-log/rogg "$@" "$sh" "$ci/publish.sh" "$dir" >/dev/null 2>&1
}

test_publish_creates_a_draft_then_publishes_it_as_latest() {
  local name=${FUNCNAME[0]} log=$work/pub-new.log version tag c e f
  version=$("$sh" "$ci/version.sh"); tag="v$version-build.7"
  publish_with "$log" 7 GH_TAGS="v$version-build.5 v$version-build.6 main-latest some-other-tag" || { fail "$name" "publish.sh failed"; return; }
  c=$(line_of "$log" "gh release create $tag "); e=$(line_of "$log" "gh release edit $tag --draft=false --latest")
  [[ -n $c && -n $e && $c -lt $e ]] || { fail "$name" "create=$c publish=$e"; return; }
  grep -- "gh release create $tag " "$log" | grep -q -- "--draft --target $SHA" || { fail "$name" "not created as a draft on the commit"; return; }
  grep -q -- "--title rogg $version · build 7 · $(date -u +%F) · 0123456" "$log" || { fail "$name" "title"; return; }
  for f in rogg-macos-universal.tar.gz rogg-linux-x86_64.tar.gz rogg-linux-arm64.tar.gz rogg-windows-x86_64.zip SHA256SUMS manifest.json; do
    grep -- "gh release create $tag " "$log" | grep -q -- "/$f" || { fail "$name" "$f not attached to the draft"; return; }
  done
  grep -q "feat: the commit subject" "$log.notes" && grep -q "$SHA" "$log.notes" && grep -q "rogg-windows-x86_64.zip" "$log.notes" \
    && grep -q "SHA256SUMS" "$log.notes" || { fail "$name" "release notes"; return; }
  grep -q -- "gh release delete main-latest --cleanup-tag --yes" "$log" || { fail "$name" "main-latest kept"; return; }
  ! grep -qE -- "gh release delete (v|some-other-tag)" "$log" || { fail "$name" "deleted a release it should keep"; return; }
  ! grep -qE -- "git push|gh api" "$log" || { fail "$name" "moved a tag by hand"; return; }
  pass "$name"
}

test_publish_keeps_the_newest_30_builds() {
  local name=${FUNCNAME[0]} log=$work/pub-prune.log version tags="" i
  version=$("$sh" "$ci/version.sh")
  for i in $(seq 1 31); do tags+="v$version-build.$i "; done
  publish_with "$log" 32 GH_TAGS="$tags v0.0.1 nightly" || { fail "$name" "publish.sh failed"; return; }
  [[ $(grep -c -- "gh release delete v" "$log") == 2 ]] || { fail "$name" "$(grep -c -- "gh release delete v" "$log") build releases deleted, not 2"; return; }
  grep -q -- "gh release delete v$version-build.1 --cleanup-tag --yes" "$log" && grep -q -- "gh release delete v$version-build.2 --cleanup-tag --yes" "$log" \
    || { fail "$name" "did not delete the two oldest"; return; }
  ! grep -qE -- "gh release delete (v0.0.1|nightly)" "$log" || { fail "$name" "deleted a non-build release"; return; }
  pass "$name"
}

test_publish_skips_an_already_published_build() {
  local name=${FUNCNAME[0]} log=$work/pub-again.log version
  version=$("$sh" "$ci/version.sh")
  publish_with "$log" 7 GH_TAGS="v$version-build.7" || { fail "$name" "a re-run failed instead of skipping"; return; }
  ! grep -qE -- "release (create|edit|delete)" "$log" || { fail "$name" "changed an already published build"; return; }
  pass "$name"
}

test_publish_replaces_a_leftover_draft() {
  local name=${FUNCNAME[0]} log=$work/pub-draft.log version d c
  version=$("$sh" "$ci/version.sh")
  publish_with "$log" 7 GH_TAGS="v$version-build.7" GH_DRAFTS="v$version-build.7" || { fail "$name" "publish.sh failed"; return; }
  d=$(line_of "$log" "gh release delete v$version-build.7 --yes"); c=$(line_of "$log" "gh release create v$version-build.7 ")
  [[ -n $d && -n $c && $d -lt $c ]] || { fail "$name" "draft delete=$d create=$c"; return; }
  pass "$name"
}

test_publish_refuses_an_older_build() {
  local name=${FUNCNAME[0]} log=$work/pub-old.log version
  version=$("$sh" "$ci/version.sh")
  if publish_with "$log" 7 GH_TAGS="v$version-build.9"; then fail "$name" "published build 7 over build 9"; return; fi
  ! grep -q -- "release create" "$log" || { fail "$name" "created a release before refusing"; return; }
  pass "$name"
}

test_publish_refuses_without_manifest() {
  local name=${FUNCNAME[0]} dir
  dir=$(make_release_dir rel-nomanifest)
  if GITHUB_SHA=abc1234 ROGG_BUILD_NUMBER=1 GITHUB_REF=refs/heads/main GITHUB_REPOSITORY=rogg-log/rogg "$sh" "$ci/publish.sh" "$dir" >/dev/null 2>&1; then
    fail "$name" "published without a manifest"; return
  fi
  pass "$name"
}

test_publish_refuses_other_branches() {
  local name=${FUNCNAME[0]} dir bin log
  dir=$(make_release_dir rel-branch)
  "$sh" "$ci/manifest.sh" "$dir" 8 0123456789abcdef0123456789abcdef01234567 main >/dev/null
  bin=$(make_stubs); log=$work/publish-branch.log; : > "$log"
  if STUB_LOG=$log PATH=$bin:$PATH GITHUB_SHA=0123456789abcdef0123456789abcdef01234567 ROGG_BUILD_NUMBER=8 \
       GITHUB_REF=refs/heads/feature GITHUB_REPOSITORY=rogg-log/rogg "$sh" "$ci/publish.sh" "$dir" >/dev/null 2>&1; then
    fail "$name" "published from refs/heads/feature"; return
  fi
  ! grep -q -- "gh " "$log" || { fail "$name" "called gh before refusing"; return; }
  pass "$name"
}

# The workflow's safety rails (CI review): publish only from main and never cancelled mid-upload; builds time out;
# cargo never re-resolves Cargo.lock; checkouts don't keep the token.
test_workflow_is_guarded() {
  local name=${FUNCNAME[0]} wf=$root/.github/workflows/build.yml jobs timeouts
  grep -qx "    if: github.ref == 'refs/heads/main'" "$wf" || { fail "$name" "publish is not limited to main"; return; }
  grep -qx "      group: publish-release" "$wf" || { fail "$name" "publish has no own concurrency group"; return; }
  grep -qx "      cancel-in-progress: false" "$wf" || { fail "$name" "publish can be cancelled"; return; }
  ! grep -qE "^concurrency:" "$wf" || { fail "$name" "workflow-level concurrency would cancel publish"; return; }
  jobs=$(sed -n '/^jobs:$/,$p' "$wf" | grep -cE "^  [a-z0-9_-]+:$"); timeouts=$(grep -cE "^    timeout-minutes: [0-9]+$" "$wf")
  [[ $jobs -gt 0 && $jobs == "$timeouts" ]] || { fail "$name" "$timeouts timeouts for $jobs jobs"; return; }
  ! grep -E "cargo (build|clippy|test)" "$wf" | grep -qv -- "--locked" || { fail "$name" "a cargo command without --locked"; return; }
  [[ $(grep -c "actions/checkout@" "$wf") == $(grep -c "persist-credentials: false" "$wf") ]] || { fail "$name" "a checkout keeps the token"; return; }
  # Debug test binaries with full debuginfo filled the runner's disk on the first run (ld: Bus error, 0 MB left).
  sed -n '/^  check:$/,/^  macos:$/p' "$wf" | grep -q 'CARGO_PROFILE_DEV_DEBUG: "0"' || { fail "$name" "check builds tests with debuginfo"; return; }
  sed -n '/^  check:$/,/^  macos:$/p' "$wf" | grep -q "name: Free disk space" || { fail "$name" "check does not free disk space"; return; }
  # Builds wait for check: a failing check must not spend (10× macOS, 2× Windows) minutes on builds it can't publish.
  local job
  for job in macos linux windows-x86_64; do
    sed -n "/^  $job:\$/,/^  [a-z0-9_-]*:\$/p" "$wf" | grep -qx "    needs: check" || { fail "$name" "$job does not wait for check"; return; }
  done
  pass "$name"
}

test_scripts_are_checked_out_with_lf() {
  local name=${FUNCNAME[0]} f
  for f in tools/ci/package.sh tools/ci/macos-app.sh tools/ci/version.sh tools/ci/manifest.sh tools/ci/publish.sh tools/ci/test.sh tools/ci/linux-deps.sh tools/linux/shot.sh dev .github/workflows/build.yml; do
    [[ $(cd "$root" && git check-attr eol -- "$f" | sed 's/.*: //') == lf ]] || { fail "$name" "$f is not eol=lf"; return; }
  done
  pass "$name"
}

for t in test_tar_archive_lists_exactly_the_four_files test_windows_archive_is_a_zip_with_rogg_exe \
         test_package_refuses_missing_binary_and_unknown_os test_manifest_checksums_verify_and_json_describes_the_archives \
         test_manifest_rejects_bad_build_numbers test_manifest_rejects_unexpected_archives \
         test_publish_creates_a_draft_then_publishes_it_as_latest test_publish_keeps_the_newest_30_builds \
         test_publish_skips_an_already_published_build test_publish_replaces_a_leftover_draft test_publish_refuses_an_older_build \
         test_publish_refuses_without_manifest \
         test_publish_refuses_other_branches test_workflow_is_guarded test_scripts_are_checked_out_with_lf \
         test_mac_app_archive_holds_a_signed_bundle_with_its_info_plist test_mac_app_build_number_defaults_to_zero_and_rejects_junk \
         test_manifest_marks_the_app_and_the_binaries test_manifest_rejects_app_archives_off_macos; do
  "$t"
done
(( failures == 0 )) && echo "all CI script tests passed" || { echo "$failures failed"; exit 1; }
