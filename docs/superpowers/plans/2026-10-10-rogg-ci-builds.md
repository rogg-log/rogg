# rogg CI Builds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On every push to `main`, GitHub Actions tests rogg, builds macOS universal, Linux x86_64/arm64 and Windows x86_64 archives, and publishes them with `SHA256SUMS` and `manifest.json` to the rolling `main-latest` prerelease.

**Architecture:**
- **Scripts:** the packaging, checksum/manifest and publishing logic lives in small `bash` scripts under `tools/ci/`. They run on every runner and on the Mac, and `tools/ci/test.sh` tests them locally, with a stubbed `gh` and `git` for publishing.
- **Workflow:** `.github/workflows/build.yml` is thin. It installs dependencies, builds, and calls the scripts.
- **Build number:** the app gets its build number and channel from environment variables at build time.

**Tech Stack:**
- GitHub Actions: `actions/checkout`, `actions/cache`, `actions/upload-artifact`, `actions/download-artifact`, all pinned to commit SHAs, plus the preinstalled `gh` CLI.
- `bash`, compatible with macOS's `/bin/bash` 3.2.
- Rust 1.99.0 (`rust-toolchain.toml`); `lipo` and `codesign`; `actionlint` 1.7.12 run through Apple `container`.

**Spec:** `docs/superpowers/specs/2026-10-10-rogg-ci-builds-design.md`

## Global Constraints

- **Triggers and permissions:**
  - The workflow is `.github/workflows/build.yml`. It triggers on `push` to `main` and on `workflow_dispatch`.
  - Concurrency group is `build-main` with `cancel-in-progress: true`.
  - Default permissions are `contents: read`. Only `publish` gets `contents: write`.
- **Pinned actions** (full SHA, version in a comment):
  - `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1` (v7.0.1)
  - `actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9` (v6.1.0)
  - `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9` (v7.0.2)
  - `actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333` (v8.0.2)
- **Runners:**
  - `check` and Linux x86_64 on `ubuntu-22.04`;
  - Linux arm64 on `ubuntu-22.04-arm`;
  - macOS on `macos-14`;
  - Windows on `windows-latest`;
  - `publish` on `ubuntu-22.04`.
- **Archives:**
  - Names: `rogg-macos-universal.tar.gz`, `rogg-linux-x86_64.tar.gz`, `rogg-linux-arm64.tar.gz`, `rogg-windows-x86_64.zip`. No version in names.
  - Contents, at the archive root: the binary (`rogg`, or `rogg.exe` on Windows), `README.md`, `Inter-OFL.txt` and `JetBrainsMono-OFL.txt`.
- **macOS binary:** `MACOSX_DEPLOYMENT_TARGET=11.0`. Join the two architectures with `lipo -create`, then `codesign --force --sign -`.
- **Build number:** build jobs set `ROGG_BUILD_NUMBER=${{ github.run_number }}` and `ROGG_BUILD_CHANNEL=main`.
- **Release:** the tag is `main-latest`, published as a prerelease with the title `Build <n> · <short sha>`. Upload order: archives, then `SHA256SUMS`, then `manifest.json` last. Assets not in the new set are deleted.
- **`manifest.json` fields:**
  - top level: `channel`, `build` (a JSON number), `commit` (the full sha), `date` (UTC ISO-8601), `assets`;
  - each asset: `name`, `os`, `arch`, `sha256`, `size`, listed in name order.
- **Local builds:** the About dialog shows `local` when the build variables are unset. The macOS goldens are unchanged, and `./dev check` stays green.
- **Pushing:** pushing to `origin` is an outward action. The controller asks the user before every push.

## Review Focus

1. **A Windows checkout turns the scripts' line endings into CRLF.** Git on the Windows runner converts line endings by default, and `bash` then fails on `$'\r'`. Expected: `.gitattributes` forces LF for `*.sh`, `dev` and the workflow. Task 1: `test_scripts_are_checked_out_with_lf`.
2. **A malformed build number** (for example `007`, `12a` or empty) would make `manifest.json` invalid JSON, or wrong for the updater. Expected: `manifest.sh` refuses. Task 1: `test_manifest_rejects_bad_build_numbers`.
3. **A reader fetches during publishing.** It must never get a manifest describing files that aren't uploaded yet, and assets from an older build layout must not linger. Expected: archives, then `SHA256SUMS`, then `manifest.json`, then the stale assets deleted. Task 1: `test_publish_uploads_manifest_last_and_prunes_stale_assets`.
4. **An archive with the wrong name or extension** (a Windows `.tar.gz`, a missing arch) would publish a manifest the installer can't trust. Expected: `manifest.sh` refuses. Task 1: `test_manifest_rejects_unexpected_archives`.
5. **macOS `tar` adds AppleDouble `._*` files** to archives, and `/bin/bash` 3.2 lacks bash 4 features. Either would make local runs differ from CI. Expected: the exact archive listing, and the tests run under `/bin/bash`. Task 1: `test_tar_archive_lists_exactly_the_four_files`, run by `./dev ci-test` with `/bin/bash`.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `tools/ci/package.sh` | create | One platform's archive into `$DIST_DIR` (default `dist/`) |
| `tools/ci/manifest.sh` | create | `SHA256SUMS` and `manifest.json` for the archives in a directory |
| `tools/ci/publish.sh` | create | Update the `main-latest` prerelease through `gh` and `git` |
| `tools/ci/linux-deps.sh` | create | Install gpui's Linux build dependencies on an Ubuntu runner |
| `tools/ci/test.sh` | create | Local tests for the three CI scripts (stubbed `gh`/`git`) |
| `.gitattributes` | create | LF line endings for scripts |
| `.github/workflows/build.yml` | create | The workflow |
| `crates/rogg-app/build.rs` | modify | Pass `ROGG_BUILD_NUMBER` and `ROGG_BUILD_CHANNEL` through |
| `crates/rogg-app/src/about.rs` | modify | `BuildInfo.build` and the About "Build" row |
| `dev`, `justfile` | modify | `ci-test`, `ci-lint` |
| `.gitignore` | modify | `dist/` |
| `README.md` | modify | A "Builds" section (CI, the release URL, the manifest) |

---

### Task 1: CI scripts with local tests

**Files:**
- Create: `tools/ci/package.sh`, `tools/ci/manifest.sh`, `tools/ci/publish.sh`, `tools/ci/test.sh`, `.gitattributes`
- Modify: `dev` (the `ci-test` command, help and dispatch), `justfile`, `.gitignore`

**Interfaces:**
- **Produces:**
  - `tools/ci/package.sh <os> <arch> <binary>` writes `$DIST_DIR/rogg-<os>-<arch>.<ext>` and prints its path. `DIST_DIR` defaults to `<repo>/dist`. `os` is one of `macos|linux|windows`; `arch` matches `[a-z0-9_]+`.
  - `tools/ci/manifest.sh <dir> <build> <commit> <channel>` writes `<dir>/SHA256SUMS` and `<dir>/manifest.json`.
  - `tools/ci/publish.sh <dir>` reads `TAG` (default `main-latest`), `GITHUB_SHA` and `ROGG_BUILD_NUMBER`, and calls `gh` and `git`.
  - `./dev ci-test`.
- **All scripts** must run under macOS `/bin/bash` 3.2: no `mapfile`, no associative arrays, no `${var,,}`.

- [ ] **Step 1: Write the failing tests**

Create `tools/ci/test.sh`:

```bash
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
  python3 - "$dir" <<'PY' || { fail "$name" "manifest.json content"; return; }
import hashlib, json, os, sys
d = sys.argv[1]
m = json.load(open(os.path.join(d, "manifest.json")))
assert m["channel"] == "main" and m["build"] == 42 and isinstance(m["build"], int)
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

# A fake `gh` and `git` that log their arguments; `gh release view <tag>` succeeds only when GH_RELEASE_EXISTS=1.
make_stubs() {
  local bin=$work/bin
  mkdir -p "$bin"
  cat > "$bin/gh" <<'STUB'
#!/usr/bin/env bash
echo "gh $*" >> "$STUB_LOG"
if [[ $1 == release && $2 == view ]]; then
  if [[ " $* " == *" --json "* ]]; then printf '%s\n' rogg-macos-universal.tar.gz rogg-linux-x86_64.tar.gz old-layout.zip manifest.json SHA256SUMS; exit 0; fi
  [[ ${GH_RELEASE_EXISTS:-0} == 1 ]]; exit
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

test_publish_uploads_manifest_last_and_prunes_stale_assets() {
  local name=${FUNCNAME[0]} dir bin log a s m
  dir=$(make_release_dir rel-pub)
  "$sh" "$ci/manifest.sh" "$dir" 7 0123456789abcdef0123456789abcdef01234567 main >/dev/null
  bin=$(make_stubs); log=$work/publish-new.log; : > "$log"
  STUB_LOG=$log PATH=$bin:$PATH GITHUB_SHA=0123456789abcdef0123456789abcdef01234567 ROGG_BUILD_NUMBER=7 GH_RELEASE_EXISTS=0 \
    "$sh" "$ci/publish.sh" "$dir" >/dev/null || { fail "$name" "publish.sh failed"; return; }
  grep -q -- "gh release create main-latest --prerelease --target 0123456789abcdef0123456789abcdef01234567" "$log" || { fail "$name" "no create"; return; }
  grep -q -- "git push -f origin refs/tags/main-latest" "$log" || { fail "$name" "tag not pushed"; return; }
  grep -q -- "--title Build 7 · 0123456" "$log" || { fail "$name" "title"; return; }
  a=$(line_of "$log" "upload main-latest .*rogg-windows-x86_64.zip"); s=$(line_of "$log" "upload main-latest .*SHA256SUMS"); m=$(line_of "$log" "upload main-latest .*manifest.json")
  [[ -n $a && -n $s && -n $m && $a -lt $s && $s -lt $m ]] || { fail "$name" "upload order archives=$a sums=$s manifest=$m"; return; }
  grep -q -- "delete-asset main-latest old-layout.zip --yes" "$log" || { fail "$name" "stale asset kept"; return; }
  ! grep -qE -- "delete-asset main-latest (rogg-|manifest.json|SHA256SUMS)" "$log" || { fail "$name" "deleted a current asset"; return; }
  log=$work/publish-existing.log; : > "$log"
  STUB_LOG=$log PATH=$bin:$PATH GITHUB_SHA=0123456789abcdef0123456789abcdef01234567 ROGG_BUILD_NUMBER=7 GH_RELEASE_EXISTS=1 \
    "$sh" "$ci/publish.sh" "$dir" >/dev/null || { fail "$name" "publish.sh failed on an existing release"; return; }
  ! grep -q -- "release create" "$log" || { fail "$name" "re-created an existing release"; return; }
  grep -q -- "gh release edit main-latest" "$log" || { fail "$name" "no edit"; return; }
  pass "$name"
}

test_publish_refuses_without_manifest() {
  local name=${FUNCNAME[0]} dir
  dir=$(make_release_dir rel-nomanifest)
  if GITHUB_SHA=abc1234 ROGG_BUILD_NUMBER=1 "$sh" "$ci/publish.sh" "$dir" >/dev/null 2>&1; then fail "$name" "published without a manifest"; return; fi
  pass "$name"
}

test_scripts_are_checked_out_with_lf() {
  local name=${FUNCNAME[0]} f
  for f in tools/ci/package.sh tools/ci/manifest.sh tools/ci/publish.sh tools/ci/test.sh tools/ci/linux-deps.sh tools/linux/shot.sh dev .github/workflows/build.yml; do
    [[ $(cd "$root" && git check-attr eol -- "$f" | sed 's/.*: //') == lf ]] || { fail "$name" "$f is not eol=lf"; return; }
  done
  pass "$name"
}

for t in test_tar_archive_lists_exactly_the_four_files test_windows_archive_is_a_zip_with_rogg_exe \
         test_package_refuses_missing_binary_and_unknown_os test_manifest_checksums_verify_and_json_describes_the_archives \
         test_manifest_rejects_bad_build_numbers test_manifest_rejects_unexpected_archives \
         test_publish_uploads_manifest_last_and_prunes_stale_assets test_publish_refuses_without_manifest \
         test_scripts_are_checked_out_with_lf; do
  "$t"
done
(( failures == 0 )) && echo "all CI script tests passed" || { echo "$failures failed"; exit 1; }
```

Add to `dev`:
- help, in a new section after the Windows section:

  ```
  CI
    ci-test                       Test the CI scripts in tools/ci (archives, checksums, manifest, publishing)
  ```
- the function, next to the other `cmd_*` functions:

  ```zsh
  # The CI scripts must run under macOS's /bin/bash 3.2 as well as the runners' bash, so test with /bin/bash.
  cmd_ci_test() { step "CI script tests"; run /bin/bash tools/ci/test.sh; }
  ```
- dispatch: `ci-test) cmd_ci_test ;;`

(`ci-lint` is added in Task 3.)

Add to `justfile`:

```
ci-test:
    @./dev ci-test
```

Add `dist/` to `.gitignore`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `./dev ci-test`
Expected: FAIL. The run stops at the first test with a non-zero exit, because `tools/ci/package.sh` is missing and `set -e` aborts on the failed command substitution. Once the scripts exist (Step 7), every test reports `ok`/`FAIL` individually.

- [ ] **Step 3: Implement `.gitattributes`**

```
# Scripts must keep LF endings on every checkout (Windows runners convert to CRLF by default, which breaks bash).
*.sh text eol=lf
dev text eol=lf
*.yml text eol=lf
justfile text eol=lf
```

- [ ] **Step 4: Implement `tools/ci/package.sh`**

```bash
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
```

- [ ] **Step 5: Implement `tools/ci/manifest.sh`**

```bash
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
  [[ $a =~ ^rogg-(macos|linux|windows)-([a-z0-9_]+)\.(tar\.gz|zip)$ ]] || die "unexpected archive name '$a'"
  os=${BASH_REMATCH[1]} arch=${BASH_REMATCH[2]} ext=${BASH_REMATCH[3]}
  if [[ $os == windows ]]; then [[ $ext == zip ]] || die "$a: Windows archives are .zip"
  else [[ $ext == tar.gz ]] || die "$a: macOS and Linux archives are .tar.gz"; fi
  hash=$(sha256 "$a")
  size=$(wc -c < "$a" | tr -d ' ')
  sums+="$hash  $a"$'\n'
  [[ -n $entries ]] && entries+=$',\n'
  entries+="    { \"name\": \"$a\", \"os\": \"$os\", \"arch\": \"$arch\", \"sha256\": \"$hash\", \"size\": $size }"
done

printf '%s' "$sums" > SHA256SUMS
date=$(date -u +%Y-%m-%dT%H:%M:%SZ)
{
  printf '{\n  "channel": "%s",\n  "build": %s,\n  "commit": "%s",\n  "date": "%s",\n  "assets": [\n' "$channel" "$build" "$commit" "$date"
  printf '%s\n' "$entries"
  printf '  ]\n}\n'
} > manifest.json
```

- [ ] **Step 6: Implement `tools/ci/publish.sh`**

```bash
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
```

Make all four scripts executable (`chmod +x tools/ci/*.sh`).

- [ ] **Step 7: Run the tests to verify they pass**

Run: `./dev ci-test`
Expected: every line `ok …` and `all CI script tests passed`. Two exceptions:
- `test_scripts_are_checked_out_with_lf` checks `tools/ci/linux-deps.sh` and `.github/workflows/build.yml`. `git check-attr` reports attributes by pattern even for files that don't exist yet, so it passes now.
- If `zip` is missing on the Mac, the zip test uses `7z`, which is also missing. In that case report it. macOS ships `zip`, so this shouldn't happen.

Also run `./dev check`. Expected: green and unchanged.

- [ ] **Step 8: Commit**

```bash
git add tools/ci .gitattributes .gitignore dev justfile
git commit -m "build(ci): packaging, manifest and publishing scripts with local tests"
```

### Task 2: Build number and channel in the app

**Files:**
- Modify: `crates/rogg-app/build.rs`, `crates/rogg-app/src/about.rs`
- Test: unit tests in `crates/rogg-app/src/about.rs`

**Interfaces:**
- **Produces:**
  - compile-time env `ROGG_BUILD_NUMBER` / `ROGG_BUILD_CHANNEL`, when set at build time;
  - `about::BuildInfo { version, commit, date, rustc, build: String }`;
  - `about::describe(commit, date, rustc, number, channel) -> BuildInfo`;
  - `about::LOCAL: &str = "local"`.
  - The About dialog's build rows become `Build`, `Commit`, `Built`, `Rust`.

- [ ] **Step 1: Write the failing test**

In `crates/rogg-app/src/about.rs` tests:
- add `None, None` to the two existing `describe(...)` calls in `missing_build_details_read_unknown`;
- add:

```rust
    #[test]
    fn ci_builds_show_their_number_and_channel_and_local_builds_say_local() {
        assert_eq!(describe(None, None, None, Some("123"), Some("main")).build, "123 (main)");
        assert_eq!(describe(None, None, None, Some("123"), None).build, "123");
        assert_eq!(describe(None, None, None, None, Some("main")).build, LOCAL);
        assert_eq!(describe(None, None, None, None, None).build, LOCAL);
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev test app about`
Expected: compile error (`describe` takes 3 arguments; `LOCAL` and `build` unknown).

- [ ] **Step 3: Implement**

In `crates/rogg-app/build.rs` `main`, extend `values`, and declare the env reruns:

```rust
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let values = [
        ("ROGG_GIT_COMMIT", output("git", &["rev-parse", "--short", "HEAD"])),
        ("ROGG_BUILD_DATE", Some(today())),
        ("ROGG_RUSTC_VERSION", output(&rustc, &["--version"])),
        // Set by CI (.github/workflows/build.yml): the run number and channel the updater compares against.
        ("ROGG_BUILD_NUMBER", env("ROGG_BUILD_NUMBER")),
        ("ROGG_BUILD_CHANNEL", env("ROGG_BUILD_CHANNEL")),
    ];
```

Next to the other `rerun-if` lines:

```rust
    println!("cargo:rerun-if-env-changed=ROGG_BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=ROGG_BUILD_CHANNEL");
```

Update the module doc's first sentence to mention the CI build number and channel.

In `crates/rogg-app/src/about.rs`:

```rust
/// What a build made outside CI shows in place of a build number.
pub const LOCAL: &str = "local";

pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub date: &'static str,
    pub rustc: &'static str,
    /// `123 (main)` for a CI build, `local` otherwise.
    pub build: String,
}

/// Build details captured by `build.rs`.
pub fn build_info() -> BuildInfo {
    describe(
        option_env!("ROGG_GIT_COMMIT"),
        option_env!("ROGG_BUILD_DATE"),
        option_env!("ROGG_RUSTC_VERSION"),
        option_env!("ROGG_BUILD_NUMBER"),
        option_env!("ROGG_BUILD_CHANNEL"),
    )
}

/// `unknown` for each detail the build could not find; `local` when no CI build number was set.
pub fn describe(
    commit: Option<&'static str>,
    date: Option<&'static str>,
    rustc: Option<&'static str>,
    number: Option<&'static str>,
    channel: Option<&'static str>,
) -> BuildInfo {
    let build = match (number, channel) {
        (Some(n), Some(c)) => format!("{n} ({c})"),
        (Some(n), None) => n.to_string(),
        (None, _) => LOCAL.to_string(),
    };
    BuildInfo { version: env!("CARGO_PKG_VERSION"), commit: commit.unwrap_or(UNKNOWN), date: date.unwrap_or(UNKNOWN), rustc: rustc.unwrap_or(UNKNOWN), build }
}
```

In `view`:

```rust
        build: vec![("Build".into(), b.build.into()), ("Commit".into(), b.commit.into()), ("Built".into(), b.date.into()), ("Rust".into(), b.rustc.into())],
```

- [ ] **Step 4: Run the tests**

Run: `./dev test app about`, then `./dev check`.
Expected: PASS, with visual-check at 0.000% (the About dialog has no golden).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app/build.rs crates/rogg-app/src/about.rs
git commit -m "feat(app): build number and channel from CI in the About dialog"
```

### Task 3: The workflow, its lint, and docs

**Files:**
- Create: `.github/workflows/build.yml`, `tools/ci/linux-deps.sh`
- Modify: `dev` (`ci-lint`), `justfile`, `README.md`

**Interfaces:**
- **Consumes:**
  - `tools/ci/package.sh <os> <arch> <binary>` (writes `dist/…`);
  - `tools/ci/manifest.sh <dir> <build> <commit> <channel>`;
  - `tools/ci/publish.sh <dir>` (env `GITHUB_SHA`, `ROGG_BUILD_NUMBER`, `GH_TOKEN`);
  - `ROGG_BUILD_NUMBER` / `ROGG_BUILD_CHANNEL` (Task 2).
- **Produces:** `./dev ci-lint`.

- [ ] **Step 1: `tools/ci/linux-deps.sh`**

```bash
#!/usr/bin/env bash
# Installs gpui's Linux build dependencies on an Ubuntu CI runner (the -dev packages of tools/linux/Containerfile).
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config cmake clang \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcursor-dev libxrandr-dev libxi-dev \
  libfontconfig1-dev libfreetype6-dev libvulkan-dev libssl-dev
```

Run `chmod +x tools/ci/linux-deps.sh`.

- [ ] **Step 2: `.github/workflows/build.yml`**

```yaml
# Tests rogg and builds it for macOS, Linux and Windows on every push to main, then publishes the builds to the
# rolling `main-latest` prerelease (docs/superpowers/specs/2026-10-10-rogg-ci-builds-design.md).
name: build

on:
  push:
    branches: [main]
  workflow_dispatch:

concurrency:
  group: build-main
  cancel-in-progress: true

permissions:
  contents: read

defaults:
  run:
    shell: bash

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  ROGG_BUILD_NUMBER: ${{ github.run_number }}
  ROGG_BUILD_CHANNEL: main

jobs:
  check:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: Linux build dependencies
        run: tools/ci/linux-deps.sh
      - name: Rust toolchain (rust-toolchain.toml)
        run: rustup toolchain install
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: check-${{ runner.os }}-${{ runner.arch }}-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml') }}
          restore-keys: check-${{ runner.os }}-${{ runner.arch }}-
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Tests
        run: cargo test --workspace
      - name: CI script tests
        run: bash tools/ci/test.sh

  macos:
    runs-on: macos-14
    env:
      MACOSX_DEPLOYMENT_TARGET: "11.0"
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: Rust toolchain and both Mac targets
        run: |
          rustup toolchain install
          rustup target add aarch64-apple-darwin x86_64-apple-darwin
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: macos-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml') }}
          restore-keys: macos-
      - name: Build Apple silicon
        run: cargo build --release -p rogg-app --target aarch64-apple-darwin
      - name: Build Intel
        run: cargo build --release -p rogg-app --target x86_64-apple-darwin
      - name: Universal binary, ad-hoc signed
        run: |
          mkdir -p target/universal
          lipo -create -output target/universal/rogg \
            target/aarch64-apple-darwin/release/rogg target/x86_64-apple-darwin/release/rogg
          codesign --force --sign - target/universal/rogg
          codesign --verify --verbose target/universal/rogg
          archs=$(lipo -archs target/universal/rogg)
          [[ $archs == *x86_64* && $archs == *arm64* ]] || { echo "unexpected architectures: $archs"; exit 1; }
      - name: Package
        run: tools/ci/package.sh macos universal target/universal/rogg
      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: rogg-macos-universal
          path: dist/rogg-macos-universal.tar.gz
          retention-days: 1
          if-no-files-found: error

  linux:
    name: linux-${{ matrix.arch }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - arch: x86_64
            runner: ubuntu-22.04
          - arch: arm64
            runner: ubuntu-22.04-arm
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: Linux build dependencies
        run: tools/ci/linux-deps.sh
      - name: Rust toolchain (rust-toolchain.toml)
        run: rustup toolchain install
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: linux-${{ matrix.arch }}-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml') }}
          restore-keys: linux-${{ matrix.arch }}-
      - name: Build
        run: cargo build --release -p rogg-app
      - name: Package
        run: tools/ci/package.sh linux ${{ matrix.arch }} target/release/rogg
      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: rogg-linux-${{ matrix.arch }}
          path: dist/rogg-linux-${{ matrix.arch }}.tar.gz
          retention-days: 1
          if-no-files-found: error

  windows-x86_64:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: Rust toolchain (rust-toolchain.toml)
        run: rustup toolchain install
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: windows-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml') }}
          restore-keys: windows-
      - name: Build
        run: cargo build --release -p rogg-app
      - name: Package
        run: bash tools/ci/package.sh windows x86_64 target/release/rogg.exe
      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: rogg-windows-x86_64
          path: dist/rogg-windows-x86_64.zip
          retention-days: 1
          if-no-files-found: error

  publish:
    needs: [check, macos, linux, windows-x86_64]
    runs-on: ubuntu-22.04
    permissions:
      contents: write
    env:
      GH_TOKEN: ${{ github.token }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2
        with:
          pattern: rogg-*
          path: dist
          merge-multiple: true
      - name: Checksums and manifest
        run: tools/ci/manifest.sh dist "$ROGG_BUILD_NUMBER" "$GITHUB_SHA" "$ROGG_BUILD_CHANNEL"
      - name: Publish main-latest
        run: tools/ci/publish.sh dist
```

- [ ] **Step 3: `./dev ci-lint`**

Add to `dev`:

```zsh
ACTIONLINT_IMAGE=rhysd/actionlint:1.7.12

# Lints .github/workflows with actionlint from its official image, so nothing is installed on the Mac.
cmd_ci_lint() {
  need_container
  step "actionlint"
  run container run --rm --volume "$ROOT:/repo" --workdir /repo "$ACTIONLINT_IMAGE" -color
}
```

Add the help line under the `CI` section from Task 1:

```
  ci-lint                       Lint .github/workflows with actionlint (container image, nothing installed)
```

Add the dispatch case `ci-lint) cmd_ci_lint ;;`, and a `ci-lint` recipe to the `justfile`.

- [ ] **Step 4: Lint**

Run: `./dev ci-lint`
Expected: no findings. The image runs shellcheck on `run:` blocks, so fix any warnings it reports in the YAML or the scripts.
- If the image can't be pulled (no network), report BLOCKED.
- If the image has no arm64 variant, add `--arch amd64` to `container run` (it runs under Rosetta) and note it.

Also run `./dev ci-test`. It now includes the real `linux-deps.sh` and `build.yml` in the LF check.

- [ ] **Step 5: README "Builds" section**

Append to `README.md`:

```markdown
## Builds

Every push to `main` runs [`.github/workflows/build.yml`](.github/workflows/build.yml). It runs clippy and the
tests on Linux, then builds:

| File | Platform |
|---|---|
| `rogg-macos-universal.tar.gz` | macOS 11+, Apple silicon and Intel (ad-hoc signed, not notarised) |
| `rogg-linux-x86_64.tar.gz` | Linux x86_64, glibc 2.35+ |
| `rogg-linux-arm64.tar.gz` | Linux arm64, glibc 2.35+ |
| `rogg-windows-x86_64.zip` | Windows x86_64 |

They are published, with `SHA256SUMS` and `manifest.json` (build number, commit, checksums), to the rolling
prerelease `main-latest`: `https://github.com/rogg-log/rogg/releases/download/main-latest/<file>`. The manifest is
uploaded last; check every download against it. While the repository is private, downloads need a GitHub token.

`./dev ci-test` tests the packaging scripts locally; `./dev ci-lint` lints the workflow.
```

- [ ] **Step 6: Gate and commit**

Run: `./dev ci-lint`, `./dev ci-test`, `./dev check`.
Expected: all green.

```bash
git add .github tools/ci/linux-deps.sh dev justfile README.md
git commit -m "build(ci): GitHub Actions workflow building and publishing macOS, Linux and Windows"
```

### Task 4: First run on GitHub (controller, with the user)

This task is not dispatched to a subagent. The controller does it after merging Tasks 1–3 to `main`, with the user's approval at each push.

- [ ] **Step 1: Ask the user before pushing.** Say which commits go to `origin/main`, that the push starts the workflow and spends Actions minutes, and that it publishes the `main-latest` prerelease.
- [ ] **Step 2: Push** `main` to `origin` (`git push origin main`).
- [ ] **Step 3: Watch the run.** Use `gh run watch` if `gh` is installed and authenticated. Otherwise ask the user for the run page or job logs. Record each job's result.
- [ ] **Step 4: Fix what fails** in follow-up commits on a branch or worktree: reviewed, merged, and pushed only after asking again. Known decision points:
  - **`ubuntu-22.04-arm` is unavailable** to this private repo: remove the arm64 matrix entry, and update the README table and spec §3 with a dated note.
  - **A missing Linux `-dev` package:** add it to `tools/ci/linux-deps.sh` and to `tools/linux/Containerfile`.
  - **Windows packaging can't find `zip`/`7z`:** check the runner image and use `7z`'s full path.
- [ ] **Step 5: Verify the release.** `main-latest` exists as a prerelease with 6 assets: 4 archives, `SHA256SUMS` and `manifest.json`. The manifest's `build` equals the run number, and its `commit` equals the pushed sha. Download one archive and check it against `SHA256SUMS`, using a token, since the repo is private.

---

## Spec coverage

| Spec item | Task |
|---|---|
| §1 success 1 (`main-latest` holds the 4 archives, `SHA256SUMS` and the manifest) | 3, 4 |
| §1 success 2 (binaries per platform: universal, glibc 2.35, Windows release) | 3 (runners, `lipo`/`codesign`, `MACOSX_DEPLOYMENT_TARGET`), 4 |
| §1 success 3 (build number and channel in the About dialog) | 2 |
| §1 success 4 (local builds and goldens unchanged) | 2 (`local`), every task's `./dev check` |
| §2 rolling prerelease, private repo, targets, tests in CI | 3 |
| §3 workflow: triggers, concurrency, permissions, pinned actions, toolchain, caching, jobs, arm64 risk | 3, 4 |
| §4 archives and `package.sh` | 1 |
| §5 `manifest.sh`, publish order, stale deletion, consistency | 1 (scripts and tests), 3 (wiring) |
| §6 build number in the app | 2 |
| §7 verification: `ci-lint`, `ci-test`, push only after asking | 1, 3, 4 |
| §8 out of scope | not implemented |
