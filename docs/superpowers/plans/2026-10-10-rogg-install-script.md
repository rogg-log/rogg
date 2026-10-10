# rogg install.sh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash` interactively installs, upgrades or (with `--uninstall`) removes rogg on macOS and Linux from the latest release.

**Architecture:**
- **`rogg --version`:** a new flag prints the build, so the installer can see what's installed.
- **Release change:** Linux archives gain the icon PNG for a desktop entry.
- **`install.sh`:** a single bash 3.2-compatible script at the repository root. It reads answers from the terminal and checks downloads against `SHA256SUMS`.
- **Tests:** `tools/ci/test-install.sh` drives the script against a fake release served through `file://`.

**Tech Stack:** bash (macOS `/bin/bash` 3.2 and Linux bash), curl, tar, shasum/sha256sum, sed, awk; Rust (rogg-app); Playwright Chromium (icon PNG); shellcheck from the actionlint image.

**Spec:** `docs/superpowers/specs/2026-10-10-rogg-install-script-design.md`

## Global Constraints

- **Commands:**
  - `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash`
  - `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash -s -- --uninstall`
- **Release URL:** `https://github.com/rogg-log/rogg/releases/latest/download/<file>`. Public repo only: plain curl, no tokens.
- **Test seams** (documented as such in the script): `ROGG_INSTALL_BASE` (release folder URL), `ROGG_INSTALL_INPUT` (answers file, default `/dev/tty`), `ROGG_INSTALL_PLATFORM` (`macos-universal` | `linux-x86_64` | `linux-arm64`).
- **`rogg --version` / `-V`:** prints `rogg <version> (build <n>, <channel>, <commit>)` for CI builds and `rogg <version> (local, <commit>)` otherwise, then exits 0 without a window.
- **Defaults:**
  - macOS app: `/Applications`, with `~/Applications` or another folder offered; `sudo` only when the folder isn't writable, after saying why.
  - Binary: `~/.local/bin`, with another folder offered.
- **PATH line:**
  - wording: `export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh`; fish uses `set -gx PATH "…" $PATH  # added by rogg install.sh`;
  - target file: zsh `${ZDOTDIR:-$HOME}/.zshrc`; bash `~/.bash_profile` on macOS, `~/.bashrc` on Linux; fish `~/.config/fish/conf.d/rogg.fish`; any other shell `~/.profile`;
  - added at most once.
- **Desktop entry (Linux):** `${XDG_DATA_HOME:-~/.local/share}/applications/rogg.desktop` with `Exec="<binary>" %F`, `Icon=rogg`, `Categories=Development;Utility;`, `MimeType=text/plain;text/x-log;`. The icon goes to `…/icons/hicolor/256x256/apps/rogg.png`.
- **Uninstall:** looks at `/Applications/rogg.app`, `~/Applications/rogg.app`, `~/.local/bin/rogg` and `rogg` on `PATH`. It never touches settings or data, and prints where they are.
- **Script safety:** every function is defined first, and the last line is `main "$@"`, so a truncated download runs nothing. The script uses `set -euo pipefail` and a temporary folder removed on exit.
- **Shell compatibility:** everything runs under macOS `/bin/bash` 3.2: no `mapfile`, no associative arrays, no `${var,,}`, no `[[ … =~ ]]` with inline spaces (use a regex variable).
- **macOS stays unchanged:** `./dev check` green, goldens at 0.000%.
- **Commits:** plain messages. **No `Co-Authored-By` or any AI attribution trailer.**
- **Pushing:** pushing to `origin` is asked first.

## Review Focus

1. **macOS runs `curl … | bash` with `/bin/bash` 3.2.** Any bash 4 construct breaks the first real use. Expected: the whole test suite runs under `/bin/bash` on macOS. Task 3: `./dev ci-test` runs `test-install.sh` with `/bin/bash`.
2. **A dropped connection truncates the piped script.** Partial code must not run. Expected: everything is defined in functions, and `main "$@"` is the last line. Task 3: `test_main_is_called_last`.
3. **A folder with spaces** (for example "My Apps") must install correctly. Task 3: `test_mac_app_installs_to_a_folder_with_spaces`.
4. **A shell startup file without a final newline** must not have the `PATH` line glued onto its last line. Task 4: `test_path_line_lands_on_its_own_line`.
5. **An existing rogg too old to have `--version`** must show as "unknown version" and still be upgraded, not crash or open a window. Task 3: `test_an_old_binary_without_version_shows_unknown_and_upgrades`.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/rogg-app/src/args.rs` | modify | `--version` / `-V` flag, usage line |
| `crates/rogg-app/src/about.rs` | modify | `BuildInfo.number`/`channel`, `version_line` |
| `crates/rogg-app/src/main.rs` | modify | print the version and exit before anything else |
| `crates/rogg-app/tests/version.rs` | create | the real binary answers `--version` without opening a window |
| `tools/icon/make-icns.mjs` | modify | also render `rogg.png` (256 px) |
| `crates/rogg-app/assets/icon/rogg.png` | create (generated) | the Linux desktop icon |
| `tools/ci/package.sh` | modify | add `rogg.png` to Linux archives |
| `install.sh` | create | the installer |
| `tools/ci/test-install.sh` | create | installer tests against a fake release |
| `tools/ci/test.sh` | modify | Linux archive listing includes `rogg.png` |
| `dev`, `justfile` | modify | `ci-test` runs both test files; `ci-lint` also shellchecks `install.sh` |
| `.github/workflows/build.yml` | modify | the `check` job runs `test-install.sh` too |
| `README.md` | modify | an "Install" section |
| `docs/superpowers/notes/2026-10-10-install-manual-checks.md` | create | end-to-end checks once the repository is public |

---

### Task 1: `rogg --version`

**Files:**
- Modify: `crates/rogg-app/src/args.rs`, `crates/rogg-app/src/about.rs`, `crates/rogg-app/src/main.rs`
- Create: `crates/rogg-app/tests/version.rs`

**Interfaces:**
- Produces:
  - `Args.version: bool`;
  - `about::BuildInfo { number: Option<&'static str>, channel: Option<&'static str>, .. }`;
  - `about::version_line(&BuildInfo) -> String`.
  - The `rogg --version` output format in Global Constraints is what Task 3's installer parses.

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-app/src/args.rs` tests add:

```rust
    #[test]
    fn version_flag_is_recognised() {
        assert!(parse("--version").unwrap().version);
        assert!(parse("-V").unwrap().version);
        assert!(!parse("a.log").unwrap().version);
        assert!(USAGE.contains("--version"));
    }
```

In `crates/rogg-app/src/about.rs` tests add:

```rust
    #[test]
    fn version_line_names_the_build_or_says_local() {
        let v = env!("CARGO_PKG_VERSION");
        assert_eq!(version_line(&describe(Some("cbdea91"), None, None, Some("5"), Some("main"))), format!("rogg {v} (build 5, main, cbdea91)"));
        assert_eq!(version_line(&describe(Some("cbdea91"), None, None, Some("5"), None)), format!("rogg {v} (build 5, cbdea91)"));
        assert_eq!(version_line(&describe(Some("abc1234"), None, None, None, None)), format!("rogg {v} (local, abc1234)"));
        assert_eq!(version_line(&describe(None, None, None, None, None)), format!("rogg {v} (local, unknown)"));
    }
```

Create `crates/rogg-app/tests/version.rs`:

```rust
//! The real binary answers `--version` on stdout and exits without opening a window (install.sh relies on this).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn rogg_version_prints_one_line_and_exits() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rogg"))
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("rogg starts");
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().ok();
            panic!("rogg --version did not exit (did it open a window?)");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with(&format!("rogg {} (", env!("CARGO_PKG_VERSION"))), "{text:?}");
    assert_eq!(text.lines().count(), 1, "{text:?}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test app version`
Expected: compile errors (`version` field, `version_line` and `describe` unknown or of the wrong arity).

- [ ] **Step 3: Implement**

In `args.rs`:
- the module doc becomes `//! Command line: \`rogg [--demo] [--version] [FILE...]\`.`;
- `pub const USAGE: &str = "usage: rogg [--demo] [--version] [FILE...]   (FILE \`-\` reads stdin)";`;
- add the field:

  ```rust
      /// Print the version and build, then exit (install.sh reads it).
      pub version: bool,
  ```
- in `parse`, before the `-h` arm: `Some("-V" | "--version") => out.version = true,`.

In `about.rs`, extend `BuildInfo` and `describe`, and add `version_line`:

```rust
pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub date: &'static str,
    pub rustc: &'static str,
    /// The CI run number and channel, when CI built it.
    pub number: Option<&'static str>,
    pub channel: Option<&'static str>,
    /// `123 (main)` for a CI build, `local` otherwise.
    pub build: String,
}
```

In `describe`, set `number, channel` in the returned struct (they are already parameters).

```rust
/// `rogg --version`: `rogg 0.1.0 (build 5, main, cbdea91)` for a CI build, `rogg 0.1.0 (local, cbdea91)` otherwise.
pub fn version_line(b: &BuildInfo) -> String {
    let mut parts = match b.number {
        Some(n) => vec![format!("build {n}")],
        None => vec![LOCAL.to_string()],
    };
    if let (Some(_), Some(channel)) = (b.number, b.channel) {
        parts.push(channel.to_string());
    }
    parts.push(b.commit.to_string());
    format!("rogg {} ({})", b.version, parts.join(", "))
}
```

In `main.rs`, the `Ok(mut args)` arm starts with:

```rust
            if args.version {
                println!("{}", rogg_app::about::version_line(&rogg_app::about::build_info()));
                return;
            }
```

- [ ] **Step 4: Run the tests**

Run: `./dev test app version`, then `./dev check`.
Expected: PASS; goldens 0.000%.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app/src/args.rs crates/rogg-app/src/about.rs crates/rogg-app/src/main.rs crates/rogg-app/tests/version.rs
git commit -m "feat(app): rogg --version prints the version and build"
```

### Task 2: The icon PNG in Linux archives

**Files:**
- Modify: `tools/icon/make-icns.mjs`, `tools/ci/package.sh`, `tools/ci/test.sh`
- Create (generated): `crates/rogg-app/assets/icon/rogg.png`

**Interfaces:**
- Produces: Linux archives contain `rogg.png` at the root. Task 3 copies it into the desktop icon folder.

- [ ] **Step 1: Write the failing test**

In `tools/ci/test.sh`:
- rename `test_tar_archive_lists_exactly_the_four_files` to `test_tar_archive_lists_exactly_its_files`, in both the function and the run list;
- change its expected listing to:

```bash
  [[ $list == "Inter-OFL.txt JetBrainsMono-OFL.txt README.md rogg rogg.png " ]] || { fail "$name" "listing '$list'"; return; }
```

Add, after it:

```bash
test_mac_and_windows_archives_carry_no_png() {
  local name=${FUNCNAME[0]} dist=$work/dist-nopng out
  out=$(DIST_DIR=$dist "$sh" "$ci/package.sh" macos universal "$(fake_binary rogg-mac-nopng)")
  ! tar -tzf "$out" | grep -q rogg.png || { fail "$name" "the macOS archive has rogg.png"; return; }
  pass "$name"
}
```

Add `test_mac_and_windows_archives_carry_no_png` to the run list.

- [ ] **Step 2: Run it to verify it fails**

Run: `./dev ci-test`
Expected: `FAIL test_tar_archive_lists_exactly_its_files: listing '… rogg '` (no `rogg.png`).

- [ ] **Step 3: Implement**

In `tools/icon/make-icns.mjs`, just before `await browser.close();`, render the 256 px PNG into the icon folder:

```js
await page.setViewportSize({ width: 256, height: 256 });
await page.setContent(`<html><body style="margin:0;background:transparent">${full.replace("<svg ", `<svg width="256" height="256" `)}</body></html>`);
await page.screenshot({ path: join(icons, "rogg.png"), omitBackground: true, clip: { x: 0, y: 0, width: 256, height: 256 } });
```

Then update the header comment: "…packs them into rogg.icns with iconutil, and writes rogg.png (256 px) for the Linux desktop entry." Run `./dev icon`, then check `crates/rogg-app/assets/icon/rogg.png` exists and is a 256×256 PNG (`file` reports `PNG image data, 256 x 256`).

In `tools/ci/package.sh`, after `docs=(…)`:

```bash
# Linux archives carry the app icon for install.sh's desktop entry.
[[ $os == linux ]] && docs+=("$root/crates/rogg-app/assets/icon/rogg.png")
```

- [ ] **Step 4: Run the tests**

Run: `./dev ci-test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add tools/icon/make-icns.mjs tools/ci/package.sh tools/ci/test.sh crates/rogg-app/assets/icon/rogg.png
git commit -m "build(ci): Linux archives carry rogg.png for a desktop entry"
```

### Task 3: `install.sh`: install and upgrade

**Files:**
- Create: `install.sh`, `tools/ci/test-install.sh`
- Modify: `dev` (`ci-test` runs both files; `ci-lint` shellchecks `install.sh`), `.github/workflows/build.yml` (the `check` job runs `test-install.sh`)

**Interfaces:**
- Consumes:
  - `rogg --version` output (Task 1);
  - `rogg.png` in Linux archives (Task 2);
  - `manifest.json` top-level lines `  "version": "0.1.0",` and `  "build": 5,`, as written by `tools/ci/manifest.sh`;
  - `SHA256SUMS` lines `<hex>  <file>`.
- Produces: `install.sh` with functions `main`, `main_install` and `main_uninstall`. The uninstall function is a stub here, filled in by Task 4. Helpers: `ask`, `yes_no`, `platform`, `fetch`, `field`, `installed_version`, `sha256`, `writable`, `expand_path`, `on_path`, `shell_rc`, `path_line`, `add_to_path`, `desktop_entry`.

- [ ] **Step 1: Write the failing tests**

Create `tools/ci/test-install.sh`:

```bash
#!/usr/bin/env bash
# Tests for install.sh against a fake release served through file:// (a temporary HOME, scripted answers).
# Runs under macOS /bin/bash 3.2 and on Linux. Usage: bash tools/ci/test-install.sh
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
sh=${BASH:-bash}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0
pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s: %s\n' "$1" "$2"; failures=$((failures + 1)); }
version=$("$sh" "$root/tools/ci/version.sh")
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

# A fake rogg whose --version reports <build>; with "old" it rejects --version like builds before it existed.
fake_rogg() {
  if [[ $2 == old ]]; then printf '#!/bin/sh\necho "unknown option" >&2\nexit 2\n' > "$1"
  else printf '#!/bin/sh\n[ "$1" = --version ] && echo "rogg %s (build %s, main, abc1234)"\nexit 0\n' "$version" "$2" > "$1"; fi
  chmod 755 "$1"
}

# A release folder for <build>: the three archives install.sh uses, SHA256SUMS and manifest.json.
release() {
  local build=$1 dir=$work/release-$1 s=$work/stage-$1
  [[ -d $dir ]] && { echo "$dir"; return; }
  mkdir -p "$dir" "$s/lin" "$s/bin" "$s/app/rogg.app/Contents/MacOS"
  fake_rogg "$s/lin/rogg" "$build"; cp "$root/crates/rogg-app/assets/icon/rogg.png" "$s/lin/"
  (cd "$s/lin" && tar -czf "$dir/rogg-linux-x86_64.tar.gz" rogg rogg.png)
  fake_rogg "$s/bin/rogg" "$build"
  (cd "$s/bin" && tar -czf "$dir/rogg-macos-universal.tar.gz" rogg)
  fake_rogg "$s/app/rogg.app/Contents/MacOS/rogg" "$build"; printf '<plist/>' > "$s/app/rogg.app/Contents/Info.plist"
  (cd "$s/app" && tar -czf "$dir/rogg-macos-universal-app.tar.gz" rogg.app)
  "$sh" "$root/tools/ci/manifest.sh" "$dir" "$build" abc1234 main
  echo "$dir"
}

# run <home> <platform> <release-dir> <answers, one per line> [-- install.sh args]; output in $work/out, status in $status
run() {
  local home=$1 plat=$2 rel=$3 answers=$4
  shift 4
  [[ ${1:-} == -- ]] && shift
  printf '%s' "$answers" > "$work/answers"
  set +e
  env -i HOME="$home" SHELL="${TEST_SHELL:-/bin/zsh}" PATH=/usr/bin:/bin TERM=dumb \
    ROGG_INSTALL_BASE="file://$rel" ROGG_INSTALL_INPUT="$work/answers" ROGG_INSTALL_PLATFORM="$plat" \
    "$sh" "$root/install.sh" "$@" > "$work/out" 2>&1
  status=$?
  set -e
}
new_home() { local h=$work/home-$1; rm -rf "$h"; mkdir -p "$h"; echo "$h"; }
has() { grep -qF -- "$1" "$work/out"; }

test_main_is_called_last() {
  local name=${FUNCNAME[0]}
  local last
  last=$(grep -v '^[[:space:]]*\(#.*\)\{0,1\}$' "$root/install.sh" 2>/dev/null | tail -1 || true)
  [[ $last == 'main "$@"' ]] || { fail "$name" "the last line is not main \"\$@\" (got '$last')"; return; }
  pass "$name"
}

test_fresh_binary_install_adds_path_once() {
  local name=${FUNCNAME[0]} h rel
  h=$(new_home fresh); rel=$(release 7)
  run "$h" linux-x86_64 "$rel" $'\n\n\nn\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  has "Installed:  not installed" && has "Installing: rogg $version build 7" || { fail "$name" "summary"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "binary not installed"; return; }
  [[ $(grep -c "added by rogg install.sh" "$h/.zshrc") == 1 ]] || { fail "$name" "PATH line not added once"; return; }
  grep -qF 'export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh' "$h/.zshrc" || { fail "$name" "PATH line wording"; return; }
  run "$h" linux-x86_64 "$rel" $'\n\n'
  has "Already up to date" && has "Nothing changed" || { fail "$name" "second run did not say up to date"; return; }
  [[ $(grep -c "added by rogg install.sh" "$h/.zshrc") == 1 ]] || { fail "$name" "PATH line added twice"; return; }
  pass "$name"
}

test_upgrade_shows_both_builds() {
  local name=${FUNCNAME[0]} h
  h=$(new_home upgrade)
  run "$h" linux-x86_64 "$(release 6)" $'\n\nn\nn\n'
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  has "Installed:  rogg $version build 6" && has "Installing: rogg $version build 7" || { fail "$name" "summary"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "not upgraded"; return; }
  pass "$name"
}

test_downgrade_is_refused_by_default() {
  local name=${FUNCNAME[0]} h
  h=$(new_home downgrade)
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  run "$h" linux-x86_64 "$(release 6)" $'\n\n'
  has "newer than the latest release" && has "Nothing changed" || { fail "$name" "no downgrade warning"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "downgraded"; return; }
  pass "$name"
}

test_checksum_mismatch_changes_nothing() {
  local name=${FUNCNAME[0]} h bad
  h=$(new_home checksum)
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  bad=$work/release-bad; rm -rf "$bad"; cp -R "$(release 8)" "$bad"
  sed -i.bak "s/^[0-9a-f]\{64\}  rogg-linux-x86_64.tar.gz/$(printf '0%.0s' $(seq 64))  rogg-linux-x86_64.tar.gz/" "$bad/SHA256SUMS"
  run "$h" linux-x86_64 "$bad" $'\n\n'
  [[ $status != 0 ]] && has "checksum mismatch" || { fail "$name" "exit $status without a mismatch error"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "the old binary was touched"; return; }
  pass "$name"
}

test_mac_app_installs_to_a_folder_with_spaces() {
  local name=${FUNCNAME[0]} h
  h=$(new_home macapp)
  run "$h" macos-universal "$(release 7)" "1"$'\n'"3"$'\n'"$h/My Apps"$'\n\n'
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  [[ $("$h/My Apps/rogg.app/Contents/MacOS/rogg" --version) == *"build 7,"* ]] || { fail "$name" "app not installed"; return; }
  [[ ! -e $h/.zshrc ]] || { fail "$name" "touched PATH for an app install"; return; }
  run "$h" macos-universal "$(release 8)" "1"$'\n'"3"$'\n'"$h/My Apps"$'\n\n'
  has "Installed:  rogg $version build 7" || { fail "$name" "upgrade summary"; return; }
  [[ $("$h/My Apps/rogg.app/Contents/MacOS/rogg" --version) == *"build 8,"* && ! -e "$h/My Apps/rogg.app.old" ]] || { fail "$name" "app not replaced cleanly"; return; }
  pass "$name"
}

test_mac_binary_uses_bash_profile_for_bash() {
  local name=${FUNCNAME[0]} h
  h=$(new_home macbin)
  TEST_SHELL=/bin/bash run "$h" macos-universal "$(release 7)" $'2\n\n\n\n'
  [[ -x $h/.local/bin/rogg ]] || { fail "$name" "binary not installed"; return; }
  grep -q "added by rogg install.sh" "$h/.bash_profile" || { fail "$name" "no line in .bash_profile"; return; }
  pass "$name"
}

test_an_old_binary_without_version_shows_unknown_and_upgrades() {
  local name=${FUNCNAME[0]} h
  h=$(new_home old); mkdir -p "$h/.local/bin"; fake_rogg "$h/.local/bin/rogg" old
  run "$h" linux-x86_64 "$(release 7)" $'\n\nn\nn\n'
  has "unknown version" || { fail "$name" "did not say unknown version"; return; }
  [[ $("$h/.local/bin/rogg" --version) == *"build 7,"* ]] || { fail "$name" "not upgraded"; return; }
  pass "$name"
}

test_unsupported_platform_stops_cleanly() {
  local name=${FUNCNAME[0]} h
  h=$(new_home unsupported)
  run "$h" freebsd-x86_64 "$(release 7)" ""
  [[ $status != 0 ]] && has "no build for" || { fail "$name" "exit $status: $(cat "$work/out")"; return; }
  pass "$name"
}

test_no_terminal_stops_with_a_message() {
  local name=${FUNCNAME[0]} h
  h=$(new_home notty)
  set +e
  env -i HOME="$h" PATH=/usr/bin:/bin ROGG_INSTALL_BASE="file://$(release 7)" ROGG_INSTALL_INPUT=/nonexistent/tty \
    ROGG_INSTALL_PLATFORM=linux-x86_64 "$sh" "$root/install.sh" > "$work/out" 2>&1
  status=$?; set -e
  [[ $status != 0 ]] && has "terminal" || { fail "$name" "exit $status: $(cat "$work/out")"; return; }
  pass "$name"
}

for t in test_main_is_called_last test_fresh_binary_install_adds_path_once test_upgrade_shows_both_builds \
         test_downgrade_is_refused_by_default test_checksum_mismatch_changes_nothing test_mac_app_installs_to_a_folder_with_spaces \
         test_mac_binary_uses_bash_profile_for_bash test_an_old_binary_without_version_shows_unknown_and_upgrades \
         test_unsupported_platform_stops_cleanly test_no_terminal_stops_with_a_message; do
  "$t"
done
(( failures == 0 )) && echo "all install tests passed" || { echo "$failures failed"; exit 1; }
```

Answer order, for reading the test strings:
- **Linux binary:** folder, then install confirmation (or reinstall/downgrade), then add-to-`PATH`, then desktop entry.
- **macOS app:** what (`1`), where (`1`/`2`/`3`, plus the folder for `3`), then confirmation.
- **macOS binary:** what (`2`), folder, then confirmation, then `PATH`.

An empty line takes the default.

In `dev`:
- change `cmd_ci_test` to:

```zsh
cmd_ci_test() { step "CI script tests"; run /bin/bash tools/ci/test.sh; run /bin/bash tools/ci/test-install.sh; }
```
- extend `cmd_ci_lint` with a shellcheck run after actionlint:

```zsh
  run container run --rm --volume "$ROOT:/repo" --workdir /repo --entrypoint shellcheck "$ACTIONLINT_IMAGE" install.sh tools/ci/test-install.sh
```
- update the `ci-test` and `ci-lint` help lines to mention the installer.

In `.github/workflows/build.yml`, after the `CI script tests` step of `check`:

```yaml
      - name: Installer tests
        run: bash tools/ci/test-install.sh
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev ci-test`
Expected: `test-install.sh` reports FAIL for every case (no `install.sh` yet). `test_main_is_called_last` fails reading a missing file; let `grep` fail and the test report FAIL. `test.sh` still passes.

- [ ] **Step 3: Implement `install.sh`**

```bash
#!/usr/bin/env bash
# shellcheck shell=bash
# rogg installer for macOS and Linux (docs/superpowers/specs/2026-10-10-rogg-install-script-design.md):
#   curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash -s -- --uninstall
# It asks before changing anything, checks downloads against SHA256SUMS, and never touches settings or data.
# Answers are read from the terminal (when piped, stdin is this script). Test seams, not user options:
# ROGG_INSTALL_BASE (release folder URL), ROGG_INSTALL_INPUT (answers file), ROGG_INSTALL_PLATFORM.
# Everything is a function and main runs on the last line, so a cut-off download runs nothing.
set -euo pipefail

BASE=${ROGG_INSTALL_BASE:-https://github.com/rogg-log/rogg/releases/latest/download}
MARK="# added by rogg install.sh"

say() { printf '%s\n' "$*"; }
die() { printf 'rogg install: %s\n' "$*" >&2; exit 1; }

# ask <prompt> <default>: prints the answer (the default for an empty line or no more input).
ask() {
  local reply=""
  printf '%s ' "$1" >&2
  IFS= read -r reply <&3 || true
  [[ -t 3 ]] || printf '\n' >&2
  printf '%s' "${reply:-$2}"
}

# yes_no <question> <y|n>: succeeds for yes.
yes_no() {
  local hint="[y/N]" reply
  [[ $2 == y ]] && hint="[Y/n]"
  reply=$(ask "$1 $hint" "$2")
  case $reply in
    [Yy]*) return 0 ;;
    [Nn]*) return 1 ;;
    *) [[ $2 == y ]] ;;
  esac
}

platform() {
  local p=${ROGG_INSTALL_PLATFORM:-}
  if [[ -z $p ]]; then
    case "$(uname -s)/$(uname -m)" in
      Darwin/*) p=macos-universal ;;
      Linux/x86_64 | Linux/amd64) p=linux-x86_64 ;;
      Linux/aarch64 | Linux/arm64) p=linux-arm64 ;;
      *) p="$(uname -s)-$(uname -m)" ;;
    esac
  fi
  case $p in
    macos-universal | linux-x86_64 | linux-arm64) printf '%s' "$p" ;;
    *) die "rogg has no build for $p; builds exist for macOS (Apple silicon and Intel) and Linux (x86_64, arm64)" ;;
  esac
}

fetch() { curl -fsSL --retry 2 -o "$2" "$BASE/$1" || die "could not download $1 from $BASE"; }

# field <name> <manifest>: a top-level manifest.json value (strings unquoted).
field() { sed -n "s/^  \"$1\": \"\{0,1\}\([^\",]*\)\"\{0,1\},\{0,1\}\$/\1/p" "$2" | head -1; }

sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }

# installed_version <binary>: "0.1.0 5", "0.1.0 local", "unknown" (no --version), or nothing when absent.
installed_version() {
  local out ci_re='^rogg ([0-9]+\.[0-9]+\.[0-9]+) \(build ([0-9]+)' local_re='^rogg ([0-9]+\.[0-9]+\.[0-9]+) \(local'
  [[ -x $1 ]] || return 0
  out=$("$1" --version 2>/dev/null </dev/null) || { printf unknown; return 0; }
  if [[ $out =~ $ci_re ]]; then printf '%s %s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
  elif [[ $out =~ $local_re ]]; then printf '%s local' "${BASH_REMATCH[1]}"
  else printf unknown; fi
}

describe() {
  case $1 in
    "") printf 'not installed' ;;
    unknown) printf 'rogg, unknown version' ;;
    *" local") printf 'rogg %s, a local build' "${1% *}" ;;
    *) printf 'rogg %s build %s' "${1% *}" "${1#* }" ;;
  esac
}

# writable <dir>: the folder, or its nearest existing parent, can be written without sudo.
writable() { local d=$1; while [[ ! -d $d ]]; do d=$(dirname "$d"); done; [[ -w $d ]]; }

expand_path() {
  local p=$1
  [[ $p == "~" ]] && p=$HOME
  [[ $p == "~/"* ]] && p=$HOME/${p#"~/"}
  [[ -n $p ]] || die "no folder given"
  printf '%s' "$p"
}

on_path() { case ":$PATH:" in *":$1:"*) return 0 ;; *) return 1 ;; esac; }

shell_rc() {
  case $(basename "${SHELL:-sh}") in
    zsh) printf '%s' "${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) if [[ $PLATFORM == macos-* ]]; then printf '%s' "$HOME/.bash_profile"; else printf '%s' "$HOME/.bashrc"; fi ;;
    fish) printf '%s' "$HOME/.config/fish/conf.d/rogg.fish" ;;
    *) printf '%s' "$HOME/.profile" ;;
  esac
}

path_line() {
  local d=$1
  [[ $d == "$HOME"/* ]] && d="\$HOME/${d#"$HOME"/}"
  if [[ $(basename "${SHELL:-sh}") == fish ]]; then printf 'set -gx PATH "%s" $PATH  %s' "$d" "$MARK"
  else printf 'export PATH="%s:$PATH"  %s' "$d" "$MARK"; fi
}

add_to_path() {
  local dir=$1 rc line
  on_path "$dir" && return 0
  rc=$(shell_rc); line=$(path_line "$dir")
  if [[ -f $rc ]] && grep -qF -- "$line" "$rc"; then
    say "$dir is added to PATH in $rc; open a new terminal to use rogg."
    return 0
  fi
  if yes_no "$dir is not on your PATH. Add it in $rc?" y; then
    mkdir -p "$(dirname "$rc")"
    [[ -s $rc && -n $(tail -c 1 "$rc") ]] && printf '\n' >> "$rc"
    printf '%s\n' "$line" >> "$rc"
    say "Added to $rc. Open a new terminal to use rogg."
  fi
}

desktop_entry() {
  local target=$1 icon=$2 data=${XDG_DATA_HOME:-$HOME/.local/share}
  yes_no "Add rogg to your applications menu?" y || return 0
  mkdir -p "$data/icons/hicolor/256x256/apps" "$data/applications"
  cp "$icon" "$data/icons/hicolor/256x256/apps/rogg.png"
  cat > "$data/applications/rogg.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=rogg
Comment=Log viewer
Exec="$target" %F
Icon=rogg
Terminal=false
Categories=Development;Utility;
MimeType=text/plain;text/x-log;
DESKTOP
  if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$data/applications" >/dev/null 2>&1 || true; fi
  say "Added rogg to the applications menu."
}

main_install() {
  local version build kind=binary dir target bin archive current have want got sudo="" tmp
  # Global, so the EXIT trap still sees it after this function returns.
  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT
  tmp=$TMP_DIR
  fetch manifest.json "$tmp/manifest.json"
  version=$(field version "$tmp/manifest.json"); build=$(field build "$tmp/manifest.json")
  [[ -n $version && $build =~ ^[0-9]+$ ]] || die "the release manifest could not be read"
  say "rogg installer — latest release: rogg $version build $build"

  if [[ $PLATFORM == macos-* ]]; then
    say "What to install?"
    say "  1) rogg.app, opens from Finder (recommended)"
    say "  2) the command-line binary"
    case $(ask "Choose [1]:" 1) in 1) kind=app ;; 2) kind=binary ;; *) die "choose 1 or 2" ;; esac
  fi
  if [[ $kind == app ]]; then
    say "Where?"
    say "  1) /Applications (recommended)"
    say "  2) ~/Applications"
    say "  3) another folder"
    case $(ask "Choose [1]:" 1) in
      1) dir=/Applications ;;
      2) dir=$HOME/Applications ;;
      3) dir=$(expand_path "$(ask "Folder:" "")") ;;
      *) die "choose 1, 2 or 3" ;;
    esac
    target=$dir/rogg.app; bin=$target/Contents/MacOS/rogg; archive=rogg-macos-universal-app.tar.gz
  else
    dir=$(expand_path "$(ask "Install the binary to [~/.local/bin]:" "$HOME/.local/bin")")
    target=$dir/rogg; bin=$target; archive=rogg-$PLATFORM.tar.gz
  fi

  current=$(installed_version "$bin")
  say "Installed:  $(describe "$current") ($target)"
  say "Installing: rogg $version build $build"
  local re='^[0-9.]+ ([0-9]+)$'
  if [[ $current =~ $re ]]; then
    have=${BASH_REMATCH[1]}
    if (( have == build )); then
      yes_no "Already up to date. Reinstall?" n || { say "Nothing changed."; return 0; }
    elif (( have > build )); then
      yes_no "The installed build is newer than the latest release. Downgrade?" n || { say "Nothing changed."; return 0; }
    else
      yes_no "Install?" y || { say "Nothing changed."; return 0; }
    fi
  else
    yes_no "Install?" y || { say "Nothing changed."; return 0; }
  fi

  fetch "$archive" "$tmp/$archive"
  fetch SHA256SUMS "$tmp/SHA256SUMS"
  want=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
  [[ -n $want ]] || die "$archive is not listed in SHA256SUMS; nothing was changed"
  got=$(sha256 "$tmp/$archive")
  [[ $got == "$want" ]] || die "checksum mismatch for $archive; nothing was changed"
  mkdir -p "$tmp/x"
  tar -xzf "$tmp/$archive" -C "$tmp/x"

  if ! writable "$dir"; then
    command -v sudo >/dev/null 2>&1 || die "$dir is not writable and sudo is not available"
    say "$dir needs administrator rights; sudo will ask for your password."
    sudo=sudo
  fi
  $sudo mkdir -p "$dir"
  if [[ $kind == app ]]; then
    $sudo rm -rf "$target.old"
    [[ -e $target ]] && $sudo mv "$target" "$target.old"
    if ! $sudo mv "$tmp/x/rogg.app" "$target"; then
      [[ -e $target.old ]] && $sudo mv "$target.old" "$target"
      die "could not install $target; the previous version is back in place"
    fi
    $sudo rm -rf "$target.old"
    say "Installed rogg $version build $build to $target. Open it from Finder, or: open \"$target\""
  else
    $sudo cp "$tmp/x/rogg" "$dir/.rogg.new.$$"
    $sudo chmod 755 "$dir/.rogg.new.$$"
    $sudo mv -f "$dir/.rogg.new.$$" "$target"
    say "Installed rogg $version build $build to $target."
    add_to_path "$dir"
    if [[ $PLATFORM == linux-* ]]; then desktop_entry "$target" "$tmp/x/rogg.png"; fi
  fi
}

main_uninstall() { die "--uninstall is not implemented yet"; }

main() {
  local input=${ROGG_INSTALL_INPUT:-/dev/tty}
  (: <"$input") 2>/dev/null || die "no terminal to ask questions on; run the installer from a terminal"
  exec 3<"$input"
  PLATFORM=$(platform)
  case ${1:-} in
    "") main_install ;;
    --uninstall) main_uninstall ;;
    -h | --help) say "usage: install.sh [--uninstall]" ;;
    *) die "unknown option $1 (usage: install.sh [--uninstall])" ;;
  esac
}

main "$@"
```

Then `chmod +x install.sh`.

- [ ] **Step 4: Run the tests**

Run: `./dev ci-test`, then `./dev ci-lint` (shellcheck must be clean; fix what it reports, or add a narrowly scoped `# shellcheck disable=SCxxxx` with a reason), then `./dev check`.
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add install.sh tools/ci/test-install.sh dev .github/workflows/build.yml
git commit -m "feat: install.sh installs and upgrades rogg on macOS and Linux"
```

### Task 4: `--uninstall`, README and manual checks

**Files:**
- Modify: `install.sh` (`main_uninstall`), `tools/ci/test-install.sh`, `README.md`
- Create: `docs/superpowers/notes/2026-10-10-install-manual-checks.md`

**Interfaces:**
- Consumes: Task 3's `install.sh` helpers (`installed_version`, `describe`, `writable`, `yes_no`, `say`, `MARK`, `PLATFORM`).

- [ ] **Step 1: Write the failing tests**

In `tools/ci/test-install.sh` add before the run list, and add both names to the list:

```bash
test_linux_desktop_entry_and_uninstall() {
  local name=${FUNCNAME[0]} h data
  h=$(new_home uninstall); data=$h/.local/share
  run "$h" linux-x86_64 "$(release 7)" $'\n\n\n\n'
  grep -qx "Exec=\"$h/.local/bin/rogg\" %F" "$data/applications/rogg.desktop" || { fail "$name" "desktop Exec"; return; }
  [[ -s $data/icons/hicolor/256x256/apps/rogg.png ]] || { fail "$name" "icon not installed"; return; }
  run "$h" linux-x86_64 "$(release 7)" $'\n\n' -- --uninstall
  [[ $status == 0 ]] || { fail "$name" "exit $status: $(tail -3 "$work/out")"; return; }
  has "rogg $version build 7" || { fail "$name" "did not list the install with its version"; return; }
  [[ ! -e $h/.local/bin/rogg && ! -e $data/applications/rogg.desktop && ! -e $data/icons/hicolor/256x256/apps/rogg.png ]] || { fail "$name" "left files behind"; return; }
  ! grep -q "added by rogg install.sh" "$h/.zshrc" || { fail "$name" "PATH line kept"; return; }
  has "settings" || { fail "$name" "did not mention settings and data"; return; }
  pass "$name"
}

test_path_line_lands_on_its_own_line() {
  local name=${FUNCNAME[0]} h
  h=$(new_home rcnewline); printf 'alias ll="ls -l"' > "$h/.zshrc"
  run "$h" linux-x86_64 "$(release 7)" $'\n\n\nn\n'
  [[ $(head -1 "$h/.zshrc") == 'alias ll="ls -l"' ]] || { fail "$name" "the last line was changed: $(head -1 "$h/.zshrc")"; return; }
  grep -qx 'export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh' "$h/.zshrc" || { fail "$name" "PATH line not on its own line"; return; }
  pass "$name"
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev ci-test`
Expected:
- `test_linux_desktop_entry_and_uninstall` fails ("--uninstall is not implemented yet");
- `test_path_line_lands_on_its_own_line` passes already, because Task 3's `add_to_path` handles a missing final newline. That's correct, and it stays as a guard. Write in the report that it passed before this task.

- [ ] **Step 3: Implement `main_uninstall`**

Replace the stub:

```bash
main_uninstall() {
  local found=() c p bin rc data=${XDG_DATA_HOME:-$HOME/.local/share} desktop icon sudo removed=0
  for c in /Applications/rogg.app "$HOME/Applications/rogg.app" "$HOME/.local/bin/rogg"; do [[ -e $c ]] && found+=("$c"); done
  p=$(command -v rogg 2>/dev/null || true)
  if [[ -n $p && $p != */rogg.app/* ]]; then
    for c in "${found[@]+"${found[@]}"}"; do [[ $c == "$p" ]] && p=""; done
    [[ -n $p ]] && found+=("$p")
  fi
  desktop=$data/applications/rogg.desktop; icon=$data/icons/hicolor/256x256/apps/rogg.png

  if (( ${#found[@]} == 0 )) && [[ ! -e $desktop ]]; then
    say "No rogg installation found in the usual places (/Applications, ~/Applications, ~/.local/bin, PATH)."
  else
    say "Found:"
    for c in "${found[@]+"${found[@]}"}"; do
      bin=$c; [[ $c == *.app ]] && bin=$c/Contents/MacOS/rogg
      say "  $c — $(describe "$(installed_version "$bin")")"
    done
    [[ -e $desktop ]] && say "  $desktop (applications menu entry)"
    if yes_no "Remove these?" y; then
      for c in "${found[@]+"${found[@]}"}"; do
        sudo=""
        writable "$(dirname "$c")" || { say "$(dirname "$c") needs administrator rights; sudo will ask for your password."; sudo=sudo; }
        $sudo rm -rf "$c"
      done
      rm -f "$desktop" "$icon"
      if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$data/applications" >/dev/null 2>&1 || true; fi
      removed=1
      say "Removed."
    fi
  fi

  for rc in "${ZDOTDIR:-$HOME}/.zshrc" "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile" "$HOME/.config/fish/conf.d/rogg.fish"; do
    [[ -f $rc ]] && grep -qF "$MARK" "$rc" || continue
    if yes_no "Remove the PATH line the installer added to $rc?" y; then
      if [[ $rc == */rogg.fish ]]; then rm -f "$rc"
      else grep -vF "$MARK" "$rc" > "$rc.rogg-tmp" || true; cat "$rc.rogg-tmp" > "$rc"; rm -f "$rc.rogg-tmp"; fi
    fi
  done

  if [[ $PLATFORM == macos-* ]]; then
    say "Settings and data are kept in ~/Library/Application Support/rogg and ~/Library/Caches/rogg; delete them yourself if you want."
  else
    say "Settings and data are kept in ${XDG_CONFIG_HOME:-~/.config}/rogg, ${XDG_DATA_HOME:-~/.local/share}/rogg and ${XDG_CACHE_HOME:-~/.cache}/rogg; delete them yourself if you want."
  fi
  (( removed )) || true
}
```

`"${found[@]+"${found[@]}"}"` is the bash 3.2-safe way to expand an array that may be empty under `set -u`.

- [ ] **Step 4: README and manual checks**

In `README.md`, insert after the opening paragraph (before the first `##` section):

```markdown
## Install

On macOS and Linux:

    curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash

The installer asks what to install (on macOS: `rogg.app` or the command-line binary) and where (`/Applications` for
the app, `~/.local/bin` for the binary). It shows the installed and the new version before changing anything, checks
the download against the release's `SHA256SUMS`, offers to add the binary's folder to your `PATH`, and on Linux offers
an applications-menu entry. Run it again to upgrade. To remove rogg (settings and data are kept):

    curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash -s -- --uninstall

Or download an archive for your platform from the [latest release](https://github.com/rogg-log/rogg/releases/latest).
`rogg --version` shows the installed version and build. While the repository is private, the installer and the
downloads need access to it.
```

Create `docs/superpowers/notes/2026-10-10-install-manual-checks.md`:

```markdown
# install.sh — manual checks (once the repository is public)

- macOS, fresh: `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash`.
  Choose rogg.app → /Applications: sudo asks for the password, rogg.app opens from Finder with its icon, no
  Gatekeeper prompt.
- macOS, binary: choose 2 → ~/.local/bin; the PATH offer edits ~/.zshrc; a new terminal runs `rogg --version`.
- macOS, upgrade: after a newer build is published, run again; it shows "Installed: … build N" and "Installing:
  … build N+1".
- Linux (container or a desktop): binary install, the PATH offer, the applications-menu entry with the icon, and
  rogg starting from the menu.
- `--uninstall` on both: lists what it found with versions, removes it, offers to remove the PATH line, and leaves
  settings and data.
- A run with no terminal (`echo | bash install.sh` inside `ssh -T`): stops with the terminal message.
```

- [ ] **Step 5: Run the tests**

Run: `./dev ci-test`, `./dev ci-lint`, `./dev check`.
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add install.sh tools/ci/test-install.sh README.md docs/superpowers/notes/2026-10-10-install-manual-checks.md
git commit -m "feat: install.sh --uninstall, README install section and manual checks"
```

---

## Spec coverage

| Spec item | Task |
|---|---|
| §1 success 1 (macOS app/binary and where; Linux binary) | 3 |
| §1 success 2 (installed vs installing, confirm) | 3 |
| §1 success 3 (`SHA256SUMS`; a mismatch changes nothing) | 3 |
| §1 success 4 (PATH offer) | 3, 4 (own-line guard) |
| §1 success 5 (Linux desktop entry) | 3 (written), 4 (tested with uninstall) |
| §1 success 6 (`--uninstall` keeps settings and data) | 4 |
| §2 public repo only, defaults, scope | Global Constraints, 3, 4 |
| §3 `rogg --version` | 1 |
| §4 script steps, `/dev/tty`, colours, robustness | 3 (colour: plain output only, see note) |
| §5 `rogg.png` in Linux archives; script served from the repository | 2; README URL in 4 |
| §6 README | 4 |
| §7 testing, shellcheck, end-to-end note | 3, 4 |
| §8 out of scope | not implemented |

Note: spec §4 allows colour when stdout is a terminal. The plan keeps output plain, because nothing needs colour and it keeps test output stable.
