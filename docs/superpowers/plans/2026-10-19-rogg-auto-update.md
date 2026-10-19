# Auto-update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rogg checks GitHub for a newer build at startup, shows an Update badge, and on request downloads, verifies and installs the new build over the running copy (asking for admin rights when needed). The user restarts it.

**Architecture:**
- **Pure logic** (manifest, install detection, asset choice, checksum, unpacking, replace with rollback) lives in a new `rogg-core::update` module with no network and no gpui.
- **`rogg-app::updater`** adds HTTP (`ureq`), the background job and state machine, and the per-OS admin-rights launchers.
- **`rogg-ui`** adds the title-bar badge and the Update dialog view.
- **The workspace** wires the startup check, the menu item, the setting and the dialog.

**Tech Stack:** Rust 1.99 (edition 2024), gpui-kit 0.7.1, `ureq` 3.4.2 (rustls + platform verifier), `sha2` 0.10.9, existing `flate2`/`tar`/`zip`, `windows-sys` 0.61.2 (Windows only).

**Spec:** `docs/superpowers/specs/2026-10-19-rogg-auto-update-design.md`

## Global Constraints

- **Manifest URL:** `https://github.com/rogg-log/rogg/releases/latest/download/manifest.json`. The base can be overridden by the environment variable `ROGG_UPDATE_BASE`.
- **Release page:** `https://github.com/rogg-log/rogg/releases/tag/v<version>-build.<n>`.
- **Comparison:** only `manifest.build > running build` is offered. Builds without `ROGG_BUILD_NUMBER` (local) never auto-check.
- **Timeouts:** the startup check runs 3 s after the first window opens. A check times out after 10 s; a download fails after 30 s without data.
- **Assets:**

  | Install | Asset |
  |---|---|
  | macOS app | `rogg-macos-universal-app.tar.gz` |
  | macOS binary | `rogg-macos-universal.tar.gz` |
  | Linux x86_64 | `rogg-linux-x86_64.tar.gz` |
  | Linux aarch64 | `rogg-linux-arm64.tar.gz` |
  | Windows x86_64 | `rogg-windows-x86_64.zip` |

- **Archive layouts:** `rogg` or `rogg.exe` at the archive root; the app archive has `rogg.app/` at the root.
- **Temp files** go in `<cache>/update/<build>/`.
- **Names beside the target:** `.rogg-new` and `.rogg-old` suffixes; on Windows the running exe becomes `rogg.exe.old`.
- **Hidden CLI:** `rogg --apply-update <staged> <target>` exits 0 on success, 1 on failure (message on stderr), 2 for a refused target.
- **Badge** (id `update-badge`):
  - Available: label `Update`, tooltip `rogg <version> build <n> is available`.
  - Installed: label `Restart to update`, tooltip `Build <n> is installed. Restart rogg to use it.`
- **Menu:** the item `Check for Updates…` (action `CheckForUpdates`) goes directly below `About rogg`, in the native menu and the ☰ menu.
- **Setting:** `behavior.check_updates`, default `true`, labelled `Check for updates on start`.
- **Dialog copy (exact):**

  | State | Copy |
  |---|---|
  | Checking | "Checking for updates…" |
  | Up to date | "rogg is up to date (build <n>)." |
  | Local build | "This is a local build. Updates apply to release builds only." |
  | Available | "rogg <version> · build <n> · <date> · <short sha>" and "You have build <m>." |
  | Downloading | "<done> of <total> MB" |
  | Installing | "Installing…" |
  | Installed | "Build <n> is installed. Restart rogg to use it." |

- **Buttons:**

  | State | Buttons |
  |---|---|
  | Checking, Up to date, Local build, Installed | Close |
  | Available | Update, Release notes, Later |
  | Downloading | Cancel |
  | Installing | none |
  | Failed | Retry, Release notes, Close, plus Copy install command when `pkexec` is missing |

- **Failure copy (exact, spec §5):**

  | Failure | Copy |
  |---|---|
  | Offline | "No network connection." |
  | Timeout | "The download timed out." |
  | HTTP error | "GitHub returned <status>." |
  | Bad manifest | "The release information couldn't be read." |
  | No asset | "No update is published for this platform." |
  | Checksum | "The download is damaged (checksum mismatch). Nothing was changed." |
  | Unpack | "The download couldn't be unpacked. Nothing was changed." |
  | Declined | "Admin rights were declined." |
  | No pkexec | "rogg can't ask for admin rights here (pkexec not found)." |
  | Rolled back | "The update couldn't be installed. The current version was kept." |
  | Rollback failed | "The update failed. The previous version is at <path>." |

- **Install command** copied on "No pkexec": `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash`.
- **Dependencies:** pin exactly (`=x.y.z`) in the workspace `Cargo.toml`, like the others.
- **Commits:** none carry a Co-Authored-By or Claude trailer (project rule).
- **Gate:** `./dev check` must stay green after every task (goldens 0.000%).

**Deviation from the spec, decided while planning:** the Windows admin prompt uses `windows-sys` 0.61.2, already a workspace dependency, by adding its `Win32_UI_Shell` and `Win32_UI_WindowsAndMessaging` features. It does not add the separate `windows` crate. The behaviour is the same and there's one dependency fewer.

## Review Focus

1. **Checking again after an install.** The manifest still says build N, which is newer than the running build. The state must stay **Installed** ("Restart to update") and never offer or download build N again. Test: Task 5.
2. **A symlinked binary** (`~/.local/bin/rogg` → `/opt/rogg/rogg`). The real file is replaced and the symlink stays a symlink. Test: Task 1 (`detect` canonicalizes) and Task 2 (`apply` on the resolved path).
3. **Cache and install on different volumes.** The staged copy lives on one volume and the target on another; `rename` fails with a cross-device error. The move must fall back to copying, then renaming within the target folder. Test: Task 2 (`move_into` copy branch).
4. **Update or Check clicked again while a job runs.** No second job starts, and the dialog shows the running job. Test: Task 5.
5. **A macOS app running from App Translocation** (`…/AppTranslocation/…/rogg.app`, quarantined and launched from Downloads). Updating that copy is pointless. Detection returns `Unsupported`, the startup check is silent, and the manual check says "No update is published for this platform." Test: Task 1.

---

### Task 1: `rogg-core::update` — manifest, install detection, asset choice

**Files:**
- Create: `crates/rogg-core/src/update/mod.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod update;`)
- Test: `crates/rogg-core/tests/update.rs`

**Interfaces:**
- Produces:
  - `Manifest { version: String, build: u64, commit: String, date: String, channel: String /* serde default */, assets: Vec<Asset> }`, with `Manifest::parse(json: &str) -> Result<Manifest, UpdateError>`, `asset(&self, name: &str) -> Option<&Asset>`, `short_commit(&self) -> &str` (first 7 characters) and `release_url(&self) -> String`.
  - `Asset { name, os, arch, sha256: String, size: u64 }`.
  - `Install::{App { bundle: PathBuf }, Binary { path: PathBuf }, Unsupported }`, with `Install::detect(exe: &Path, os: Os) -> Install` and `target(&self) -> Option<&Path>`.
  - `asset_name(install: &Install, os: Os, arch: &str) -> Option<&'static str>`.
  - `is_newer(available: u64, running: u64) -> bool`.
  - `UpdateError` (thiserror): `BadManifest(String)`, `Checksum`, `Unpack(String)`, `MissingItem(&'static str)`, `NotRogg(PathBuf)`, `Io { path: PathBuf, source: io::Error }`, `Replace { rolled_back: bool, old: PathBuf, source: io::Error }`.
- Consumes: `rogg_core::platform::Os`.

- [ ] **Step 1: Write the failing tests** in `crates/rogg-core/tests/update.rs`:
  - `parses_a_release_manifest`: the JSON shape from the CI spec §5/§10 with `"version":"0.1.0","build":14,"commit":"a1b2c3d4e5f6…","date":"2026-10-18T10:00:00Z","channel":"main"`, two assets and an unknown extra field.
    - `build == 14`
    - `short_commit() == "a1b2c3d"`
    - `asset("rogg-linux-arm64.tar.gz").unwrap().size` matches
    - `release_url() == "https://github.com/rogg-log/rogg/releases/tag/v0.1.0-build.14"`
  - `a_manifest_without_build_is_rejected`: `Manifest::parse(r#"{"version":"0.1.0"}"#)` is `Err(UpdateError::BadManifest(_))`.
  - `is_newer_compares_builds`: `is_newer(14, 11)`, `!is_newer(11, 11)`, `!is_newer(10, 11)`.
  - `detects_an_app_bundle_and_a_binary`, using real temp dirs, since `detect` canonicalizes:
    - `<tmp>/Apps/rogg.app/Contents/MacOS/rogg` with `Os::Mac` → `App { bundle: <tmp canonical>/Apps/rogg.app }`;
    - `<tmp>/bin/rogg` with `Os::Mac` → `Binary`;
    - the same `.app` path with `Os::Linux` → `Binary` (only macOS has bundles).
  - `a_symlinked_binary_resolves_to_the_real_file` (Unix only): `<tmp>/bin/rogg` → `<tmp>/opt/rogg`; `detect` gives `Binary { path: <tmp canonical>/opt/rogg }`.
  - `a_translocated_app_is_unsupported`: a path containing the component `AppTranslocation` with `Os::Mac` → `Install::Unsupported`.
  - `asset_names_per_platform`: every row of the Global Constraints asset table, plus `("linux", "riscv64")` → `None`, `Unsupported` → `None`, and Windows `aarch64` → `None`.
- [ ] **Step 2: Run them and see them fail**
  - Run: `./dev test core update`
  - Expected: a compile error, because `rogg_core::update` doesn't exist.
- [ ] **Step 3: Implement** `crates/rogg-core/src/update/mod.rs` with the interfaces above.
  - `detect` canonicalizes `exe` and falls back to the path as given if canonicalizing fails.
  - The `.app` check is: the parent is named `MacOS`, its parent is `Contents`, and that one's parent ends in `.app`.
  - `Unsupported` covers any component named `AppTranslocation`.
  - `arch` is `std::env::consts::ARCH` (`x86_64`, `aarch64`); macOS ignores it (universal).
- [ ] **Step 4: Run the tests and see them pass**
  - Run: `./dev test core update`
  - Expected: all pass.
- [ ] **Step 5: Commit** `feat(update): release manifest, install detection and asset choice`

### Task 2: `rogg-core::update::apply` — verify, stage, replace, sweep

**Files:**
- Create: `crates/rogg-core/src/update/apply.rs` (with `pub mod apply;` in `update/mod.rs`)
- Modify: `Cargo.toml` (workspace dependencies: `sha2 = "=0.10.9"`), `crates/rogg-core/Cargo.toml` (`sha2.workspace = true`)
- Test: `crates/rogg-core/tests/update_apply.rs`

**Interfaces:**
- Consumes (Task 1): `Install`, `UpdateError`.
- Produces:
  - `verify(file: &Path, sha256_hex: &str) -> Result<(), UpdateError>` (case-insensitive hex)
  - `stage(archive: &Path, dir: &Path, install: &Install) -> Result<PathBuf, UpdateError>`
  - `ReplaceStyle::{Unix, Windows}` and `ReplaceStyle::current()`
  - `apply(staged: &Path, target: &Path, style: ReplaceStyle) -> Result<(), UpdateError>`
  - `#[doc(hidden)] pub enum Fault { None, SwapIn }` and `#[doc(hidden)] pub fn apply_with_fault(staged, target, style, fault: Fault) -> Result<(), UpdateError>`. `apply` calls it with `Fault::None`; `SwapIn` makes the final rename (`.rogg-new` → target) fail, for the rollback test.
  - `move_into(src: &Path, dst: &Path) -> io::Result<()>`, which is rename with a cross-device fallback that copies the tree to `dst`
  - `copy_tree(src: &Path, dst: &Path) -> io::Result<()>`
  - `sweep(target: Option<&Path>, update_dir: &Path)`
  - `can_write(dir: &Path) -> bool`
  - `is_rogg_target(target: &Path) -> bool`

- [ ] **Step 1: Write the failing tests** in `crates/rogg-core/tests/update_apply.rs`. Build archives with `rogg_genlog::bundle::write_archive`.
  - `verify_accepts_the_right_digest_and_rejects_others`: a file holding `b"abc"`. Its digest `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` is `Ok`, the uppercase form is `Ok`, and a wrong digest is `Err(UpdateError::Checksum)`.
  - `stages_each_archive_kind`:
    - a `.tar.gz` with `rogg` and `README.md` for `Binary` returns `<dir>/rogg`;
    - a `.zip` with `rogg.exe` for a `Binary` whose target ends in `rogg.exe` returns `<dir>/rogg.exe`;
    - a `.tar.gz` with `rogg.app/Contents/MacOS/rogg` and `rogg.app/Contents/Info.plist` for `App` returns `<dir>/rogg.app`.
  - `staging_rejects_a_missing_item_and_escaping_entries`:
    - an archive without `rogg` gives `Err(UpdateError::MissingItem("rogg"))`;
    - a tar entry `../evil` gives `Err(UpdateError::Unpack(_))`, and no file `evil` exists outside `dir`.
  - `apply_replaces_a_binary`: target `<tmp>/bin/rogg` holding `b"old"`, staged `<tmp>/stage/rogg` holding `b"new"`.
    - The target now reads `b"new"`.
    - On Unix its mode is `0o755`.
    - No `rogg.rogg-new` or `rogg.rogg-old` is left.
    - The staged file is gone.
  - `apply_replaces_an_app_bundle`: the whole tree is replaced, and a file only the old bundle had is gone.
  - `apply_refuses_a_target_that_is_not_rogg`: target `<tmp>/bin/bash` → `Err(UpdateError::NotRogg(_))`, untouched.
  - `apply_rolls_back_when_the_swap_fails`: `apply_with_fault(staged, target, ReplaceStyle::Unix, Fault::SwapIn)` returns `Err(UpdateError::Replace { rolled_back: true, .. })`. The target still reads `b"old"`, and no `.rogg-old` or `.rogg-new` is left.
  - `windows_style_keeps_the_old_exe_aside`: `ReplaceStyle::Windows` on `<tmp>/rogg.exe` leaves `rogg.exe.old` holding `b"old"`, and `rogg.exe` holds `b"new"`.
  - `move_into_falls_back_to_copying`: `copy_tree` of a nested dir reproduces files and contents. `move_into` from a dir to a destination in a different temp dir works and removes the source.
  - `sweep_removes_leftovers`: `rogg.exe.old`, `rogg.rogg-new`, `rogg.rogg-old` beside the target, and `<update_dir>`, are all gone; the target itself is kept.
  - `can_write_reports_read_only_folders` (Unix): `true` for a temp dir; `false` after `chmod 0o555`.
- [ ] **Step 2: Run them and see them fail**
  - Run: `./dev test core update_apply`
  - Expected: a compile error, because `rogg_core::update::apply` doesn't exist.
- [ ] **Step 3: Implement** `apply.rs` following spec §4.1.
  - `stage` uses `flate2::read::GzDecoder` with `tar::Archive`, or `zip::ZipArchive`.
  - Reject entries whose path has a `ParentDir`, `RootDir` or `Prefix` component.
  - `apply` steps:
    1. `is_rogg_target`.
    2. `move_into(staged, <target>.rogg-new)`.
    3. Set mode 755 on the binary, or on `Contents/MacOS/rogg`.
    4. `rename(target, <target>.rogg-old)`.
    5. `rename(<target>.rogg-new, target)`. On failure, rename `.rogg-old` back and return `Replace { rolled_back, old, source }`.
    6. Delete `.rogg-old`; with `Windows`, rename it to `<target>.old` instead (the running exe can't be deleted).
  - `move_into` treats `io::ErrorKind::CrossesDevices` as the cue to `copy_tree`, then remove the source.
- [ ] **Step 4: Run the tests and see them pass**
  - Run: `./dev test core update_apply`
  - Expected: all pass.
- [ ] **Step 5: Commit** `feat(update): verify, stage and replace with rollback`

### Task 3: hidden `--apply-update` command

**Files:**
- Modify: `crates/rogg-app/src/args.rs`, `crates/rogg-app/src/main.rs`
- Test: `crates/rogg-app/src/args.rs` (unit), `crates/rogg-app/tests/apply_update_cli.rs`

**Interfaces:**
- Consumes (Task 2): `apply`, `ReplaceStyle::current()`, `is_rogg_target`.
- Produces:
  - `Args.apply_update: Option<(PathBuf, PathBuf)>`.
  - `rogg_app::updater::run_apply_update(staged: &Path, target: &Path) -> i32`, which returns 0, 1 or 2. This function lives in `crates/rogg-app/src/updater/mod.rs`; create that module here with only this function and `pub mod updater;` in `lib.rs`.

- [ ] **Step 1: Write the failing tests**
  - Unit test `apply_update_takes_two_paths`:
    - `parse("--apply-update /a/s /b/rogg").apply_update == Some(("/a/s".into(), "/b/rogg".into()))`;
    - `parse("--apply-update /a")` is `Err` (with usage).
  - Integration test `apply_update_cli.rs`: run `env!("CARGO_BIN_EXE_rogg")` with `--apply-update <staged> <tmp>/rogg`. It exits 0 and the target holds the new bytes. With the target `<tmp>/notes.txt` it exits with status 2 and the file is unchanged.
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test app apply_update`
  - Expected: fails, because the field and function don't exist.
- [ ] **Step 3: Implement.**
  - `--apply-update` consumes exactly the next two arguments.
  - `main` handles it before anything else, with no window, and calls `std::process::exit(updater::run_apply_update(..))`.
  - `run_apply_update` returns 2 when `!is_rogg_target(target)`, prints the `UpdateError` to stderr and returns 1 on error, and returns 0 otherwise.
  - Leave `USAGE` unchanged; the flag is hidden.
- [ ] **Step 4: Run and see them pass**
  - Run: `./dev test app apply_update`
  - Expected: pass.
- [ ] **Step 5: Commit** `feat(update): hidden --apply-update command for the elevated helper`

### Task 4: HTTP — manifest fetch and download

**Files:**
- Create: `crates/rogg-app/src/updater/http.rs` (`pub mod http;` in `updater/mod.rs`; Tasks 5 and 6 add `state` and `elevate` the same way)
- Create: `crates/rogg-app/tests/common/http.rs` (a test server shared by later tests; `mod http;` in `tests/common/mod.rs`)
- Modify: `Cargo.toml` (workspace dependencies: `ureq = { version = "=3.4.2", default-features = false, features = ["rustls", "platform-verifier"] }`), `crates/rogg-app/Cargo.toml` (`ureq.workspace = true`, `serde_json.workspace = true` if not present)
- Test: `crates/rogg-app/tests/updater_http.rs`

**Interfaces:**
- Consumes (Task 1): `Manifest::parse`.
- Produces:
  - `pub const DEFAULT_BASE: &str = "https://github.com/rogg-log/rogg/releases/latest/download"`
  - `pub fn base_url() -> String`, which honours `ROGG_UPDATE_BASE`
  - `pub enum NetError { Offline, TimedOut, Http(u16), BadManifest, Cancelled, Io(String) }`
  - `pub fn fetch_manifest(base: &str) -> Result<Manifest, NetError>`, with a 10 s global timeout
  - `pub fn download(url: &str, dest: &Path, on_progress: &mut dyn FnMut(u64, Option<u64>), cancel: &CancelToken) -> Result<(), NetError>`, with a 30 s read timeout and progress about every 64 KiB
  - The test server: `TestServer::start(routes: Vec<(&'static str, Route)>) -> TestServer`, with `server.base() -> String` (`http://127.0.0.1:<port>`), and `Route::{Bytes(u16, Vec<u8>), Stall}`. It reads one request line per connection and answers HTTP/1.1 with `Content-Length` and `Connection: close`.
- TLS: build the agent with `ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier)`.

- [ ] **Step 1: Write the failing tests** in `updater_http.rs`:
  - `fetches_and_parses_the_manifest`: the server serves `/manifest.json` and `fetch_manifest(&server.base())` returns build 14.
  - `maps_http_errors`: a 404 gives `Err(NetError::Http(404))`; invalid JSON gives `Err(NetError::BadManifest)`.
  - `downloads_with_progress`: a 300 000-byte body. The file matches, the progress calls are increasing, and the last one is `(300000, Some(300000))`.
  - `cancel_stops_a_download`: the token is cancelled in the first progress callback; the result is `Err(NetError::Cancelled)`.
  - `a_stalled_server_times_out`: `Route::Stall` accepts and sends nothing. Use a test-only `download_with_timeout(.., Duration::from_millis(300))` (`pub(crate)` with `#[doc(hidden)] pub` re-export) so the test runs fast. The result is `Err(NetError::TimedOut)`.
  - `offline_is_reported`: `fetch_manifest("http://127.0.0.1:1")` gives `Err(NetError::Offline)`.
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test app updater_http`
  - Expected: a compile error.
- [ ] **Step 3: Implement `http.rs`.**
  - Connection-refused and DNS errors map to `Offline`; ureq timeout errors map to `TimedOut`.
  - Write to `dest` with a `.part` suffix, then rename it to `dest` on success.
- [ ] **Step 4: Run and see them pass**
  - Run: `./dev test app updater_http`
  - Expected: pass.
- [ ] **Step 5: Commit** `feat(update): manifest fetch and download over ureq`

### Task 5: `Updater` — state machine and update job

**Files:**
- Modify: `crates/rogg-app/src/updater/mod.rs`
- Create: `crates/rogg-app/src/updater/state.rs`
- Test: `crates/rogg-app/tests/updater.rs`

**Interfaces:**
- Consumes: Task 1 (`Manifest`, `Install`, `asset_name`, `is_newer`), Task 2 (`verify`, `stage`, `apply`, `can_write`, `ReplaceStyle`), Task 4 (`fetch_manifest`, `download`, `NetError`). The admin-rights path is injected as an `Elevator`; Task 6 supplies the real one.
- Produces:
  - `UpdaterConfig { base: String, running_build: Option<u64>, install: Install, os: Os, arch: String, work_dir: PathBuf /* <cache>/update */, elevator: Elevator }`. Task 6 adds the production constructor `from_env`.
  - `pub type Elevator = Arc<dyn Fn(&Path, &Path) -> Result<(), Failure> + Send + Sync>`.
  - `Release { manifest: Manifest, asset: Asset }`.
  - `pub enum UpdateState { Idle, Checking { manual: bool }, UpToDate, LocalBuild, Available(Release), Downloading { release: Release, done: u64, total: Option<u64> }, Installing(Release), Installed(Release), Failed { release: Option<Release>, failure: Failure } }`.
  - `pub enum Failure { Offline, TimedOut, Http(u16), BadManifest, NoAsset, Checksum, Unpack, Declined, NoPkexec, RolledBack, RollbackFailed(PathBuf) }`, with `message(&self) -> String` giving the exact copy from Global Constraints.
  - `pub struct Updater` (a gpui `Global`):
    - `new(config)`
    - `state(&self) -> &UpdateState`
    - `check(&mut self, manual: bool) -> bool`, which returns false when a job is running
    - `start_update(&mut self) -> bool`
    - `cancel(&mut self)`
    - `poll(&mut self) -> bool`, which drains the job channel and returns true while a job is running
    - `badge(&self) -> Option<(SharedString, SharedString)>`, the label and tooltip
  - Jobs run on `std::thread::spawn` and send `JobEvent`s over `std::sync::mpsc`.

- [ ] **Step 1: Write the failing tests** in `updater.rs`. They use `TestServer` from Task 4 and archives from `rogg_genlog`. The helper `config(server, running, target)` builds an `UpdaterConfig` with `Install::Binary { path: <tmp>/bin/rogg }` and an `elevator` that records its calls and returns `Ok(())`. The helper `run(&mut Updater)` loops `poll()` with a 5 ms sleep until it returns false, up to 10 s.
  - `a_newer_build_becomes_available`: running 11, manifest 14. After `check(false)` and `run`, the state is `Available` and `badge() == Some(("Update", "rogg 0.1.0 build 14 is available"))`.
  - `the_same_build_is_up_to_date`: running 14. A manual check gives `UpToDate` and `badge() == None`.
  - `a_local_build_never_checks`: `running_build: None`. `check(false)` stays `Idle` and makes no request (the server counts requests: 0). `check(true)` gives `LocalBuild`.
  - `startup_errors_are_silent_and_manual_ones_are_not`: a 500 manifest. `check(false)` gives `Idle`; `check(true)` gives `Failed { failure: Http(500), .. }`.
  - `a_missing_asset_is_no_update`: the manifest lacks the platform's asset. `check(false)` gives `Idle`; `check(true)` gives `Failed { failure: NoAsset }`.
  - `an_update_replaces_the_installed_binary`: `start_update()` from `Available`.
    - Downloading states are seen with increasing `done`.
    - It ends `Installed`.
    - `<tmp>/bin/rogg` holds the new bytes.
    - `badge()` gives `("Restart to update", "Build 14 is installed. Restart rogg to use it.")`.
    - `work_dir` is empty.
  - `a_bad_checksum_changes_nothing`: the result is `Failed { failure: Checksum }`, the target is unchanged, and the work dir is cleaned.
  - `cancel_returns_to_available`: `Route::Stall` for the asset; `start_update()`, then `cancel()` while `Downloading`. The state becomes `Available`.
  - `an_unwritable_folder_uses_the_elevator` (Unix): `chmod 0o555` on `<tmp>/bin`. The elevator is called once with `(staged, <tmp>/bin/rogg)` and the state is `Installed`.
  - `a_declined_elevation_fails_cleanly`: the elevator returns `Err(Failure::Declined)`. The state is `Failed { failure: Declined }` and `Failure::Declined.message() == "Admin rights were declined."`.
  - `checking_again_after_install_keeps_installed` (Review Focus 1): after `Installed`, `check(true)` leaves `Installed` and the asset request count stays 1.
  - `no_second_job_while_one_runs` (Review Focus 4): during `Downloading`, `start_update()` and `check(true)` both return false and the state is unchanged.
  - `failure_messages_match_the_spec`: one assertion per `Failure` variant against the exact copy, including `Http(404)` → "GitHub returned 404." and `RollbackFailed("/x/rogg.rogg-old")` → "The update failed. The previous version is at /x/rogg.rogg-old.".
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test app updater`
  - Expected: a compile error.
- [ ] **Step 3: Implement** `state.rs` (the types and `Failure::message`) and the `Updater` in `mod.rs`.
  - **Check job:** fetch the manifest, then:
    - the running build or install is unsupported → `NoAsset`;
    - `is_newer` and the asset exists → `Available`;
    - otherwise → `UpToDate`.
  - **Silence:** when `manual` is false, any failure or `UpToDate` maps to `Idle`.
  - **Update job:**
    1. `download` to `work_dir/<build>/<asset>`.
    2. `verify`.
    3. `stage` into `work_dir/<build>/staged`.
    4. If `can_write(target.parent())`, run `apply(staged, target, ReplaceStyle::current())`; otherwise call the elevator.
    5. Remove `work_dir/<build>`.
  - Map `UpdateError::Checksum` → `Checksum`; `Unpack` and `MissingItem` → `Unpack`; `Replace { rolled_back: true }` → `RolledBack`; `Replace { rolled_back: false, old }` → `RollbackFailed(old)`.
  - **Rule:** in `Installed`, a check only refreshes the manifest and never leaves `Installed`.
- [ ] **Step 4: Run and see them pass**
  - Run: `./dev test app updater`
  - Expected: pass.
- [ ] **Step 5: Commit** `feat(update): updater state machine, check and update jobs`

### Task 6: elevation launchers

**Files:**
- Create: `crates/rogg-app/src/updater/elevate.rs`
- Modify: `Cargo.toml` (add `"Win32_UI_Shell", "Win32_UI_WindowsAndMessaging"` to the workspace `windows-sys` features), `crates/rogg-app/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys.workspace = true`)
- Test: unit tests in `elevate.rs`

**Interfaces:**
- Consumes (Task 5): `Failure`.
- Produces:
  - `UpdaterConfig::from_env(cache: &Path) -> UpdaterConfig`:
    - `base` is `http::base_url()`;
    - `running_build` is `about::build_info().number` parsed as `u64`;
    - `install` is `Install::detect(current_exe, Os::current())`;
    - `arch` is `std::env::consts::ARCH`;
    - `work_dir` is `<cache>/update`;
    - `elevator` is `Arc::new(apply_elevated)`.
  - `pub fn apply_elevated(staged: &Path, target: &Path) -> Result<(), Failure>`
  - `pub fn applescript(exe: &Path, staged: &Path, target: &Path) -> String`
  - `pub fn pkexec_argv(exe: &Path, staged: &Path, target: &Path) -> Vec<OsString>`
  - `pub fn map_status(os: Os, status: Option<i32>, stderr: &str) -> Result<(), Failure>`

- [ ] **Step 1: Write the failing unit tests:**
  - `applescript_quotes_every_path`: with paths containing a space and a `'`, the result is exactly:

    ```
    do shell script quoted form of "/Apps/r o g g.app/Contents/MacOS/rogg" & " --apply-update " & quoted form of "/tmp/it's/rogg.app" & " " & quoted form of "/Apps/r o g g.app" with administrator privileges
    ```

    AppleScript string literals escape `\` and `"` with a backslash.
  - `pkexec_runs_the_hidden_command`: `[exe, "--apply-update", staged, target]`.
  - Status mapping:
    - Mac `Some(0)` → `Ok`;
    - Mac `Some(1)` with stderr containing `(-128)` → `Declined`;
    - Linux `Some(126)` and `Some(127)` → `Declined`;
    - helper `Some(2)` on any OS → `RolledBack`;
    - helper `Some(1)` → `RolledBack`.
  - `missing_pkexec_is_reported` (Linux only, `cfg(target_os = "linux")`): with `PATH` pointing at an empty temp dir, `apply_elevated` gives `Err(Failure::NoPkexec)`.
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test app elevate`
  - Expected: a compile error.
- [ ] **Step 3: Implement.**
  - **macOS:** `Command::new("/usr/bin/osascript").args(["-e", &applescript(..)])`, waited, then `map_status`.
  - **Linux:** look up `pkexec` on `PATH`; if it's missing, return `NoPkexec`. Otherwise run it and call `map_status`.
  - **Windows:**
    1. `ShellExecuteExW` with `lpVerb = "runas"`, `lpFile = exe`, `lpParameters = --apply-update "<staged>" "<target>"` (quoted), `fMask = SEE_MASK_NOCLOSEPROCESS`, `nShow = SW_HIDE`.
    2. If it fails with `ERROR_CANCELLED` (1223), return `Declined`.
    3. Otherwise run `WaitForSingleObject` and `GetExitCodeProcess`, then `map_status`.
  - `exe` is always `std::env::current_exe()`.
  - Add `UpdaterConfig::from_env` as listed under Produces, with a unit test `from_env_reads_the_build_and_install`: `work_dir == <cache>/update`, and `running_build` is `None` in a test build (no `ROGG_BUILD_NUMBER`).
- [ ] **Step 4: Run and see them pass,** then check the Windows build.
  - Run: `./dev test app elevate && ./dev windows-build`
  - Expected: the tests pass, and the Windows cross-build compiles.
- [ ] **Step 5: Commit** `feat(update): admin-rights launchers for macOS, Linux and Windows`

### Task 7: UI — badge and Update dialog view

**Files:**
- Modify: `crates/rogg-ui/src/titlebar.rs`, `crates/rogg-ui/src/dialogs.rs`
- Test: `crates/rogg-ui/tests/titlebar.rs`, `crates/rogg-ui/tests/dialogs.rs`

**Interfaces:**
- Produces:
  - `TitleBar::update_badge(label: impl Into<SharedString>, tooltip: impl Into<SharedString>, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self`. This renders the element `update-badge` as the first child of `title-actions`: an accent-coloured pill with 10px text and the same height as a compact icon button.
  - `pub struct UpdateView { pub title: SharedString /* "Software update" */, pub lines: Vec<SharedString>, pub progress: Option<(f32, SharedString)>, pub buttons: Vec<UpdateButton> }`.
  - `pub struct UpdateButton { pub id: &'static str, pub label: SharedString, pub primary: bool, pub on_click: Callback }`.
  - `pub fn update(view: UpdateView) -> DialogFrame`. Lines render as paragraphs, progress uses the existing progress-bar style from `chrome::bar_fraction`, and buttons are right-aligned `ActionButton`s.

- [ ] **Step 1: Write the failing tests:**
  - `the_update_badge_sits_first_in_the_actions_and_reports_clicks`:
    - `update-badge` exists, and its left edge is left of `all-tabs`;
    - its height equals the height of `all-tabs`;
    - clicking it fires the callback;
    - without `update_badge`, `update-badge` doesn't exist.
  - `the_update_dialog_renders_lines_progress_and_buttons`: a view with 2 lines, progress `(0.5, "15 of 30 MB")` and buttons `update-now` (primary) and `update-later`.
    - `update-progress` exists;
    - both button ids exist;
    - clicking `update-now` fires its callback.
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test ui titlebar dialogs`
  - Expected: a compile error.
- [ ] **Step 3: Implement** both components, following the existing `IconButton` and `ActionButton` styles.
- [ ] **Step 4: Run and see them pass,** then check visuals.
  - Run: `./dev test ui && ./dev visual-check`
  - Expected: the tests pass, and the goldens are 0.000% (the replica sets no badge).
- [ ] **Step 5: Commit** `feat(ui): update badge and update dialog view`

### Task 8: app wiring — startup check, badge, dialog, menu, setting

**Files:**
- Modify:
  - `crates/rogg-core/src/settings.rs` (`BehaviorSettings.check_updates`, serde default `true`)
  - `crates/rogg-app/src/dialogs/settings.rs` (toggle `setting-check-updates`, row label `Check for updates on start`)
  - `crates/rogg-app/src/actions.rs` (`CheckForUpdates`)
  - `crates/rogg-app/src/menus.rs` (the item below `About rogg`)
  - `crates/rogg-app/src/lib.rs` (`install_globals` sets `Updater::new(UpdaterConfig::from_env(&paths.cache))` unless one is already set, and calls `update::apply::sweep(install.target(), &work_dir)`)
  - `crates/rogg-app/src/workspace/mod.rs` (`Dialog::Update`, badge, startup check, the `CheckForUpdates` handler, polling, `open_url` recording)
- Create: `crates/rogg-app/src/updater/view.rs` (`UpdateState` → `UpdateView`)
- Test: `crates/rogg-app/tests/update_ui.rs`; update the menu unit tests in `menus.rs`

**Interfaces:**
- Consumes: Task 5 `Updater` and its methods; Task 6 `UpdaterConfig::from_env`; Task 7 `update_badge`, `UpdateView`, `dialogs::update`.
- Produces:
  - `Workspace::check_for_updates(&mut self, manual: bool, window, cx)`, which starts the check, opens `Dialog::Update` when `manual`, and starts polling.
  - `Workspace::last_opened_url() -> Option<&str>`.
  - `pub open_urls: bool`, which defaults to true; tests set it false, like `reveal_in_finder`.
  - `updater::view::update_view(state: &UpdateState, running: Option<u64>, actions: ViewActions) -> UpdateView`, where `ViewActions` holds the callbacks for update, cancel, retry, release notes, copy command and close.

- [ ] **Step 1: Write the failing tests:**
  - **In `update_ui.rs`:** set a test `Updater` global (Task 5 helpers with `TestServer`) before `common::open_workspace`; the Updater starts with `running_build: Some(11)`.
    - `the_startup_check_shows_the_badge`: the setting is on. Advance the clock 3 s and `wait_until` the badge `update-badge` exists. Clicking it opens the dialog showing the exact text "rogg 0.1.0 · build 14 · 2026-10-18 · a1b2c3d" and "You have build 11.".
    - `the_setting_turns_the_startup_check_off`: `check_updates = false`. After advancing 5 s, there's no badge and the server request count is 0.
    - `update_then_restart_badge`: click `update-now`; `wait_until` the badge reads "Restart to update". The dialog text is "Build 14 is installed. Restart rogg to use it." and the only button is `update-close`.
    - `closing_the_dialog_keeps_the_download_running`: click `update-now`, then press `escape` (Dismiss closes the dialog; Downloading has no Close button). `wait_until` the badge reads "Restart to update". No dialog is open, and the target holds the new bytes.
    - `release_notes_opens_the_release_page`: `last_opened_url() == Some("https://github.com/rogg-log/rogg/releases/tag/v0.1.0-build.14")`.
    - `check_for_updates_from_the_menu_when_up_to_date`: the server's manifest is build 11. Dispatch `CheckForUpdates`; the dialog shows "rogg is up to date (build 11)." and no badge.
    - `a_local_build_says_so`: `running_build: None` and `CheckForUpdates` → "This is a local build. Updates apply to release builds only.".
    - `copy_install_command_on_missing_pkexec`: the elevator returns `NoPkexec`. `update-copy-command` exists, and clicking it puts `curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash` on the clipboard.
  - **Menu unit tests:** in the native "rogg" menu, items 0 and 1 are `About rogg` and `Check for Updates…`, and the item-count assertion becomes 10. The ☰ popup's first two labels are `About rogg` and `Check for Updates…`.
  - **Settings:** `BehaviorSettings::default().check_updates == true`. A settings TOML without the key loads with `true`. In the Settings dialog, `setting-check-updates` exists, and clicking it sets `check_updates` to `false`.
- [ ] **Step 2: Run and see them fail**
  - Run: `./dev test app update_ui && ./dev test app menus && ./dev test core settings`
  - Expected: failures, from compile errors and the old menu count.
- [ ] **Step 3: Implement the wiring.**
  - **Startup:** `Workspace::new` spawns a 3 s timer, then `check_for_updates(false)` when `SettingsStore::get(cx).behavior.check_updates`.
  - **Polling:** a polling loop (as in `ensure_tree_polling`) calls `Updater::poll` and `cx.notify()` until it returns false.
  - **Badge:** the title bar gets `update_badge` from `Updater::badge()`; its click opens `Dialog::Update`.
  - **Dialog:** `dialog_element` renders `dialogs::update(update_view(..))`.
  - **Buttons:**
    - **Release notes** calls `open_url` → `cx.open_url` (when `open_urls`) and records `last_opened_url`.
    - **Copy install command** writes the clipboard.
    - **Later** and **Close** close the dialog only.
  - **Button ids:** `update-now`, `update-notes`, `update-later`, `update-cancel`, `update-retry`, `update-close`, `update-copy-command`.
- [ ] **Step 4: Run and see them pass,** then the full gate.
  - Run: `./dev check`
  - Expected: all checks passed, and the goldens are 0.000%.
- [ ] **Step 5: Commit** `feat(update): startup check, title-bar badge, update dialog and menu item`

### Task 9: manual-checks note, README, full verification

**Files:**
- Create: `docs/superpowers/notes/2026-10-19-auto-update-manual-checks.md` (the checklist from spec §6.2, one checkbox per item)
- Modify: `README.md` (one line under Install: "rogg checks for new builds at startup and can update itself: click **Update** in the title bar, or use **Check for Updates…** in the app menu.")

- [ ] **Step 1: Write the note and the README line.**
- [ ] **Step 2: Run the cross-platform gate**
  - Run: `./dev check-all`
  - Expected: `check` passes, `linux-test` passes in the container, and `windows-build` compiles.
- [ ] **Step 3: Capture screenshots** of the badge (Available and Installed) and of each dialog state, in dark and light, for the user's approval. Use a throwaway capture hook as in earlier work; don't commit it.
- [ ] **Step 4: Commit** `docs: auto-update manual checks and README note`
