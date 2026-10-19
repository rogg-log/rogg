# rogg — Auto-update — Design

Date: 2026-10-19
Status: Agreed in conversation; written for review
Builds on: `2026-10-10-rogg-ci-builds-design.md` (§10 a release per build, `manifest.json`), `2026-10-10-rogg-install-script-design.md` (how each install form is replaced), at `main` 76ec88b

## 1. Intent

rogg finds out by itself that a newer build exists, says so in the title bar, and updates its own installed copy when the user asks. The user restarts rogg to use the new build; rogg never restarts itself.

**Success means all of the following:**
1. At startup, a release build checks GitHub for a newer build in the background and shows an **Update** badge in the title bar when there is one. Nothing is shown otherwise, and a failed check is silent.
2. Clicking the badge opens a dialog with the new build's details. **Update** downloads it with progress, verifies its SHA-256, and replaces the running copy (`rogg.app`, the `rogg` binary, or `rogg.exe`) wherever it is installed.
3. When rogg can't write to that folder, it asks for admin rights through the platform's own prompt and finishes the replacement with them.
4. After a successful update the badge says **Restart to update**. Settings, history and the session are untouched.
5. The app menu has **Check for Updates…** right below **About rogg**. It always checks and shows the result, including "up to date".
6. Any failure leaves the installed copy working, either untouched or rolled back, and says what happened.

## 2. Decisions (agreed in conversation)

| Topic | Decision |
|---|---|
| When to download | Only when the user clicks **Update** in the dialog. The startup check only reads `manifest.json`. |
| Folder not writable | Ask for admin rights: `osascript … with administrator privileges` on macOS, `pkexec` on Linux, UAC (`ShellExecuteExW` `runas`) on Windows. |
| HTTP | `ureq` (blocking) with rustls and the OS certificate store (platform verifier), plus `sha2`. Archives are unpacked with the existing `flate2`, `tar` and `zip`. |
| Version comparison | The manifest's `build` against the build number compiled in (`ROGG_BUILD_NUMBER`). Only a higher build is offered. |
| Local builds | No build number: no automatic check. The manual check says updates apply to release builds only. |
| Restart | Never automatic. |
| Startup check | On by default; Settings → Behavior → **Check for updates on start** turns it off. The menu item works either way. |

## 3. Behaviour and UI

### 3.1 Startup check

- About 3 s after the first window opens, if the setting is on and the build has a number, rogg fetches `https://github.com/rogg-log/rogg/releases/latest/download/manifest.json` on a background thread. This is a plain file download: no GitHub API, so no rate limit and no token.
- If the manifest's `build` is higher than the running build, and an asset exists for this install (§4.2), the state becomes **Available** and the badge appears.
- Any error (offline, timeout, HTTP error, bad JSON, no asset for this platform) is silent at startup and leaves the state **Idle**.

### 3.2 Badge

- A small accent-coloured pill in the title bar's action area, left of the ⌄ / ⌘ / ⚙ buttons. It is shown in every chrome style (Mac, Linux, Windows).
- **Available:** label "Update", tooltip "rogg <version> build <n> is available".
- **Installed:** label "Restart to update", tooltip "Build <n> is installed. Restart rogg to use it."
- Clicking it in either state opens the Update dialog. In any other state there is no badge.

### 3.3 Update dialog (`Dialog::Update`)

One dialog whose content follows the updater state:

| State | Content | Buttons |
|---|---|---|
| Checking (manual check only) | "Checking for updates…" | Close |
| Up to date (manual check only) | "rogg is up to date (build <n>)." | Close |
| Local build (manual check only) | "This is a local build. Updates apply to release builds only." | Close |
| Available | "rogg <version> · build <n> · <date> · <short sha>" and "You have build <m>." | **Update**, Release notes, Later |
| Downloading | Progress bar, "<done> of <total> MB" | Cancel |
| Installing | "Installing…". The admin prompt, when needed, appears now. | none |
| Installed | "Build <n> is installed. Restart rogg to use it." | Close |
| Failed | A one-line reason (§5) | **Retry**, Release notes, Close. When admin rights can't be requested (no `pkexec`), also **Copy install command**, which copies the `curl … \| bash` line. |

- **Release notes** opens the release page (`https://github.com/rogg-log/rogg/releases/tag/v<version>-build.<n>`) in the browser.
- **Later** and **Close** only close the dialog; the badge stays.
- **Cancel** stops the download, deletes its files, and returns to **Available**.
- Closing the dialog during Downloading or Installing doesn't stop the job; the badge reopens it.

### 3.4 Menu

- A new action `CheckForUpdates`, labelled **Check for Updates…**, goes right below **About rogg** in the native "rogg" menu (macOS) and in the ☰ app menu (Linux, Windows).
- It always fetches the manifest again (unless a download or install is running, in which case it just opens the dialog) and opens the dialog on the result.

### 3.5 Setting

- `behavior.check_updates: bool`, default `true`, saved with the other settings.
- Shown in Settings → Behavior as **Check for updates on start**.

## 4. Code layout

### 4.1 `rogg-core::update` (new module; no gpui, no network)

Pure logic, unit-tested.

- **`Manifest`:** `version`, `build`, `commit`, `date`, `channel` and `assets: Vec<Asset { name, os, arch, sha256, size }>`, parsed with `serde_json`. Unknown fields are ignored.
- **`Install`:** `App { bundle: PathBuf }` | `Binary { path: PathBuf }`.
- **`Install::detect(current_exe)`:**
  - On macOS, a path ending in `<X>.app/Contents/MacOS/rogg` is `App { bundle: <X>.app }`.
  - Otherwise it is `Binary { path }`.
  - Paths are canonicalized first.
- **`asset_name(install, os, arch) -> Option<&'static str>`:**

  | Install | Asset |
  |---|---|
  | macOS app | `rogg-macos-universal-app.tar.gz` |
  | macOS binary | `rogg-macos-universal.tar.gz` |
  | Linux x86_64 | `rogg-linux-x86_64.tar.gz` |
  | Linux aarch64 | `rogg-linux-arm64.tar.gz` |
  | Windows x86_64 | `rogg-windows-x86_64.zip` |

  Anything else is `None`, which means no update for this platform.
- **`is_newer(available: u64, running: u64) -> bool`.**
- **`verify(file, sha256_hex) -> Result<()>`:** streams the file through `sha2::Sha256`.
- **`stage(archive, dir, install) -> Result<PathBuf>`:**
  - Unpacks `.tar.gz` (`flate2` + `tar`) or `.zip` (`zip`) into `dir`.
  - Checks that the expected item exists: `rogg.app` with `Contents/MacOS/rogg`, `rogg`, or `rogg.exe`.
  - Returns its path.
  - Archive entries that would escape `dir` (absolute paths or `..`) are rejected.
- **`apply(staged, target, style) -> Result<()>`:** the replacement.
  1. Refuse a target whose file name isn't `rogg`, `rogg.exe` or `<name>.app` containing `Contents/MacOS/rogg`.
  2. Move (rename, or copy then rename when across file systems) `staged` to `<target>.rogg-new` beside the target.
  3. Rename `target` to `<target>.rogg-old`.
  4. Rename `<target>.rogg-new` to `target`.
  5. Delete `<target>.rogg-old`. With the Windows style, a running `rogg.exe` can't be deleted, so it is renamed to `rogg.exe.old` instead.

  - If step 3 or 4 fails, rename `.rogg-old` back to `target`. The error says whether that rollback worked, and if not, where the old copy is.
  - The binary's mode is set to 755 on Unix.
  - `style` (Unix or Windows) defaults to the build's OS; tests set it explicitly.
- **`sweep(dir_of_target, cache_update_dir)`:** at startup, deletes `rogg.exe.old`, stale `.rogg-new`/`.rogg-old` beside the installed copy, and `<cache>/update/`.
- **`can_write(dir) -> bool`:** creates and removes a test file in `dir`.

### 4.2 `rogg-app::updater` (new module)

- **HTTP** (`ureq`; rustls with the platform verifier):
  - `fetch_manifest(base)`: 10 s timeout.
  - `download(url, dest, on_progress, cancel)`: streams to a temp file, reports bytes done and total, honours a `CancelToken`, and fails if no data arrives for 30 s.
  - `base` defaults to `https://github.com/rogg-log/rogg/releases/latest/download`. The environment variable `ROGG_UPDATE_BASE` overrides it, for tests and staging, like `ROGG_INSTALL_BASE` in `install.sh`.
- **`Updater`:** an app global holding `UpdateState`:

  ```
  Idle | Checking { manual } | UpToDate | LocalBuild | Available(Release)
  | Downloading { release, done, total } | Installing(release) | Installed(release) | Failed { release: Option<Release>, reason }
  ```

  - `Release` is the manifest and the chosen asset.
  - One job at a time, on a background thread, reporting over a channel that the app polls on the existing poll interval (as document and source jobs do).
  - Temp files go to `<cache>/update/<build>/`.
- **Job steps:**
  1. Download the asset.
  2. `verify`.
  3. `stage`.
  4. If `can_write(parent of target)`, run `apply` in-process; otherwise elevate (§4.3).
  5. Delete the temp folder.
  6. Set the state to **Installed**.
- **Hidden CLI mode** (`rogg-app::args`): `rogg --apply-update <staged> <target>` runs `rogg-core::update::apply` and exits with status 0 on success, 1 on failure (the message goes to stderr), and 2 for a refused target. It opens no window and is meant only for the elevated helper.

### 4.3 Elevation (`rogg-app::updater::elevate`)

The elevated command is always the running executable (`std::env::current_exe()`) with `--apply-update <staged> <target>`.

| OS | How | Declined |
|---|---|---|
| macOS | `osascript -e 'do shell script "<cmd>" with administrator privileges'`. Every path is passed as AppleScript `quoted form of`. | osascript error `-128` → "Admin rights were declined." |
| Linux | `pkexec <exe> --apply-update <staged> <target>`. | Exit status 126 or 127 → "Admin rights were declined." No `pkexec` on `PATH` → "rogg can't ask for admin rights here (pkexec not found)", plus the install command to copy. |
| Windows | `ShellExecuteExW` with verb `runas` and `SEE_MASK_NOCLOSEPROCESS`; wait for the process and read its exit code. Adds the `windows` crate (Shell features only), pinned to the version already in `Cargo.lock`. | `ERROR_CANCELLED` → "Admin rights were declined." |

The helper's exit status maps to the dialog's Failed reason.

### 4.4 `rogg-ui`

- **`TitleBar::update_badge(label, tooltip, on_click)`:** the pill in the action area (§3.2), with id `update-badge`.
- **`dialogs::update(view: &UpdateView)`:** the dialog. `UpdateView` is a plain view-model with a title, lines, an optional progress fraction and label, and buttons (each a label and a callback).

### 4.5 Wiring

- **Workspace:**
  - Starts the startup check (§3.1).
  - Calls `sweep` at start.
  - Renders the badge from the `Updater` state.
  - Opens `Dialog::Update` from the badge and the menu.
  - Re-renders on updater progress.
- **Menus:** `menus.rs` adds `CheckForUpdates` below `ShowAbout`.
- **Settings:** `rogg-core::settings::BehaviorSettings` gains `check_updates` (serde default `true`), and the Settings dialog gains its toggle.

### 4.6 New dependencies (exact pins, chosen at implementation)

- `ureq`, with rustls and the platform verifier.
- `sha2`, the version already in `Cargo.lock`.
- `windows`, for Windows targets only, with Shell features, at the version already in `Cargo.lock`.

## 5. Failure handling

Each failure ends in **Failed** with a short reason and **Retry**. In every case the installed copy keeps working.

| Failure | Reason shown |
|---|---|
| Offline or DNS failure | "No network connection." |
| Timeout or stalled download | "The download timed out." |
| HTTP error | "GitHub returned <status>." |
| Bad manifest | "The release information couldn't be read." |
| No asset for this install | "No update is published for this platform." |
| Checksum mismatch | "The download is damaged (checksum mismatch). Nothing was changed." |
| Unpacking failed or expected item missing | "The download couldn't be unpacked. Nothing was changed." |
| Admin rights declined, or `pkexec` missing | As in §4.3. |
| Replace failed, rolled back | "The update couldn't be installed. The current version was kept." |
| Replace failed, rollback failed | "The update failed. The previous version is at <path>." |

Leftovers from a quit or crash mid-update are removed by `sweep` at the next start.

**Trust:** HTTPS to GitHub plus the SHA-256 checksums from the manifest. This is the same trust model as `install.sh` and `install.ps1`. Files written by rogg get no macOS quarantine flag, so an updated `rogg.app` opens without a Gatekeeper prompt, as after `install.sh`.

## 6. Testing

### 6.1 Automated (in `./dev check`)

- **`rogg-core::update`:**
  - Manifest parsing, with fields missing or extra.
  - `is_newer`.
  - `Install::detect` for `.app`, bare binary and Windows paths.
  - `asset_name` for every OS and architecture, plus unsupported ones.
  - `verify` with a good and a bad checksum.
  - `stage` from `.tar.gz` and `.zip` archives built in the test: a good one, one missing the expected item, and one with an escaping entry.
  - `apply`:
    - It replaces a binary and an `.app` tree, leaving no `.rogg-new`/`.rogg-old`.
    - It refuses a target that isn't rogg.
    - A failure partway through rolls back.
    - The Windows style leaves `rogg.exe.old`.
  - `sweep`.
- **`rogg-app::updater`, against a local HTTP server on `127.0.0.1`** started in the test and reached through `ROGG_UPDATE_BASE`:
  - Up to date.
  - Newer available.
  - A full update of a fake installed `rogg` in a temp folder, with progress events.
  - Cancel.
  - A checksum mismatch.
  - HTTP 404.
  - A stalled server timing out.
- **App/UI:**
  - The badge shows only in **Available** and **Installed**, with the right labels.
  - Each dialog state renders its text and buttons.
  - **Check for Updates…** sits right below **About rogg** in the native menu and the ☰ menu.
  - A local build never auto-checks, and its manual check shows the local-build message.
  - `check_updates = false` stops the startup check.
- **CLI:** `--apply-update` refuses targets that aren't rogg (exit status 2).

### 6.2 Manual (`docs/superpowers/notes/2026-10-19-auto-update-manual-checks.md`)

- The three admin prompts, each approved and each cancelled:
  - macOS `/Applications/rogg.app` as a standard user;
  - Linux `/usr/local/bin/rogg` via `pkexec`;
  - Windows `C:\Program Files\rogg\rogg.exe` via UAC.
- A real update from one published build to the next:
  - `rogg.app` in `/Applications` as an admin user;
  - the binary in `~/.local/bin` on macOS and Linux;
  - `rogg.exe` in `%LOCALAPPDATA%\Programs\rogg`.

  After each, restart and check `rogg --version` (or the About dialog).
- Behaviour behind a proxy with its own root certificate.

### 6.3 Before merge

- `./dev check-all`: macOS, the Linux container and the Windows cross-build.
- Screenshots of the badge and of each dialog state, for the user's approval.

## 7. Out of scope

- Automatic restart.
- Downgrading, or pinning a build.
- Channels other than `main`.
- Release notes inside the dialog (the dialog links to the release page).
- Delta updates.
- Signature verification beyond SHA-256.
- Periodic checks while rogg keeps running.
