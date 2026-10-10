# rogg install.ps1 and Windows Resources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- `rogg.exe` carries the app icon, plus version info that reads exactly like `rogg --version`.
- `irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex` installs, upgrades or uninstalls rogg on Windows.

**Architecture:**
- **Resources:** `build.rs` (Windows targets only) renders a resource script from a small module, `crates/rogg-app/build/winres.rs`, and compiles it with `embed-resource`. The library includes the same module in its tests, so the product version can't drift from `about::version_line`.
- **Icon:** `./dev icon` also writes `rogg.ico`.
- **Installer:** `install.ps1` is one PowerShell 5.1-compatible script. Everything runs inside a script block, so nothing leaks into the user's session and a truncated download doesn't parse.

**Tech Stack:** Rust build script with `embed-resource` 3.0.12 (`rc.exe` on Windows, `llvm-rc` for the local cross-build); Node with Playwright (icon); Windows PowerShell 5.1 / PowerShell 7.

**Spec:** `docs/superpowers/specs/2026-10-10-rogg-install-script-design.md` §9

## Global Constraints

- **Resources (Windows targets only):** `1 ICON` from `crates/rogg-app/assets/icon/rogg.ico`, plus `1 VERSIONINFO` with:
  - `FILEVERSION` / `PRODUCTVERSION` `<major>,<minor>,<patch>,<build or 0>`;
  - `ProductName` `rogg`, `FileDescription` `rogg log viewer`, `CompanyName` `rogg-log`, `OriginalFilename` `rogg.exe`;
  - `FileVersion` `<version>.<build or 0>`;
  - `ProductVersion` equal to the `rogg --version` text.
- **The icon file:** `rogg.ico` holds PNG images at 16, 24, 32, 48, 64 and 256 px; 16 and 24 use `rogg-small.svg`.
- **Commands:**
  - `irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex`
  - `& ([scriptblock]::Create((irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1))) -Uninstall`
- **Install folder:** `%LOCALAPPDATA%\Programs\rogg` by default (no admin rights). Relative folders and `"` or `$` in the name are refused.
- **Start-menu shortcut:** `[Environment]::GetFolderPath('Programs')\rogg.lnk`, with its icon from `rogg.exe`.
- **Installer must not leave anything behind in the user's PowerShell session:** never call `exit` (it would close the window under `iex`), and never set `$ErrorActionPreference` or `StrictMode` at the top level.
- **No automated installer tests** (the user's decision). `./dev check`, `./dev ci-test` and the Windows CI build must stay green.
- **Commits:** plain messages, with **no `Co-Authored-By` or other AI attribution trailer**. Ask before pushing.

## Review Focus

These are checked by hand on Windows (the manual-checks list), except item 5, which has a Rust test.

1. **Running under `irm | iex`, the script runs in the user's own session.** It must not close the window or change their session settings. Manual check: run it, then confirm the window stays open, `$ErrorActionPreference` is unchanged, and no stray functions are left defined.
2. **A running `rogg.exe` can't be overwritten.** An upgrade while rogg is open must still succeed: rename it first, and remove the old file on the next run. Manual check.
3. **The user `PATH` may contain `%VAR%` entries.** Adding rogg must not expand or break them: read the raw registry value and write it back as an expandable string. Manual check.
4. **Windows PowerShell 5.1 defaults:** TLS 1.2 must be turned on, and `-UseBasicParsing` used. Otherwise downloads fail on a fresh Windows 10. Manual check on 5.1.
5. **`ProductVersion` and `rogg --version` must not drift,** since the installer parses both the same way. Task 1: `the_exe_product_version_is_the_version_line`.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `tools/icon/make-icns.mjs` | modify | also write `rogg.ico` |
| `crates/rogg-app/assets/icon/rogg.ico` | create (generated) | `rogg.exe`'s icon |
| `crates/rogg-app/build/winres.rs` | create | the resource script text |
| `crates/rogg-app/build.rs` | modify | compile the resources for Windows targets |
| `crates/rogg-app/Cargo.toml` | modify | `embed-resource` build dependency |
| `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/winres_tests.rs` | modify / create | tests for `winres.rs` |
| `tools/ci/test.sh` | modify | check that `rogg.ico` is a valid six-size icon |
| `install.ps1` | create | the Windows installer |
| `README.md` | modify | Windows commands in "Install" |
| `docs/superpowers/notes/2026-10-10-install-manual-checks.md` | modify | a Windows section |

---

### Task 1: The icon and version resources in `rogg.exe`

**Files:**
- Modify: `tools/icon/make-icns.mjs`, `crates/rogg-app/build.rs`, `crates/rogg-app/Cargo.toml`, `crates/rogg-app/src/lib.rs`, `tools/ci/test.sh`
- Create: `crates/rogg-app/build/winres.rs`, `crates/rogg-app/src/winres_tests.rs`, `crates/rogg-app/assets/icon/rogg.ico` (generated)

**Interfaces:**
- Produces: a Windows `rogg.exe` whose `VersionInfo.ProductVersion` is `rogg <version> (build <n>, <channel>, <commit>)` or `rogg <version> (local, <commit>)`. `install.ps1` (Task 2) parses it with `^rogg (\d+\.\d+\.\d+) \(build (\d+)` and `^rogg (\d+\.\d+\.\d+) \(local`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/src/winres_tests.rs`:

```rust
//! rogg.exe's resource script (build/winres.rs, used by build.rs): the version text must read exactly like
//! `rogg --version`, because install.ps1 parses both the same way.

use crate::about;
use crate::winres::{product_version, resource_script};

#[test]
fn the_exe_product_version_is_the_version_line() {
    let cases: [(Option<&'static str>, Option<&'static str>, Option<&'static str>); 4] =
        [(Some("3b4060e"), Some("6"), Some("main")), (Some("3b4060e"), Some("6"), None), (Some("abc1234"), None, None), (None, None, None)];
    for (commit, number, channel) in cases {
        let line = about::version_line(&about::describe(commit, None, None, number, channel));
        assert_eq!(product_version(env!("CARGO_PKG_VERSION"), commit, number, channel), line);
    }
}

#[test]
fn the_resource_script_carries_the_icon_and_versions() {
    let rc = resource_script(r"C:\src\rogg\assets\icon\rogg.ico", "0.1.0", Some("3b4060e"), Some("6"), Some("main"));
    assert!(rc.contains(r#"1 ICON "C:\\src\\rogg\\assets\\icon\\rogg.ico""#), "{rc}");
    assert!(rc.contains("FILEVERSION 0,1,0,6") && rc.contains("PRODUCTVERSION 0,1,0,6"), "{rc}");
    assert!(rc.contains(r#"VALUE "FileVersion", "0.1.0.6""#), "{rc}");
    assert!(rc.contains(r#"VALUE "ProductVersion", "rogg 0.1.0 (build 6, main, 3b4060e)""#), "{rc}");
    assert!(rc.contains(r#"VALUE "ProductName", "rogg""#) && rc.contains(r#"VALUE "OriginalFilename", "rogg.exe""#), "{rc}");
    let local = resource_script("/x/rogg.ico", "0.1.0", Some("abc1234"), None, None);
    assert!(local.contains("FILEVERSION 0,1,0,0") && local.contains("(local, abc1234)"), "{local}");
}
```

In `crates/rogg-app/src/lib.rs` add (near the other `mod` lines):

```rust
#[cfg(test)]
#[path = "../build/winres.rs"]
mod winres;
#[cfg(test)]
mod winres_tests;
```

In `tools/ci/test.sh` add (and register it in the run list):

```bash
test_ico_holds_six_png_sizes() {
  local name=${FUNCNAME[0]}
  python3 - "$root/crates/rogg-app/assets/icon/rogg.ico" <<'PY' || { fail "$name" "rogg.ico is not a six-size PNG icon"; return; }
import struct, sys
data = open(sys.argv[1], "rb").read()
reserved, kind, count = struct.unpack_from("<HHH", data, 0)
assert (reserved, kind, count) == (0, 1, 6), (reserved, kind, count)
sizes = []
for i in range(count):
    w, h, _, _, planes, bpp, size, offset = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
    sizes.append(w or 256)
    assert w == h and planes == 1 and bpp == 32, (w, h, planes, bpp)
    assert data[offset:offset + 8] == b"\x89PNG\r\n\x1a\n", i
assert sizes == [16, 24, 32, 48, 64, 256], sizes
PY
  pass "$name"
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test app winres`, then `./dev ci-test`.
Expected:
- the Rust tests fail to compile, because `build/winres.rs` doesn't exist;
- `test_ico_holds_six_png_sizes` fails, because there is no `rogg.ico` yet.

- [ ] **Step 3: Implement `build/winres.rs`**

```rust
//! rogg.exe's Windows resource script: its icon and version information (install spec §9). build.rs writes it for
//! Windows targets; the library includes this file in its tests (src/winres_tests.rs).

/// The `rogg --version` text. It must match `about::version_line` (a build script can't call the crate, so it is
/// repeated here, and a test compares the two).
pub fn product_version(version: &str, commit: Option<&str>, number: Option<&str>, channel: Option<&str>) -> String {
    let mut parts = match number {
        Some(n) => vec![format!("build {n}")],
        None => vec!["local".to_string()],
    };
    if let (Some(_), Some(c)) = (number, channel) {
        parts.push(c.to_string());
    }
    parts.push(commit.unwrap_or("unknown").to_string());
    format!("rogg {version} ({})", parts.join(", "))
}

/// The `.rc` text: `1 ICON` from `icon`, and a `VERSIONINFO` block whose fourth number is the CI build (0 locally).
pub fn resource_script(icon: &str, version: &str, commit: Option<&str>, number: Option<&str>, channel: Option<&str>) -> String {
    let mut nums: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    nums.resize(3, 0);
    let build: u32 = number.and_then(|n| n.parse().ok()).unwrap_or(0);
    let numeric = format!("{},{},{},{build}", nums[0], nums[1], nums[2]);
    let file_version = format!("{}.{}.{}.{build}", nums[0], nums[1], nums[2]);
    let product = product_version(version, commit, number, channel);
    let icon = icon.replace('\\', "\\\\");
    format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "rogg-log"
      VALUE "FileDescription", "rogg log viewer"
      VALUE "FileVersion", "{file_version}"
      VALUE "OriginalFilename", "rogg.exe"
      VALUE "ProductName", "rogg"
      VALUE "ProductVersion", "{product}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}
```

- [ ] **Step 4: Wire it into `build.rs`**

In `crates/rogg-app/Cargo.toml`:

```toml
[build-dependencies]
embed-resource = "3.0.12"
```

(If a `[build-dependencies]` table already exists, add the line to it.)

In `crates/rogg-app/build.rs`:
- add `#[path = "build/winres.rs"] mod winres;` after the `use` lines;
- add `use std::path::PathBuf;` if it isn't there;
- in `main`, keep the commit, number and channel values in variables (the `values` array already computes them; bind them first and reuse them);
- after the existing `cargo:rustc-env` loop, add:

```rust
    // Windows: rogg.exe's icon and version information (install spec §9). Its release build has no console, so
    // install.ps1 reads the version from these properties instead of `rogg --version`.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
        let icon = manifest_dir.join("assets/icon/rogg.ico");
        println!("cargo:rerun-if-changed={}", icon.display());
        println!("cargo:rerun-if-changed=build/winres.rs");
        let rc = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("rogg.rc");
        let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
        let script = winres::resource_script(&icon.display().to_string(), &version, commit.as_deref(), number.as_deref(), channel.as_deref());
        std::fs::write(&rc, script).expect("write rogg.rc");
        embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().expect("compile rogg.rc");
    }
```

Here `commit`, `number` and `channel` are `Option<String>`: the commit from `git rev-parse --short HEAD`, and the other two from `ROGG_BUILD_NUMBER` and `ROGG_BUILD_CHANNEL`. These are the same values the existing code passes to `cargo:rustc-env`. If `embed-resource` 3.0.12's API differs (check `~/.cargo/registry/src/*/embed-resource-3.0.12/src/lib.rs`), use its equivalent for "compile this .rc; a manifest is not required" and note it.

- [ ] **Step 5: Implement `rogg.ico` in `tools/icon/make-icns.mjs`**

Change the `fs` import to `import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";`. Just before `await browser.close();` add:

```js
// rogg.ico for Windows (rogg.exe's icon): PNG images in one ICO container; 16 and 24 px use the simplified artwork.
const icoSizes = [16, 24, 32, 48, 64, 256];
const pngs = [];
for (const size of icoSizes) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(`<html><body style="margin:0;background:transparent">${(size <= 24 ? small : full).replace("<svg ", `<svg width="${size}" height="${size}" `)}</body></html>`);
  pngs.push(await page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } }));
}
const ico = Buffer.alloc(6 + 16 * pngs.length);
ico.writeUInt16LE(0, 0);
ico.writeUInt16LE(1, 2);
ico.writeUInt16LE(pngs.length, 4);
let offset = ico.length;
pngs.forEach((png, i) => {
  const at = 6 + 16 * i;
  const side = icoSizes[i] >= 256 ? 0 : icoSizes[i];
  ico.writeUInt8(side, at);
  ico.writeUInt8(side, at + 1);
  ico.writeUInt16LE(1, at + 4);
  ico.writeUInt16LE(32, at + 6);
  ico.writeUInt32LE(png.length, at + 8);
  ico.writeUInt32LE(offset, at + 12);
  offset += png.length;
});
writeFileSync(join(icons, "rogg.ico"), Buffer.concat([ico, ...pngs]));
```

Update the header comment to mention `rogg.ico`. Run `./dev icon`. Check that `rogg.icns` and `rogg.png` are unchanged (`git status`), and that `rogg.ico` is new.

- [ ] **Step 6: Run the tests and the Windows build**

Run:
- `./dev test app winres` (PASS);
- `./dev ci-test` (PASS);
- `./dev check` (green; macOS doesn't compile the resources);
- `./dev windows-build`, which cross-builds and compiles `rogg.rc` with `llvm-rc`;
- `/opt/homebrew/opt/llvm/bin/llvm-readobj --coff-resources target/x86_64-pc-windows-msvc/debug/rogg.exe | grep -E "Type: (ICON|GROUP_ICON|VERSION|MANIFEST)"`.

Expected: `ICON`, `GROUP_ICON`, `VERSION` and gpui's `MANIFEST` types are listed. If the cross-build can't find a resource compiler, check how gpui's own `embed_resource` call manages, since it already works under `./dev windows-build`, and match it. Report the fix.

- [ ] **Step 7: Commit**

```bash
git add tools/icon/make-icns.mjs crates/rogg-app/assets/icon/rogg.ico crates/rogg-app/build/winres.rs crates/rogg-app/build.rs crates/rogg-app/Cargo.toml Cargo.lock crates/rogg-app/src/lib.rs crates/rogg-app/src/winres_tests.rs tools/ci/test.sh
git commit -m "feat(windows): rogg.exe carries the app icon and version information"
```

### Task 2: `install.ps1`, README and manual checks

**Files:**
- Create: `install.ps1`
- Modify: `README.md`, `docs/superpowers/notes/2026-10-10-install-manual-checks.md`

**Interfaces:**
- Consumes:
  - `rogg.exe` `VersionInfo.ProductVersion` (Task 1);
  - the release's `manifest.json` (fields `version` and `build`), `SHA256SUMS` (`<hex>  <file>` lines) and `rogg-windows-x86_64.zip` (`rogg.exe`, `README.md`, `Inter-OFL.txt`, `JetBrainsMono-OFL.txt`).

No automated tests, by decision.

- [ ] **Step 1: Write `install.ps1`**

```powershell
# rogg installer for Windows (docs/superpowers/specs/2026-10-10-rogg-install-script-design.md §9):
#   irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1))) -Uninstall
# It asks before changing anything, checks downloads against SHA256SUMS, and never touches settings or data.
# Everything runs inside the script block below: nothing leaks into your PowerShell session, and a cut-off download
# doesn't parse, so it runs nothing. $env:ROGG_INSTALL_BASE overrides the release folder URL.
param([switch]$Uninstall)

& {
  $ErrorActionPreference = 'Stop'
  $Base = if ($env:ROGG_INSTALL_BASE) { $env:ROGG_INSTALL_BASE.TrimEnd('/') } else { 'https://github.com/rogg-log/rogg/releases/latest/download' }
  $Archive = 'rogg-windows-x86_64.zip'
  $DefaultDir = Join-Path $env:LOCALAPPDATA 'Programs\rogg'
  $Shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'rogg.lnk'
  $Files = @('rogg.exe', 'README.md', 'Inter-OFL.txt', 'JetBrainsMono-OFL.txt')

  function Say([string]$Text) { Write-Host $Text }
  function Fail([string]$Text) { throw "rogg install: $Text" }

  function Ask([string]$Prompt, [string]$Default) {
    $reply = Read-Host $Prompt
    if ([string]::IsNullOrWhiteSpace($reply)) { return $Default }
    return $reply.Trim()
  }

  function YesNo([string]$Question, [bool]$Default) {
    $hint = if ($Default) { '[Y/n]' } else { '[y/N]' }
    $reply = Read-Host "$Question $hint"
    if ([string]::IsNullOrWhiteSpace($reply)) { return $Default }
    return $reply.Trim().ToLower().StartsWith('y')
  }

  # "0.1.0 6", "0.1.0 local", "unknown", or $null when there is no rogg.exe.
  function Get-InstalledVersion([string]$Exe) {
    if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { return $null }
    $text = (Get-Item -LiteralPath $Exe).VersionInfo.ProductVersion
    if ($text -match '^rogg (\d+\.\d+\.\d+) \(build (\d+)') { return "$($Matches[1]) $($Matches[2])" }
    if ($text -match '^rogg (\d+\.\d+\.\d+) \(local') { return "$($Matches[1]) local" }
    return 'unknown'
  }

  function Describe($Version) {
    if ($null -eq $Version) { return 'not installed' }
    if ($Version -eq 'unknown') { return 'rogg, unknown version' }
    $parts = $Version.Split(' ')
    if ($parts[1] -eq 'local') { return "rogg $($parts[0]), a local build" }
    return "rogg $($parts[0]) build $($parts[1])"
  }

  function Resolve-Folder([string]$Answer) {
    $p = [Environment]::ExpandEnvironmentVariables($Answer)
    if ($p.StartsWith('~')) { $p = Join-Path $env:USERPROFILE $p.Substring(1).TrimStart('\', '/') }
    if ($p -notmatch '^([A-Za-z]:\\|\\\\)') { Fail "give an absolute folder such as C:\Tools\rogg (not `"$Answer`")" }
    if ($p -match '["$]') { Fail 'the folder name cannot contain " or $' }
    return $p.TrimEnd('\')
  }

  # The user PATH as stored (keeping %VAR% entries unexpanded), and writing it back as an expandable string.
  function Get-UserPath { return [string](Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames') }
  function Set-UserPath([string]$Value) {
    Set-ItemProperty -Path 'HKCU:\Environment' -Name Path -Value $Value -Type ExpandString
    # Setting and clearing a variable through .NET broadcasts the change, so new terminals see the new PATH.
    [Environment]::SetEnvironmentVariable('ROGG_INSTALL_REFRESH', '1', 'User')
    [Environment]::SetEnvironmentVariable('ROGG_INSTALL_REFRESH', $null, 'User')
  }
  function Test-OnPath([string]$Dir, [string]$PathValue) {
    $want = $Dir.TrimEnd('\')
    foreach ($entry in $PathValue.Split(';')) {
      if ([Environment]::ExpandEnvironmentVariables($entry).TrimEnd('\') -eq $want) { return $true }
    }
    return $false
  }

  function Add-ToPath([string]$Dir) {
    $user = Get-UserPath
    if (Test-OnPath $Dir $user) { return }
    if (YesNo "$Dir is not on your PATH. Add it?" $true) {
      $new = if ($user.Trim(';')) { "$($user.TrimEnd(';'));$Dir" } else { $Dir }
      Set-UserPath $new
      Say 'Added to your PATH. Open a new terminal to use rogg.'
    }
  }

  function Add-Shortcut([string]$Exe) {
    $shell = New-Object -ComObject WScript.Shell
    if ((Test-Path -LiteralPath $Shortcut) -and ($shell.CreateShortcut($Shortcut).TargetPath -eq $Exe)) { return }
    if (-not (YesNo 'Add rogg to the Start menu?' $true)) { return }
    $link = $shell.CreateShortcut($Shortcut)
    $link.TargetPath = $Exe
    $link.WorkingDirectory = Split-Path -Parent $Exe
    $link.IconLocation = "$Exe,0"
    $link.Description = 'rogg log viewer'
    $link.Save()
    Say 'Added rogg to the Start menu.'
  }

  function Show-OtherCopy([string]$Exe) {
    $cmd = Get-Command rogg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd -and ($cmd.Path -ne $Exe)) {
      Say "Note: typing rogg runs $($cmd.Path) ($(Describe (Get-InstalledVersion $cmd.Path))), which comes first on PATH."
    }
  }

  function Remove-OldExes([string]$Dir) {
    Get-ChildItem -LiteralPath $Dir -Filter 'rogg.exe*.old' -ErrorAction SilentlyContinue |
      Remove-Item -Force -ErrorAction SilentlyContinue
  }

  function Install-Rogg {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    if (-not [Environment]::Is64BitOperatingSystem) { Fail 'rogg needs 64-bit Windows' }
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'ARM64') { Say 'Windows on ARM: installing the x64 build, which runs under emulation.' }

    $manifest = Invoke-RestMethod -UseBasicParsing "$Base/manifest.json"
    $version = [string]$manifest.version
    $build = [int]$manifest.build
    Say "rogg installer - latest release: rogg $version build $build"

    $dir = Resolve-Folder (Ask "Install to [$DefaultDir]" $DefaultDir)
    $exe = Join-Path $dir 'rogg.exe'
    if (Test-Path -LiteralPath $exe -PathType Container) { Fail "$exe is a folder; choose another place" }
    $current = Get-InstalledVersion $exe
    Say "Installed:  $(Describe $current) ($dir)"
    Say "Installing: rogg $version build $build"
    if ($current -match '^\S+ (\d+)$') {
      $have = [int]$Matches[1]
      if ($have -eq $build) { $go = YesNo 'Already up to date. Reinstall?' $false }
      elseif ($have -gt $build) { $go = YesNo 'The installed build is newer than the latest release. Downgrade?' $false }
      else { $go = YesNo 'Install?' $true }
    } else { $go = YesNo 'Install?' $true }
    if (-not $go) { Say 'Nothing changed.'; return }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('rogg-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
      $zip = Join-Path $tmp $Archive
      $sums = Join-Path $tmp 'SHA256SUMS'
      Invoke-WebRequest -UseBasicParsing "$Base/$Archive" -OutFile $zip
      Invoke-WebRequest -UseBasicParsing "$Base/SHA256SUMS" -OutFile $sums
      $want = $null
      foreach ($line in Get-Content -LiteralPath $sums) {
        $fields = $line -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].Trim() -eq $Archive) { $want = $fields[0].ToLower() }
      }
      if (-not $want) { Fail "$Archive is not listed in SHA256SUMS; nothing was changed" }
      $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLower()
      if ($got -ne $want) { Fail "checksum mismatch for $Archive; nothing was changed" }
      $unpacked = Join-Path $tmp 'x'
      Expand-Archive -LiteralPath $zip -DestinationPath $unpacked

      New-Item -ItemType Directory -Force -Path $dir | Out-Null
      Remove-OldExes $dir
      # A running rogg.exe can't be overwritten but can be renamed; the renamed file goes on the next run.
      $old = $null
      if (Test-Path -LiteralPath $exe) {
        $old = "$exe.$([guid]::NewGuid().ToString('N').Substring(0, 8)).old"
        Move-Item -LiteralPath $exe -Destination $old
      }
      try {
        Copy-Item -Path (Join-Path $unpacked '*') -Destination $dir -Recurse -Force
      } catch {
        if ($old -and -not (Test-Path -LiteralPath $exe)) { Move-Item -LiteralPath $old -Destination $exe }
        throw
      }
      Get-ChildItem -LiteralPath $dir -File | Unblock-File
      Remove-OldExes $dir
      Say "Installed rogg $version build $build to $dir."
    } finally {
      Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
    Add-ToPath $dir
    Add-Shortcut $exe
    Show-OtherCopy $exe
  }

  function Uninstall-Rogg {
    $exe = Join-Path $DefaultDir 'rogg.exe'
    $hasExe = Test-Path -LiteralPath $exe -PathType Leaf
    $hasLink = Test-Path -LiteralPath $Shortcut
    if (-not $hasExe -and -not $hasLink) {
      Say "No rogg installation found in $DefaultDir."
    } else {
      if ($hasExe) { Say "Found: $exe - $(Describe (Get-InstalledVersion $exe))" }
      if ($hasLink) { Say "Found: $Shortcut (Start menu)" }
      if (YesNo 'Remove it?' $true) {
        foreach ($f in $Files) { Remove-Item -LiteralPath (Join-Path $DefaultDir $f) -Force -ErrorAction SilentlyContinue }
        Remove-OldExes $DefaultDir
        Remove-Item -LiteralPath $Shortcut -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $exe) {
          Say 'rogg.exe is in use: quit rogg and run the uninstaller again to remove it.'
        } elseif ((Test-Path -LiteralPath $DefaultDir) -and -not (Get-ChildItem -LiteralPath $DefaultDir -Force)) {
          Remove-Item -LiteralPath $DefaultDir -Force
        }
        Say 'Removed.'
      }
    }
    $user = Get-UserPath
    if (Test-OnPath $DefaultDir $user) {
      if (YesNo "Remove $DefaultDir from your PATH?" $true) {
        $want = $DefaultDir.TrimEnd('\')
        $kept = @($user.Split(';') | Where-Object { $_ -and ([Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ne $want) })
        Set-UserPath ($kept -join ';')
      }
    }
    $cmd = Get-Command rogg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd -and -not $cmd.Path.StartsWith($DefaultDir, [StringComparison]::OrdinalIgnoreCase)) {
      Say "Also on PATH: $($cmd.Path) ($(Describe (Get-InstalledVersion $cmd.Path))). The installer did not put it there; remove it yourself if you want."
    }
    Say 'A rogg installed into another folder you chose has to be removed by hand.'
    Say "Settings and data are kept in $env:APPDATA\rogg and $env:LOCALAPPDATA\rogg\cache; delete them yourself if you want."
  }

  try {
    if ($Uninstall) { Uninstall-Rogg } else { Install-Rogg }
  } catch {
    # No exit: under `irm | iex` it would close the user's PowerShell window.
    Write-Host $_.Exception.Message -ForegroundColor Red
  }
}
```

- [ ] **Step 2: README**

In `README.md`'s "Install" section, after the macOS/Linux uninstall command block, add:

```markdown
On Windows (PowerShell), to install or upgrade, and to uninstall:

    irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex
    & ([scriptblock]::Create((irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1))) -Uninstall

It installs to `%LOCALAPPDATA%\Programs\rogg` (no admin rights), offers to add that folder to your PATH and a Start
menu entry, and reads the installed version from `rogg.exe`'s properties.
```

Change the first line of the section from "On macOS and Linux:" to "On macOS and Linux (bash):".

- [ ] **Step 3: Manual checks**

Append to `docs/superpowers/notes/2026-10-10-install-manual-checks.md`:

```markdown

## Windows (install.ps1), on Windows 10/11 with Windows PowerShell 5.1 and PowerShell 7

- Fresh: `irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex` in Windows PowerShell 5.1 →
  default folder `%LOCALAPPDATA%\Programs\rogg`; the PATH offer; the Start menu offer. A new terminal runs rogg; Start
  shows rogg with its icon; Explorer shows the icon on rogg.exe, and its Properties → Details show product version
  `rogg 0.1.0 (build N, main, <sha>)`.
- After the run, the PowerShell window is still open, `$ErrorActionPreference` is unchanged, and `Get-Command Install-Rogg`
  finds nothing (nothing leaked into the session).
- Upgrade with rogg open: run again after a newer build → "Installed: … build N" / "Installing: … build N+1"; it
  succeeds while rogg runs; the next run removes the leftover `rogg.exe.*.old`.
- PATH with `%VAR%` entries (e.g. add `%USERPROFILE%\bin` first): after the install, the entry is still unexpanded in
  System → Environment Variables.
- A checksum mismatch (point `$env:ROGG_INSTALL_BASE` at a copy with a broken SHA256SUMS): an error, nothing changed.
- `-Uninstall`: lists the install with its version; removes the files, the Start menu entry, and (after asking) the
  PATH entry; keeps `%APPDATA%\rogg`.
```

- [ ] **Step 4: Gates and commit**

Run: `./dev check`, `./dev ci-test`, `./dev windows-build`. Expected: green. `install.ps1` itself has no automated check, by decision.

```bash
git add install.ps1 README.md docs/superpowers/notes/2026-10-10-install-manual-checks.md
git commit -m "feat(windows): install.ps1 installs, upgrades and uninstalls rogg"
```

---

## Spec coverage

| Spec §9 item | Task |
|---|---|
| Resources: the icon, `VERSIONINFO`, `ProductVersion` = the `--version` text, Windows targets only, `llvm-rc` cross-build | 1 |
| `rogg.ico` from `./dev icon` (16–256 px; the small variant at 16/24) | 1 |
| `install.ps1` steps 1–9 (platform, TLS, manifest, folder, installed/installing, checksum, a running exe, PATH, Start menu, other copies) | 2 |
| `-Uninstall` | 2 |
| Robustness (no leaks, no exit, temporary files cleaned, Unblock-File) | 2 |
| README | 2 |
| Testing: none automated; manual checks | 2 |
| Out of scope | not implemented |
