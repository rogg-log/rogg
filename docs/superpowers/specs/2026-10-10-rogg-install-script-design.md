# rogg — install.sh — Design

Date: 2026-10-10
Status: Agreed in conversation; written for review
Builds on: `2026-10-10-rogg-ci-builds-design.md` (§9 app bundle, §10 a release per build), at `main` cbdea91

## 1. Intent

One command installs or upgrades rogg on macOS and Linux from the latest GitHub release, interactively:

```
curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash
curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash -s -- --uninstall
```

**Success means all of the following:**
1. On macOS the user chooses `rogg.app` or the command-line binary, and where to put it. On Linux it installs the binary.
2. Before anything changes, the user sees what is installed now (version, build, place) and what will be installed, and confirms.
3. The download is checked against `SHA256SUMS`. A mismatch changes nothing.
4. For a binary outside `PATH`, the script offers to add the folder to the shell's startup file.
5. On Linux it offers a desktop entry with the icon, so rogg appears in the app menu.
6. `--uninstall` removes what the installer put in place and keeps settings and data.

## 2. Decisions (agreed in conversation)

| Topic | Decision |
|---|---|
| Repository access | Public repo only: plain `curl`, no tokens. Until the repo is public the script is tested only against a local fake release (§7). |
| Installed version | A new `rogg --version` (Rust) prints e.g. `rogg 0.1.0 (build 5, main, cbdea91)`. The installer reads it from the binary or from `rogg.app/Contents/MacOS/rogg`. |
| macOS app default | `/Applications`, using `sudo` when it isn't writable; `~/Applications` and a custom folder are offered too. |
| Binary default | `~/.local/bin` on macOS and Linux; a custom folder is offered. |
| Extra scope | `--uninstall` and a Linux desktop entry. Not now: a terminal command pointing into the app, and unattended flags. |
| Windows | Out of scope (`install.sh` is for macOS and Linux). |

## 3. `rogg --version` (rogg-app)

- `rogg --version` (and `-V`) prints one line to stdout and exits 0 without opening a window:
  - CI builds: `rogg <version> (build <n>, <channel>, <commit>)`, e.g. `rogg 0.1.0 (build 5, main, cbdea91)`;
  - local builds: `rogg <version> (local, <commit>)`.
- The values come from the existing `about::build_info()`: `ROGG_BUILD_NUMBER`, `ROGG_BUILD_CHANNEL` and `ROGG_GIT_COMMIT`.
- The usage line and the args tests cover it.
- On Windows the release binary has no console (`windows_subsystem`), so the output is not visible there. That's acceptable, since the installer doesn't target Windows.

## 4. The script — `install.sh` at the repository root

**Input and output:**
- Prompts read from `/dev/tty`, because with `curl | bash` stdin is the script. With no terminal the script stops with a clear message. Tests point `ROGG_INSTALL_INPUT` at a file of answers.
- Output is plain, short lines; colour only when stdout is a terminal.

**Steps:**
1. **Platform:** `uname -s`/`-m` → `macos-universal`, `linux-x86_64` or `linux-arm64`; anything else stops with a message naming the supported platforms.
2. **Latest release:**
   - download `manifest.json` from `https://github.com/rogg-log/rogg/releases/latest/download/`, which can be overridden with `ROGG_INSTALL_BASE` (used by the tests, and for installing a specific build);
   - read `version` and `build` from it with `sed`; no `jq` or Python, which macOS and minimal Linux may lack.
3. **What (macOS only):** "1) rogg.app — open from Finder (recommended), 2) command-line binary". Linux always installs the binary.
4. **Where:**
   - the app goes to `/Applications` by default, with `~/Applications` or a custom folder offered;
   - the binary goes to `~/.local/bin` by default, with a custom folder offered.
5. **Installed now:** run `<place>/rogg --version`, or `<place>/rogg.app/Contents/MacOS/rogg --version`. Then show one of:
   - `Installed: rogg 0.1.0 build 4 (/Applications/rogg.app)` / `Installing: rogg 0.1.0 build 5`;
   - `not installed`;
   - `unknown version` for builds older than `--version`.

   Other copies found on `PATH` are mentioned. An equal build asks "Already up to date. Reinstall? [y/N]", and an older available build warns that it's a downgrade, defaulting to no.
6. **Confirm:** `Install? [Y/n]`.
7. **Download** the archive and `SHA256SUMS` into a temporary folder, and check the archive with `shasum -a 256` or `sha256sum`. On a mismatch, stop with nothing changed.
8. **Install:**
   - **The app:** unpack, then replace `<place>/rogg.app`; the old bundle is moved aside and removed only after the new one is in place. `sudo` is used only when `<place>` isn't writable, with a note saying why.
   - **The binary:** unpack `rogg` and install it as `<place>/rogg` (mode 755) by writing a temporary file in `<place>` and renaming it.
9. **PATH (binary only):** if `<place>` isn't on `PATH`, offer `[Y/n]` to append a marked line to the shell's startup file:
   - zsh: `~/.zshrc`;
   - bash: `~/.bashrc` on Linux, `~/.bash_profile` on macOS;
   - fish: `~/.config/fish/conf.d/rogg.fish`;
   - the line, e.g. `export PATH="$HOME/.local/bin:$PATH"  # added by rogg install.sh`.

   Then say to open a new terminal.
10. **Desktop entry (Linux only):** offer `[Y/n]`.
    - Copy `rogg.png` from the archive to `~/.local/share/icons/hicolor/256x256/apps/rogg.png`.
    - Write `~/.local/share/applications/rogg.desktop`: `Name=rogg`, `Comment=Log viewer`, `Exec=<place>/rogg %F`, `Icon=rogg`, `Terminal=false`, `Type=Application`, `Categories=Development;Utility;`, `MimeType=text/plain;text/x-log;`.
    - Run `update-desktop-database` if it exists.
11. **Done:** say what was installed where, and how to start it.

**`--uninstall`:**
- Looks for rogg in the default places and on `PATH`: `/Applications/rogg.app`, `~/Applications/rogg.app` and `~/.local/bin/rogg`.
- Lists what it found with versions, confirms, and removes them (using `sudo` only where needed).
- Removes the desktop entry and icon if present, and offers to remove the marked `PATH` line(s).
- Prints where settings and data stay, the platform folders from the cross-platform spec §3, and that deleting them is up to the user.
- Anything outside the default places has to be removed by hand; the script says so.

**Robustness:** `set -euo pipefail`; a temporary folder removed on exit; every prompt has a default; the script never runs `sudo` silently.

## 5. Release changes

- **Linux archives gain `rogg.png`** (256×256, the app icon) for the desktop entry. `./dev icon` also renders `crates/rogg-app/assets/icon/rogg.png`, which is committed, and `package.sh` adds it for Linux only.
- **The script itself** is served from the repository (`raw.githubusercontent.com/rogg-log/rogg/main/install.sh`). It is not attached to releases.

## 6. README

A new "Install" section near the top covers:
- the one-line command;
- what the script asks;
- `--uninstall`;
- manual download from the Releases page as the alternative;
- the note that while the repository is private, the script and downloads don't work without access.

## 7. Testing (`tools/ci/test-install.sh`, run by `./dev ci-test` and the `check` job)

- **Setup:**
  - a fake release in a temporary folder: archives whose `rogg` is a small script answering `--version` with a chosen build, `SHA256SUMS` and `manifest.json` built with the real `package.sh`, `macos-app.sh` and `manifest.sh`;
  - served through `file://` with `ROGG_INSTALL_BASE`;
  - `HOME` is a temporary folder, and the answers come from `ROGG_INSTALL_INPUT`;
  - `ROGG_INSTALL_PLATFORM` forces `linux-x86_64` or `macos-universal`, so both paths run on either OS (a test seam, documented as such).
- **Cases:**
  - a fresh binary install to `~/.local/bin`, with the `PATH` line added to `.zshrc` once (a second run doesn't add it again);
  - an upgrade from build 6 to 7, showing both;
  - an up-to-date install skipped on "no";
  - a downgrade warned about and refused by default;
  - a checksum mismatch stopping with the old binary untouched;
  - a macOS app install to a custom folder, which avoids `sudo`;
  - a Linux desktop entry with the right `Exec` and icon;
  - `--uninstall` removing the binary, the desktop entry and the marked `PATH` line;
  - an unsupported platform stopping cleanly;
  - no terminal stopping with the message.
- **Static checks:** `shellcheck install.sh` via the actionlint image (`./dev ci-lint`).
- **`--version`:** a Rust test in rogg-app's args tests, and `./dev check` stays green.
- **End to end:** after the repository is public, a real run on macOS and on Linux (the container). This is listed in the manual checks.

## 8. Out of scope

- Windows (`install.ps1`).
- Unattended flags.
- A terminal command pointing into the app.
- Installing a pinned build by flag (possible through `ROGG_INSTALL_BASE`, but not advertised).
- Signing and notarisation.
- Package managers (Homebrew, apt).
- Auto-update inside the app.

## 9. Amendment (2026-10-10): Windows — `install.ps1`, and the version and icon built into `rogg.exe`

Agreed in conversation: a PowerShell installer for Windows, the version and icon embedded in `rogg.exe` as Windows resources, and **no automated installer tests** (checked by hand on Windows).

**`rogg.exe` resources (`crates/rogg-app/build.rs`, Windows targets only):**
- **Why:** a release `rogg.exe` has no console, so `rogg.exe --version` prints nothing an installer can read.
- **How:** when `CARGO_CFG_TARGET_OS` is `windows`, `build.rs` writes a resource script into `OUT_DIR` and compiles it with `embed-resource` (already in `Cargo.lock` through gpui). It contains:
  - `1 ICON` → `crates/rogg-app/assets/icon/rogg.ico`;
  - `1 VERSIONINFO` with:
    - `FILEVERSION` / `PRODUCTVERSION` `<major>,<minor>,<patch>,<build or 0>`;
    - strings `ProductName` `rogg`, `FileDescription` `rogg log viewer`, `CompanyName` `rogg-log`, `OriginalFilename` `rogg.exe`, `FileVersion` `<version>.<build or 0>`;
    - `ProductVersion` = the same text as `rogg --version`: `rogg 0.1.0 (build 6, main, 3b4060e)` or `rogg 0.1.0 (local, <commit>)`.
- **Format:** `build.rs` repeats `about::version_line`'s format, because a build script can't call the crate. A comment ties the two together.
- **Platforms:** macOS and Linux builds are untouched. The local cross-build (`./dev windows-build`) compiles the script with LLVM's `llvm-rc`, which is already on its PATH.
- **The icon:** `./dev icon` also writes `rogg.ico`: PNG-compressed 16, 24, 32, 48, 64 and 256 px images in one ICO file, from the same SVGs (the small variant for 16 and 24 px). It is committed. Explorer, the taskbar and the Start-menu shortcut then show the app icon.

**`install.ps1` (repository root), Windows PowerShell 5.1 and PowerShell 7:**

```powershell
irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1))) -Uninstall
```

**Steps:**
1. **Platform:** x64 Windows. ARM64 Windows also gets the x64 build, which runs under emulation, with a note saying so.
2. **Latest release:** turn on TLS 1.2 (Windows PowerShell 5.1 may default lower), then read `manifest.json` with `Invoke-RestMethod`, which parses JSON natively. `$env:ROGG_INSTALL_BASE` overrides the release URL, as in `install.sh`.
3. **Where:** `%LOCALAPPDATA%\Programs\rogg` by default (per user, no admin rights); another folder is offered. Relative folders and the characters `"` and `$` are refused, matching `install.sh`.
4. **Installed now:** read `(Get-Item <folder>\rogg.exe).VersionInfo.ProductVersion` with the same `build N` / `local` / unknown logic, then show `Installed:` / `Installing:`. Reinstall and downgrade prompts default to no, as in `install.sh`.
5. **Download and check:** `rogg-windows-x86_64.zip` and `SHA256SUMS` go into a temporary folder, checked with `Get-FileHash -Algorithm SHA256`. A mismatch changes nothing.
6. **Install:** unpack with `Expand-Archive`. A running `rogg.exe` can't be overwritten but can be renamed, so an existing one is renamed to `rogg.exe.old` first (deleted now if possible, otherwise on the next run). Then copy the archive's files into the folder.
7. **PATH:** if the folder isn't on the user `PATH` (`[Environment]::GetEnvironmentVariable('Path','User')`), offer to add it. Then say to open a new terminal.
8. **Start menu:** offer a shortcut, `%APPDATA%\Microsoft\Windows\Start Menu\Programs\rogg.lnk` created through `WScript.Shell`, with the icon from `rogg.exe`.
9. **Other copies:** if `Get-Command rogg` resolves to another copy, say which one a terminal will run.

**`-Uninstall`:**
- Removes rogg from the default folder: the files the archive installs, `rogg.exe.old`, and the folder if it's then empty.
- Removes the Start-menu shortcut, and offers to remove the folder from the user `PATH`.
- Points out a `rogg.exe` found elsewhere without deleting it.
- Says that settings and data are kept in `%APPDATA%\rogg` and `%LOCALAPPDATA%\rogg\cache`.

**Robustness:** `$ErrorActionPreference = 'Stop'`; everything is a function, and the last line calls it, so a cut-off download runs nothing. Temporary files are removed in a `finally` block. Downloads made with `Invoke-WebRequest` carry no internet-zone mark, so SmartScreen doesn't prompt. `Unblock-File` is run on the installed files anyway.

**README:** the "Install" section gains the two PowerShell commands.

**Testing:** none automated, by decision. The Windows CI build proves that the resources compile. A manual-checks section covers: a fresh install, an upgrade, the icon in Explorer and Start, a version read back, `PATH`, `-Uninstall`, and replacing a running `rogg.exe`.

**Out of scope:** an MSI or winget/scoop packages, installing to Program Files (that would need admin rights), Windows ARM64 builds, code signing.
