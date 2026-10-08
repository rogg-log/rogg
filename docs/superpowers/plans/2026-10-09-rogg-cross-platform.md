# rogg Linux and Windows Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and run rogg on Linux and Windows as well as macOS. It keeps rogg's look and follows each platform's conventions. Linux is verified in an Apple `container` VM; Windows is cross-compiled from macOS.

**Architecture:**
- **OS layer:** a gpui-free `rogg_core::platform` module holds every OS-specific call behind `cfg(target_os)`: data folders, process check, stdin/stream detection.
- **Chrome style:** `rogg_ui::theme::ChromeStyle { Mac, Linux, Windows }` is a runtime value, defaulting to the build OS and overridable with `ROGG_CHROME`. It drives the title bar, the window controls, the ☰ menu, the shortcut labels and the font pair. This makes the Linux and Windows chrome testable and capturable on macOS.
- **Keys:** they use gpui's `secondary-` modifier (⌘ on macOS, Ctrl elsewhere).
- **Verification:** new `./dev` commands drive a Debian container for Linux build, test and screenshots, and cargo-xwin for the Windows cross-build.

**Tech Stack:**
- Rust 1.99.0 (edition 2024).
- gpui-pre 0.3.8 through gpui-kit `=0.7.1`, with its Linux (X11/Wayland, wgpu) and Windows backends.
- New dependency: `windows-sys` (Windows target only).
- Bundled fonts: Inter and JetBrains Mono (SIL OFL 1.1).
- Apple `container` 1.1.0, Debian bookworm arm64, Xvfb, Mesa lavapipe, ImageMagick, xdotool.
- cargo-xwin.

**Spec:** `docs/superpowers/specs/2026-10-09-rogg-cross-platform-design.md`

## Global Constraints

- **macOS stays unchanged** in behaviour and look. `./dev check` is green on macOS after every task, and the replica goldens (`tools/visual-diff/golden`) are never re-approved by this plan.
- **Folder rules (spec §3):**
  - macOS: everything in `~/Library/Application Support/rogg`; the cache in `~/Library/Caches/rogg`.
  - Linux: config (settings, profiles) in `$XDG_CONFIG_HOME/rogg`, else `~/.config/rogg`; data (history, recent, session) in `$XDG_DATA_HOME/rogg`, else `~/.local/share/rogg`; the cache in `$XDG_CACHE_HOME/rogg`, else `~/.cache/rogg`.
  - Windows: config and data in `%APPDATA%\rogg`; the cache in `%LOCALAPPDATA%\rogg\cache`; home is `%USERPROFILE%`.
  - Unset or empty variables fall back as listed and never become cwd-relative. The final fallback is the temp dir.
- **ChromeStyle:** `ChromeStyle { Mac, Linux, Windows }`, defaulting to the build OS, overridden by `ROGG_CHROME=mac|linux|windows`; the gallery takes `--chrome mac|linux|windows`.
- **Window controls (Linux and Windows):** ids `window-minimize`, `window-maximize`, `window-close`.
  - Windows: 46×42 px rectangles, no gap; close hover `#c42b1c` with white glyphs.
  - Linux: round 24 px buttons with 8 px gaps.
  - The ☰ button has id `app-menu`.
- **Shortcut labels:**
  - Mac: `⌘ O`, `⌘ ↑ / ⌘ ↓`, `⌥ ⌘ C` (as today).
  - Linux and Windows: `Ctrl+O`, `Ctrl+↑ / Ctrl+↓`, `Ctrl+Alt+C`, with the modifier order Ctrl, Alt, Shift.
- **macOS-only bindings:** ⌘H Hide, ⌥⌘H Hide Others and ⌘M Minimise. Linux gets `ctrl-q` to quit.
- **Fonts:**
  - Mac keeps `.SystemUIFont` / `.AppleSystemUIFontMonospaced`.
  - Linux and Windows use the bundled `Inter` (Regular, Medium, SemiBold) and `JetBrains Mono` (Regular, Bold), SIL OFL 1.1. Their licences are added to About's credits.
- **Builds:** Linux builds use `CARGO_TARGET_DIR=target/linux`; the Windows target is `x86_64-pc-windows-msvc`.
- **Gated tests:** the CoreText real-text harness, the gallery `--capture` tests and the release memory test run on macOS only (`cfg(target_os = "macos")`).
- **Conventions:**
  - Import gpui as `gpui_kit`; `rogg-core` never depends on gpui.
  - Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  - Nothing is installed system-wide without asking the user first (cargo-xwin, the rustup target, container images).

## Review Focus

1. **Moving an existing user's data:** a macOS user upgrading must find their settings, profiles, history and session exactly where they were. Task 1: `macos_folders_are_unchanged`.
2. **A Linux user with a relative `XDG_*` variable:** per the XDG spec, relative values are ignored, never used cwd-relative. Task 1: `relative_xdg_values_are_ignored`.
3. **A settings file written on macOS and opened on Linux:** `log_font = "SF Mono"`, the default token, must mean "the platform's default mono font", not a missing font. Task 2: `the_default_log_font_token_maps_to_the_platform_mono`.
4. **A label with two chords or a range:** `⌘ ↑ / ⌘ ↓`, `⌘ 1…9` and `⇧ ⌘ T` must convert to `Ctrl+↑ / Ctrl+↓`, `Ctrl+1…9` and `Ctrl+Shift+T`. Task 3: `labels_convert_chords_ranges_and_modifier_order`.
5. **The ☰ menu and the native menu stay in sync:** an item added to `app_menus` must appear in the ☰ menu. Task 4: `the_app_menu_mirrors_the_native_menus`.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/rogg-core/src/platform.rs` | create | `Os`, `Env`, the folder rules, `process_alive`, `stdin_has_data`, `is_stream_path` |
| `crates/rogg-core/src/paths.rs` | modify | use the platform folder rules (config, data and cache bases) |
| `crates/rogg-core/src/{stream,cache}.rs` | modify | delegate the OS calls to `platform`; `fd_has_data` stays `cfg(unix)` |
| `crates/rogg-core/Cargo.toml`, `Cargo.toml` | modify | `windows-sys` for the Windows target |
| `crates/rogg-app/build.rs` | modify | the build date computed in Rust |
| `crates/rogg-ui/src/theme/{mod,chrome}.rs` | modify/create | `ChromeStyle`; the font pair per style |
| `crates/rogg-ui/src/fonts.rs`, `crates/rogg-ui/assets/fonts/*` | create | the bundled fonts and their registration |
| `crates/rogg-ui/src/keys.rs` | create | `shortcut_label(mac_label, chrome)` |
| `crates/rogg-ui/src/titlebar.rs` | modify | the per-style layout, window controls and ☰ button |
| `crates/rogg-ui/src/layout.rs` | modify | resize edges for client-side decorations |
| `crates/rogg-app/src/{actions,menus,window,settings,lib}.rs` | modify | `secondary-` keys, platform-only bindings, labels, the ☰ menu, window options, the default font token, font registration |
| `crates/rogg-app/src/workspace/{mod,keys}.rs` | modify | wiring the title bar menu and the window actions |
| `crates/rogg-app/src/{about.rs,dialogs/profiles.rs}` | modify | real folder paths in UI text; font credits |
| `crates/rogg-gallery/src/{main,capture}.rs` | modify | `--chrome` |
| `crates/rogg-ui/tests/logview_real_text.rs`, `crates/rogg-gallery/tests/capture.rs`, `crates/rogg-core/tests/memory.rs` | modify | `cfg(target_os = "macos")` gating |
| `tools/linux/Containerfile`, `tools/linux/shot.sh` | create | the Linux image; the screenshot driver |
| `dev`, `justfile` | modify | `linux-image`, `linux-build`, `linux-test`, `linux-shot`, `linux-shell`, `windows-build`, `check-all`; the setup offers |
| `README.md`, `docs/superpowers/specs/2026-10-06-rogg-design.md` | modify | platform docs |

---

### Task 1: OS layer: folders, process check, stdin and streams, build date

**Files:**
- Create: `crates/rogg-core/src/platform.rs`
- Modify:
  - `crates/rogg-core/src/{lib.rs,paths.rs,stream.rs,cache.rs}`;
  - `crates/rogg-core/Cargo.toml`, `Cargo.toml` (workspace deps);
  - `crates/rogg-app/build.rs`;
  - `crates/rogg-app/src/args.rs` (test gating unchanged, already `cfg(unix)`);
  - `crates/rogg-app/src/dialogs/profiles.rs` (the note text).
- Test: unit tests in `platform.rs`, `paths.rs` and `build.rs`'s helper; `crates/rogg-core/tests/stream.rs` (gate the unix-only cases).

**Interfaces:**
- Produces:
  - `rogg_core::platform::{Os, Env, config_base(Os, &Env) -> PathBuf, data_base(Os, &Env) -> PathBuf, cache_base(Os, &Env) -> PathBuf, home(Os, &Env) -> PathBuf, process_alive(u32) -> bool, stdin_has_data() -> bool, is_stream_path(&Path) -> bool}`;
  - `Os::current()`;
  - `Env::from_process()`.
- The `rogg_core::paths::*` public functions keep their names: `home_dir`, `app_support_dir` (now the config base, kept for callers), `cache_dir`, `history_path`, `recent_path`, `settings_path`, `profiles_dir`, `session_path`.
- New: `paths::config_dir()` and `paths::data_dir()`.

- [ ] **Step 1: Write the failing tests**

At the end of `crates/rogg-core/src/platform.rs` (create the file with the tests first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn env(pairs: &[(&str, &str)]) -> Env {
        let get = |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| std::ffi::OsString::from(*v));
        Env {
            home: get("HOME"),
            userprofile: get("USERPROFILE"),
            xdg_config_home: get("XDG_CONFIG_HOME"),
            xdg_data_home: get("XDG_DATA_HOME"),
            xdg_cache_home: get("XDG_CACHE_HOME"),
            appdata: get("APPDATA"),
            localappdata: get("LOCALAPPDATA"),
        }
    }

    #[test]
    fn macos_folders_are_unchanged() {
        let e = env(&[("HOME", "/Users/u")]);
        assert_eq!(config_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Application Support/rogg"));
        assert_eq!(data_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Application Support/rogg"));
        assert_eq!(cache_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Caches/rogg"));
    }

    #[test]
    fn linux_follows_xdg_with_home_fallbacks() {
        let e = env(&[("HOME", "/home/u")]);
        assert_eq!(config_base(Os::Linux, &e), PathBuf::from("/home/u/.config/rogg"));
        assert_eq!(data_base(Os::Linux, &e), PathBuf::from("/home/u/.local/share/rogg"));
        assert_eq!(cache_base(Os::Linux, &e), PathBuf::from("/home/u/.cache/rogg"));
        let x = env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/cfg"), ("XDG_DATA_HOME", "/data"), ("XDG_CACHE_HOME", "/cache")]);
        assert_eq!(config_base(Os::Linux, &x), PathBuf::from("/cfg/rogg"));
        assert_eq!(data_base(Os::Linux, &x), PathBuf::from("/data/rogg"));
        assert_eq!(cache_base(Os::Linux, &x), PathBuf::from("/cache/rogg"));
    }

    #[test]
    fn relative_xdg_values_are_ignored() {
        let e = env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "rel/cfg"), ("XDG_DATA_HOME", ""), ("XDG_CACHE_HOME", "./c")]);
        assert_eq!(config_base(Os::Linux, &e), PathBuf::from("/home/u/.config/rogg"));
        assert_eq!(data_base(Os::Linux, &e), PathBuf::from("/home/u/.local/share/rogg"));
        assert_eq!(cache_base(Os::Linux, &e), PathBuf::from("/home/u/.cache/rogg"));
    }

    #[test]
    fn windows_uses_appdata_and_localappdata() {
        let e = env(&[("USERPROFILE", r"C:\Users\u"), ("APPDATA", r"C:\Users\u\AppData\Roaming"), ("LOCALAPPDATA", r"C:\Users\u\AppData\Local")]);
        assert_eq!(config_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Roaming").join("rogg"));
        assert_eq!(data_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Roaming").join("rogg"));
        assert_eq!(cache_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Local").join("rogg").join("cache"));
        assert_eq!(home(Os::Windows, &e), PathBuf::from(r"C:\Users\u"));
        let bare = env(&[("USERPROFILE", r"C:\Users\u")]);
        assert_eq!(config_base(Os::Windows, &bare), PathBuf::from(r"C:\Users\u").join("AppData").join("Roaming").join("rogg"));
        assert_eq!(cache_base(Os::Windows, &bare), PathBuf::from(r"C:\Users\u").join("AppData").join("Local").join("rogg").join("cache"));
    }

    #[test]
    fn no_home_falls_back_to_the_temp_dir_never_cwd() {
        let e = env(&[]);
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for p in [config_base(os, &e), data_base(os, &e), cache_base(os, &e)] {
                assert!(p.is_absolute(), "{os:?}: {p:?}");
                assert!(p.starts_with(std::env::temp_dir()), "{os:?}: {p:?}");
            }
        }
    }

    #[test]
    fn this_process_is_alive_and_a_bogus_pid_is_not() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(0));
        assert!(!process_alive(u32::MAX - 7));
    }
}
```

In `crates/rogg-core/tests/stream.rs`, add `#![cfg(unix)]` at the top of the file if all its cases use pipes/FIFOs (they do: `mkfifo`, raw fds). `StreamCapture` reading from a file, which works everywhere, is already covered by other tests.

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test core platform`
Expected: compile errors (no `platform` module).

- [ ] **Step 3: Implement `platform.rs`**

```rust
//! Everything that differs between operating systems (spec §3): data folders, whether a process is alive, whether
//! stdin carries data, and stream paths. The folder rules are pure functions of `Os` and `Env`, so every platform's
//! rules are tested on every platform.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    /// The OS this binary was built for (other Unixes use the Linux rules).
    pub const fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// The environment variables the folder rules read.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub home: Option<OsString>,
    pub userprofile: Option<OsString>,
    pub xdg_config_home: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    pub xdg_cache_home: Option<OsString>,
    pub appdata: Option<OsString>,
    pub localappdata: Option<OsString>,
}

impl Env {
    pub fn from_process() -> Env {
        let get = |k: &str| std::env::var_os(k);
        Env {
            home: get("HOME"),
            userprofile: get("USERPROFILE"),
            xdg_config_home: get("XDG_CONFIG_HOME"),
            xdg_data_home: get("XDG_DATA_HOME"),
            xdg_cache_home: get("XDG_CACHE_HOME"),
            appdata: get("APPDATA"),
            localappdata: get("LOCALAPPDATA"),
        }
    }
}

const APP: &str = "rogg";

/// A set, non-empty, absolute path from `value` (XDG ignores relative values; empty means unset).
fn absolute(value: &Option<OsString>) -> Option<PathBuf> {
    value.as_ref().filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// The user's home folder; the temp dir when unknown, so paths never become cwd-relative.
pub fn home(os: Os, env: &Env) -> PathBuf {
    let var = if os == Os::Windows { &env.userprofile } else { &env.home };
    absolute(var).unwrap_or_else(std::env::temp_dir)
}

/// Settings and profiles.
pub fn config_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => home(os, env).join("Library/Application Support").join(APP),
        Os::Linux => absolute(&env.xdg_config_home).unwrap_or_else(|| home(os, env).join(".config")).join(APP),
        Os::Windows => roaming(env).join(APP),
    }
}

/// History, recent files and the session.
pub fn data_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => config_base(os, env),
        Os::Linux => absolute(&env.xdg_data_home).unwrap_or_else(|| home(os, env).join(".local").join("share")).join(APP),
        Os::Windows => roaming(env).join(APP),
    }
}

/// Conversions, extracted members, stream captures.
pub fn cache_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => home(os, env).join("Library/Caches").join(APP),
        Os::Linux => absolute(&env.xdg_cache_home).unwrap_or_else(|| home(os, env).join(".cache")).join(APP),
        Os::Windows => absolute(&env.localappdata).unwrap_or_else(|| home(os, env).join("AppData").join("Local")).join(APP).join("cache"),
    }
}

fn roaming(env: &Env) -> PathBuf {
    absolute(&env.appdata).unwrap_or_else(|| home(Os::Windows, env).join("AppData").join("Roaming"))
}

/// Whether process `pid` exists (a process we may not signal still counts). Used to keep a running instance's cache
/// files during cleanup.
pub fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    imp::process_alive(pid)
}

/// stdin carries data: a pipe, FIFO, socket or regular file — not a terminal or a null device.
pub fn stdin_has_data() -> bool {
    imp::stdin_has_data()
}

/// A FIFO or socket on disk: read it as a stream, never map it. Windows has neither, so it is always false there.
pub fn is_stream_path(path: &Path) -> bool {
    imp::is_stream_path(path)
}

#[cfg(unix)]
mod imp {
    use std::os::unix::fs::FileTypeExt;
    use std::path::Path;

    pub fn process_alive(pid: u32) -> bool {
        let Ok(pid) = i32::try_from(pid) else { return false };
        // SAFETY: signal 0 delivers nothing; it only checks that the process exists.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        alive || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    pub fn stdin_has_data() -> bool {
        crate::stream::fd_has_data(libc::STDIN_FILENO)
    }

    pub fn is_stream_path(path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|m| m.file_type().is_fifo() || m.file_type().is_socket())
    }
}

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    pub fn process_alive(pid: u32) -> bool {
        // SAFETY: plain Win32 calls; the handle is closed before returning.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // Access denied still means the process exists.
                return std::io::Error::last_os_error().raw_os_error() == Some(5);
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code == STILL_ACTIVE as u32
        }
    }

    pub fn stdin_has_data() -> bool {
        // SAFETY: GetStdHandle/GetFileType only read the handle table.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() {
                return false;
            }
            matches!(GetFileType(handle), FILE_TYPE_PIPE | FILE_TYPE_DISK)
        }
    }

    pub fn is_stream_path(_: &Path) -> bool {
        false
    }
}
```

(`windows-sys` 0.59+ uses raw pointers for `HANDLE`; if the pinned version uses `isize`, compare with `0` instead of `is_null()`. Task 7 compiles this for Windows and fixes such details.)

`crates/rogg-core/src/lib.rs`: add `pub mod platform;`.

Workspace `Cargo.toml` `[workspace.dependencies]`:

```toml
windows-sys = { version = "=0.59.0", features = ["Win32_Foundation", "Win32_System_Threading", "Win32_System_Console", "Win32_Storage_FileSystem"] }
```

`crates/rogg-core/Cargo.toml`:
- move `libc` under `[target.'cfg(unix)'.dependencies]`;
- add:

```toml
[target.'cfg(windows)'.dependencies]
windows-sys.workspace = true
```

(Use the newest `windows-sys` already in `Cargo.lock` if one is present: `grep -A1 'name = "windows-sys"' Cargo.lock`. Pin it exactly, and adjust the code to its API.)

- [ ] **Step 4: Route paths, stream and cache through it**

`crates/rogg-core/src/paths.rs`:
- the module doc becomes "Where rogg keeps its data on each OS (spec §3)";
- `home_from` / `home_dir` delegate to `platform::home(Os::current(), &Env::from_process())`;
- keep the existing `empty_or_unset_home_falls_back_to_temp_dir` test by moving its intent onto `platform::home`;
- then:

```rust
use crate::platform::{self, Env, Os};

fn env() -> Env {
    Env::from_process()
}

/// Settings and profiles (`~/Library/Application Support/rogg`, `~/.config/rogg`, `%APPDATA%\rogg`).
pub fn config_dir() -> PathBuf {
    platform::config_base(Os::current(), &env())
}

/// History, recent files and the session.
pub fn data_dir() -> PathBuf {
    platform::data_base(Os::current(), &env())
}

/// Kept for callers: the config folder.
pub fn app_support_dir() -> PathBuf {
    config_dir()
}

pub fn cache_dir() -> PathBuf {
    platform::cache_base(Os::current(), &env())
}

pub fn history_path() -> PathBuf { data_dir().join("history.json") }
pub fn recent_path() -> PathBuf { data_dir().join("recent.json") }
pub fn session_path() -> PathBuf { data_dir().join("session.json") }
pub fn settings_path() -> PathBuf { config_dir().join("settings.toml") }
pub fn profiles_dir() -> PathBuf { config_dir().join("profiles") }
```

Gate the existing `macos_locations` test with `#[cfg(target_os = "macos")]`. Add a Linux twin under `#[cfg(target_os = "linux")]` asserting `.config/rogg` / `.local/share/rogg` endings only when `XDG_*` is unset (skip otherwise).

`crates/rogg-core/src/stream.rs`:
- `fd_has_data` and its `RawFd` / `libc` imports stay, under `#[cfg(unix)]`;
- `stdin_has_data()` becomes `crate::platform::stdin_has_data()`;
- `is_stream_path` becomes `crate::platform::is_stream_path(path)`;
- remove the now-unused `std::os::unix::fs::FileTypeExt` import.

`crates/rogg-core/src/cache.rs`:
- delete `process_alive(pid: i32)` and use `crate::platform::process_alive`;
- `stream_owner` / the pid parser return `u32` (parse as `u32`);
- adjust the call sites and existing tests (pids are positive).

`crates/rogg-app/src/dialogs/profiles.rs` (~line 625): the note becomes `format!("Profiles are saved in {}.", paths.profiles.display())`, using the `AppPaths` global (as the About dialog does), with no hard-coded `~/Library`.

- [ ] **Step 5: Build date without `date`**

In `crates/rogg-app/build.rs`, replace `output("date", &["+%Y-%m-%d"])` with `Some(today())`:

```rust
/// Today's date (UTC) as `YYYY-MM-DD`, from the system clock — no external `date` command (Windows has none).
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 → (year, month, day), Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
```

A build script can't host unit tests easily. Verify `civil_from_days` with a temporary scratch check (`0 → (1970,1,1)`, `19_000 → (2022,1,8)`, `20_734 → (2026,10,9)`), then remove the scratch. Note the check in the report. The doc comment's "no `date`" mention changes accordingly.

- [ ] **Step 6: Run the tests**

Run: `./dev test core`, `./dev test app`, `./dev check`.
Expected: PASS. macOS paths are unchanged, so the About, profile and app tests keep their expectations.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(core): platform layer for folders, process check and stdin; build date without date(1)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 2: `ChromeStyle`, bundled fonts, and the default log font

**Files:**
- Create:
  - `crates/rogg-ui/src/theme/chrome.rs`, `crates/rogg-ui/src/fonts.rs`;
  - `crates/rogg-ui/assets/fonts/{Inter-Regular,Inter-Medium,Inter-SemiBold,JetBrainsMono-Regular,JetBrainsMono-Bold}.ttf`;
  - `crates/rogg-ui/assets/fonts/{Inter-OFL.txt,JetBrainsMono-OFL.txt}`.
- Modify:
  - `crates/rogg-ui/src/{theme/mod.rs,assets.rs,lib.rs}`;
  - `crates/rogg-app/src/{lib.rs,settings.rs,about.rs}`;
  - `crates/rogg-gallery/src/{main.rs,capture.rs}` (register fonts; `--chrome`).
- Test: `crates/rogg-ui/src/theme/chrome.rs` (unit), `crates/rogg-ui/tests/fonts.rs` (new), `crates/rogg-app/src/settings.rs` (unit).

**Interfaces:**
- Produces:
  - `rogg_ui::theme::ChromeStyle::{Mac, Linux, Windows}`, with `current()`, `from_env_value(&str) -> Option<ChromeStyle>`, `resolved() -> ChromeStyle` (env override, else current), `is_mac()`;
  - the `RoggTheme.chrome` field;
  - `RoggTheme::with_chrome(ChromeStyle) -> RoggTheme` (sets `chrome` and the font pair);
  - `rogg_ui::theme::{INTER, JETBRAINS_MONO}`;
  - `rogg_ui::fonts::register(cx: &mut App)`;
  - `rogg_app::settings::default_log_font_label(ChromeStyle) -> &'static str`.

- [ ] **Step 1: Fetch the fonts**

Download the official releases into `crates/rogg-ui/assets/fonts/`:
- Inter 4.x static TTFs: Regular, Medium, SemiBold, from the Inter GitHub release zip, `extras/ttf`;
- JetBrains Mono 2.304 TTFs: Regular, Bold, from its GitHub release zip, `fonts/ttf`.

Include each project's `OFL.txt`, renamed as above. Record the exact URLs and versions in the report. If there is no network access, report BLOCKED.

- [ ] **Step 2: Write the failing tests**

`crates/rogg-ui/src/theme/chrome.rs` unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_values_parse_case_insensitively() {
        assert_eq!(ChromeStyle::from_env_value("linux"), Some(ChromeStyle::Linux));
        assert_eq!(ChromeStyle::from_env_value("Windows"), Some(ChromeStyle::Windows));
        assert_eq!(ChromeStyle::from_env_value("MAC"), Some(ChromeStyle::Mac));
        assert_eq!(ChromeStyle::from_env_value("bsd"), None);
        assert_eq!(ChromeStyle::from_env_value(""), None);
    }

    #[test]
    fn current_matches_the_build_os() {
        let expected = if cfg!(target_os = "macos") { ChromeStyle::Mac } else if cfg!(windows) { ChromeStyle::Windows } else { ChromeStyle::Linux };
        assert_eq!(ChromeStyle::current(), expected);
    }
}
```

`crates/rogg-ui/src/theme/mod.rs`, add to its tests:

```rust
    #[test]
    fn the_font_pair_follows_the_chrome_style() {
        let mac = RoggTheme::new(Appearance::Dark, Density::Compact).with_chrome(ChromeStyle::Mac);
        assert_eq!((mac.fonts.ui.as_ref(), mac.fonts.mono.as_ref(), mac.fonts.log.as_ref()), (UI_FONT, MONO_FONT, MONO_FONT));
        for style in [ChromeStyle::Linux, ChromeStyle::Windows] {
            let t = RoggTheme::new(Appearance::Dark, Density::Compact).with_chrome(style);
            assert_eq!((t.fonts.ui.as_ref(), t.fonts.mono.as_ref(), t.fonts.log.as_ref()), (INTER, JETBRAINS_MONO, JETBRAINS_MONO), "{style:?}");
            assert_eq!(t.chrome, style);
        }
    }

    #[test]
    fn new_uses_the_build_os_style() {
        assert_eq!(RoggTheme::new(Appearance::Dark, Density::Compact).chrome, ChromeStyle::current());
    }
```

`crates/rogg-ui/tests/fonts.rs`:

```rust
mod common;
use gpui_kit::TestAppContext;

#[test]
fn every_bundled_font_is_embedded() {
    for name in ["Inter-Regular", "Inter-Medium", "Inter-SemiBold", "JetBrainsMono-Regular", "JetBrainsMono-Bold"] {
        let path = format!("fonts/{name}.ttf");
        let data = gpui_kit::AssetSource::load(&rogg_ui::assets::Assets, &path).unwrap().unwrap_or_else(|| panic!("{path} missing"));
        assert!(data.len() > 10_000, "{path} looks truncated");
    }
}

#[gpui_kit::test]
fn registering_the_fonts_succeeds(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        rogg_ui::fonts::register(cx);
    });
}
```

`crates/rogg-app/src/settings.rs` unit test (Review Focus 3):

```rust
    #[test]
    fn the_default_log_font_token_maps_to_the_platform_mono() {
        use rogg_ui::theme::{ChromeStyle, JETBRAINS_MONO, MONO_FONT};
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Mac).as_ref(), MONO_FONT);
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Linux).as_ref(), JETBRAINS_MONO);
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Windows).as_ref(), JETBRAINS_MONO);
        assert_eq!(log_font_family_for("Menlo", ChromeStyle::Linux).as_ref(), "Menlo");
        assert_eq!(default_log_font_label(ChromeStyle::Mac), "SF Mono");
        assert_eq!(default_log_font_label(ChromeStyle::Linux), "JetBrains Mono");
    }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `./dev test ui chrome`, `./dev test ui fonts`, `./dev test app the_default_log_font`.
Expected: compile errors.

- [ ] **Step 4: Implement**

`crates/rogg-ui/src/theme/chrome.rs`:

```rust
//! The window chrome style: Mac (traffic lights, native menu bar, SF fonts) or Linux / Windows (rogg-drawn window
//! controls, ☰ menu, bundled Inter and JetBrains Mono). Defaults to the build OS; `ROGG_CHROME` overrides it so the
//! other styles can be checked on any machine.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeStyle {
    Mac,
    Linux,
    Windows,
}

/// The environment variable that overrides the style at startup.
pub const CHROME_ENV: &str = "ROGG_CHROME";

impl ChromeStyle {
    pub const fn current() -> ChromeStyle {
        if cfg!(target_os = "macos") {
            ChromeStyle::Mac
        } else if cfg!(windows) {
            ChromeStyle::Windows
        } else {
            ChromeStyle::Linux
        }
    }

    pub fn from_env_value(value: &str) -> Option<ChromeStyle> {
        match value.to_ascii_lowercase().as_str() {
            "mac" | "macos" => Some(ChromeStyle::Mac),
            "linux" => Some(ChromeStyle::Linux),
            "windows" | "win" => Some(ChromeStyle::Windows),
            _ => None,
        }
    }

    /// `ROGG_CHROME` when set to a known style, else the build OS.
    pub fn resolved() -> ChromeStyle {
        std::env::var(CHROME_ENV).ok().and_then(|v| Self::from_env_value(&v)).unwrap_or_else(Self::current)
    }

    pub fn is_mac(self) -> bool {
        self == ChromeStyle::Mac
    }
}
```

`crates/rogg-ui/src/theme/mod.rs`:
- `mod chrome; pub use chrome::{CHROME_ENV, ChromeStyle};`;
- `pub const INTER: &str = "Inter"; pub const JETBRAINS_MONO: &str = "JetBrains Mono";`;
- add `pub chrome: ChromeStyle` to `RoggTheme`;
- `new` sets `chrome: ChromeStyle::current()` and the fonts via the same rule as `with_chrome`, so a Mac build is byte-identical to today;
- add:

```rust
    /// The chrome style and its font pair (SF on Mac; the bundled Inter and JetBrains Mono elsewhere). A custom log
    /// font (`with_log_font`) set before this is replaced, so call `with_chrome` first.
    pub fn with_chrome(mut self, chrome: ChromeStyle) -> Self {
        self.chrome = chrome;
        let (ui, mono) = if chrome.is_mac() { (UI_FONT, MONO_FONT) } else { (INTER, JETBRAINS_MONO) };
        self.fonts.ui = ui.into();
        self.fonts.mono = mono.into();
        self.fonts.log = mono.into();
        self
    }
```

`crates/rogg-ui/src/assets.rs`: the `#[include]` gains `"fonts/**/*.ttf"` (keep `"icons/**/*.svg"`); rust-embed's `include` accepts several attributes.

`crates/rogg-ui/src/fonts.rs`:

```rust
//! The bundled font pair for Linux and Windows chrome (SIL OFL 1.1; see the OFL texts next to the files).

use gpui_kit::{App, AssetSource};

const FILES: [&str; 5] = ["Inter-Regular", "Inter-Medium", "Inter-SemiBold", "JetBrainsMono-Regular", "JetBrainsMono-Bold"];

/// Registers the bundled fonts with gpui's text system. Harmless on macOS (the Mac style keeps SF), needed there too
/// so the gallery can render the other styles.
pub fn register(cx: &mut App) {
    let fonts = FILES
        .iter()
        .filter_map(|name| crate::assets::Assets.load(&format!("fonts/{name}.ttf")).ok().flatten())
        .collect::<Vec<_>>();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        eprintln!("rogg: could not register the bundled fonts: {e:#}");
    }
}
```

`lib.rs`: `pub mod fonts;`.

`crates/rogg-app/src/lib.rs` `init_with`:
- after `gpui_kit::init(cx);`, call `rogg_ui::fonts::register(cx);`;
- the theme install uses the resolved style: `settings::theme_for(…).install(cx)` must pass through `with_chrome(ChromeStyle::resolved())`. Put the style into `theme_with` (below) so every theme rebuild (settings changes, appearance changes) keeps it.

`crates/rogg-app/src/settings.rs`:
- `log_font_family(name)` becomes `log_font_family_for(name, chrome)`. It returns `MONO_FONT` on Mac and `JETBRAINS_MONO` elsewhere for `DEFAULT_LOG_FONT`; any other name passes through.
- Keep a thin `log_font_family(name)` calling it with `ChromeStyle::resolved()` if other callers need it.
- Add `pub fn default_log_font_label(chrome) -> &'static str` ("SF Mono" on Mac, "JetBrains Mono" elsewhere). The Settings font picker shows this label for the default entry. The stored value stays `DEFAULT_LOG_FONT`, so files are compatible across platforms.
- `theme_with` builds `RoggTheme::new(..).with_chrome(ChromeStyle::resolved())` and only then applies `with_log_font(log_font_family_for(&a.log_font, chrome), size)` and `with_ui_font_size`.
- `find_log_fonts` keeps the default entry first on every OS.

`crates/rogg-app/src/about.rs`: append the font credits to `CREDITS`, which becomes `[(&str, &str); 16]`: `("Inter (font)", "OFL-1.1")` and `("JetBrains Mono (font)", "OFL-1.1")`. Update the `credits_list_the_spec_crates_with_licenses` test's expected names.

`crates/rogg-gallery/src/main.rs` and `capture.rs`:
- call `rogg_ui::fonts::register(cx)` after `gpui_kit::init`;
- parse `--chrome mac|linux|windows` (default `ChromeStyle::current()`) in the CLI options;
- install the theme with `.with_chrome(chrome)`;
- set `std::env::set_var(CHROME_ENV, …)` before initialising if the app page reads `resolved()`. `set_var` is `unsafe` in edition 2024; prefer passing the style explicitly where possible, and only set it in `main` before any threads start, with a `// SAFETY:` note.

- [ ] **Step 5: Run the tests**

Run: `./dev test ui`, `./dev test app`, `./dev test gallery`, `./dev check`.
Expected: PASS, and visual-check stays at 0.000 % (Mac style, SF unchanged).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(ui): chrome style with bundled Inter and JetBrains Mono for Linux and Windows

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 3: Keys and labels: `secondary-`, platform-only bindings, label formatter

**Files:**
- Create: `crates/rogg-ui/src/keys.rs`
- Modify:
  - `crates/rogg-ui/src/lib.rs`;
  - `crates/rogg-app/src/actions.rs` (keymap, `shortcut_rows`);
  - `crates/rogg-ui/src/logview/actions.rs`, if it holds `cmd-` defaults.
- Test: `crates/rogg-ui/src/keys.rs` (unit), `crates/rogg-app/src/actions.rs` (unit).

**Interfaces:**
- Consumes: `ChromeStyle` (Task 2).
- Produces:
  - `rogg_ui::keys::shortcut_label(mac_label: &str, chrome: ChromeStyle) -> String`;
  - `rogg_app::actions::shortcuts_for(os: rogg_core::platform::Os) -> Vec<Shortcut>` (with `shortcuts()` = `shortcuts_for(Os::current())`);
  - `shortcut_rows()` returns labels formatted for `ChromeStyle::resolved()`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/src/keys.rs` unit tests (Review Focus 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ChromeStyle::{Linux, Mac, Windows};

    #[test]
    fn mac_labels_are_kept() {
        for l in ["⌘ O", "⌘ ↑ / ⌘ ↓", "⌥ ⌘ C", "⇧ ⌘ T", "Esc", "⌘ 1…9"] {
            assert_eq!(shortcut_label(l, Mac), l);
        }
    }

    #[test]
    fn labels_convert_chords_ranges_and_modifier_order() {
        assert_eq!(shortcut_label("⌘ O", Linux), "Ctrl+O");
        assert_eq!(shortcut_label("⌘ ↑ / ⌘ ↓", Windows), "Ctrl+↑ / Ctrl+↓");
        assert_eq!(shortcut_label("⌥ ⌘ C", Linux), "Ctrl+Alt+C");
        assert_eq!(shortcut_label("⇧ ⌘ T", Linux), "Ctrl+Shift+T");
        assert_eq!(shortcut_label("⌘ 1…9", Windows), "Ctrl+1…9");
        assert_eq!(shortcut_label("⌘ ,", Linux), "Ctrl+,");
        assert_eq!(shortcut_label("⌘ \\", Linux), "Ctrl+\\");
        assert_eq!(shortcut_label("⌃ Tab", Linux), "Ctrl+Tab");
        assert_eq!(shortcut_label("Esc", Linux), "Esc");
        assert_eq!(shortcut_label("↑ / ↓", Linux), "↑ / ↓");
        assert_eq!(shortcut_label("⇧ ↑ / ⇧ ↓", Linux), "Shift+↑ / Shift+↓");
    }
}
```

`crates/rogg-app/src/actions.rs` tests:
- replace `hide_is_bound_to_cmd_h` / `minimize_is_bound_to_cmd_m` with OS-parameterised ones;
- keep `every_binding_parses_and_is_unique_per_context` (run it for all three `Os`);
- add:

```rust
    use rogg_core::platform::Os;

    fn labels(os: Os) -> Vec<&'static str> {
        shortcuts_for(os).into_iter().map(|s| s.label).collect()
    }

    #[test]
    fn mac_only_bindings_exist_only_on_mac() {
        for label in ["Hide rogg", "Hide others", "Minimize"] {
            assert!(labels(Os::Mac).contains(&label), "{label} on Mac");
            assert!(!labels(Os::Linux).contains(&label), "{label} on Linux");
            assert!(!labels(Os::Windows).contains(&label), "{label} on Windows");
        }
    }

    #[test]
    fn quit_is_ctrl_q_on_linux_and_absent_on_windows() {
        assert!(labels(Os::Mac).contains(&"Quit"));
        assert!(labels(Os::Linux).contains(&"Quit"));
        assert!(!labels(Os::Windows).contains(&"Quit"), "Alt+F4 is the OS's");
    }

    #[test]
    fn every_binding_parses_and_is_unique_per_context_on_every_os() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            let mut seen = HashSet::new();
            for s in shortcuts_for(os) {
                let binding = (s.binding)();
                let keys: Vec<String> = binding.keystrokes().iter().map(|k| format!("{k:?}")).collect();
                let context = binding.predicate().map(|p| format!("{p:?}"));
                assert!(seen.insert((keys.clone(), context.clone())), "{os:?}: duplicate {keys:?} in {context:?}");
            }
        }
    }

    #[test]
    fn the_keymap_uses_the_platform_modifier() {
        // No binding hard-codes cmd: secondary means cmd on macOS and ctrl elsewhere.
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for s in shortcuts_for(os) {
                assert!(!s.keys.contains("cmd-"), "{os:?}: {} uses cmd-", s.keys);
            }
        }
    }
```

This needs `Shortcut` to keep its key string: add `pub keys: &'static str` to the struct and set it in the macro. The existing `export_settings_and_open_as_tab_are_listed` and `listed_rows_skip_the_unlabelled_entries` tests check Mac labels; make them assert through `shortcut_label(display, ChromeStyle::Mac)`, or run them under the Mac style.

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test ui keys`, `./dev test app actions`.
Expected: compile errors or failures.

- [ ] **Step 3: Implement**

`crates/rogg-ui/src/keys.rs`:

```rust
//! Shortcut labels per chrome style. The keymap writes Mac labels (`⌘ O`, `⇧ ⌘ T`, `⌘ ↑ / ⌘ ↓`); other styles show
//! `Ctrl+O`, `Ctrl+Shift+T`, `Ctrl+↑ / Ctrl+↓` (modifiers in Ctrl, Alt, Shift order).

use crate::theme::ChromeStyle;

pub fn shortcut_label(mac_label: &str, chrome: ChromeStyle) -> String {
    if chrome.is_mac() {
        return mac_label.to_string();
    }
    mac_label.split(" / ").map(chord).collect::<Vec<_>>().join(" / ")
}

/// One chord: space-separated Mac modifier glyphs, then the key.
fn chord(mac: &str) -> String {
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    let mut key = Vec::new();
    for token in mac.split(' ') {
        match token {
            "⌘" | "⌃" => ctrl = true,
            "⌥" => alt = true,
            "⇧" => shift = true,
            other => key.push(other),
        }
    }
    let mut parts: Vec<&str> = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    let key = key.join(" ");
    if parts.is_empty() {
        key
    } else {
        format!("{}+{}", parts.join("+"), key)
    }
}
```

`lib.rs`: `pub mod keys;`.

`crates/rogg-app/src/actions.rs`:
- **The macro** stores the keys: `Shortcut { label, display, keys: $keys, binding: || KeyBinding::new($keys, $action, $context) }`.
- **The modifier:** replace `cmd-` with `secondary-` in every key string; for example `"secondary-o"`, `"alt-secondary-c"`, `"shift-secondary-t"`, `"secondary-up"`. Keep `ctrl-tab` / `ctrl-shift-tab` as they are; they're Ctrl on every OS.
- **`shortcuts()`** becomes `shortcuts_for(Os::current())`.
- **`shortcuts_for(os)`** builds the common list, then:
  - on `Os::Mac`, appends Minimize (`secondary-m`), Hide (`secondary-h`), Hide Others (`alt-secondary-h`) and Quit (`secondary-q`);
  - on `Os::Linux`, appends Quit (`ctrl-q`, display "⌘ Q" so it formats to "Ctrl+Q");
  - on `Os::Windows`, appends nothing.
- **`shortcut_rows()`** formats each display with `rogg_ui::keys::shortcut_label(s.display, ChromeStyle::resolved())`.
- **`bind`** uses `shortcuts()` (the current OS).

Check `crates/rogg-ui/src/logview/actions.rs` and the log view's own bindings for `cmd-`. If any `KeyBinding::new("cmd-…")` exists outside `actions.rs`, switch it to `secondary-`.

- [ ] **Step 4: Run the tests**

Run: `./dev test ui`, `./dev test app`, `./dev check`.
Expected: PASS. The keyboard integration tests (`tests/keyboard.rs`) still press `cmd-…` on macOS, which `secondary-` matches.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: platform modifier keys, macOS-only bindings and Ctrl-style shortcut labels

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 4: Linux and Windows title bar, window controls, ☰ menu, resize edges, gallery captures

**Files:**
- Modify:
  - `crates/rogg-ui/src/{titlebar.rs,layout.rs,icons.rs}`, plus `assets/icons/rogg/{minimize,maximize,restore}.svg` if the existing icons don't cover them (create them in the existing style);
  - `crates/rogg-app/src/{menus.rs,window.rs}`, `crates/rogg-app/src/workspace/{mod.rs,keys.rs}`;
  - `crates/rogg-gallery/src/pages.rs` (a chrome specimen row).
- Test:
  - `crates/rogg-ui/tests/titlebar.rs`, `crates/rogg-app/src/menus.rs` (unit);
  - `crates/rogg-app/tests/app.rs` (the window options per style);
  - gallery captures for review.

**Interfaces:**
- Consumes: `ChromeStyle` and `theme.chrome` (Task 2); `shortcut_label` (Task 3).
- Produces:
  - `TitleBar::app_menu(f: impl Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu + 'static)`;
  - window-control ids `window-minimize`, `window-maximize`, `window-close`, and the `app-menu` button;
  - `rogg_app::menus::popup_menu(menus: &[Menu], menu: PopupMenu) -> PopupMenu`;
  - `rogg_app::window::window_options(state, chrome, cx)`;
  - `rogg_ui::layout::resize_edges(content: AnyElement) -> impl IntoElement`, active for client decorations.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/tests/titlebar.rs`, append (the theme's chrome is set through `common::setup_with` plus a re-install with `.with_chrome(…)`; add a `common::setup_chrome(cx, ChromeStyle)` helper):

```rust
#[gpui_kit::test]
fn mac_style_has_traffic_space_and_no_window_controls(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Mac);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).on_toggle_sidebar(|_, _| {}).into_any_element());
    assert!(common::exists(cx, w, "traffic"));
    for id in ["window-minimize", "window-maximize", "window-close", "app-menu"] {
        assert!(!common::exists(cx, w, id), "{id} on Mac");
    }
}

#[gpui_kit::test]
fn windows_style_draws_rectangular_controls_on_the_right(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Windows);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).into_any_element());
    assert!(!common::exists(cx, w, "traffic"));
    let close = common::bounds(cx, w, "window-close");
    let max = common::bounds(cx, w, "window-maximize");
    let min = common::bounds(cx, w, "window-minimize");
    assert_eq!((close.size.width, close.size.height), (px(46.), px(42.)));
    assert!(min.right() <= max.left() + px(0.5) && max.right() <= close.left() + px(0.5), "min | max | close, no gaps");
    assert!(close.right() >= px(1440.) - px(0.5), "flush with the right edge");
    assert!(common::exists(cx, w, "app-menu"));
}

#[gpui_kit::test]
fn linux_style_draws_round_controls_with_gaps(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Linux);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).into_any_element());
    let close = common::bounds(cx, w, "window-close");
    let max = common::bounds(cx, w, "window-maximize");
    assert_eq!((close.size.width, close.size.height), (px(24.), px(24.)));
    assert_eq!(close.left() - max.right(), px(8.));
    assert!(common::exists(cx, w, "app-menu"));
}
```

`crates/rogg-app/src/menus.rs` unit test (Review Focus 5):

```rust
    #[test]
    fn the_app_menu_mirrors_the_native_menus() {
        let menus = app_menus(&[RecentEntry::file("/var/log/a.log")]);
        let native: Vec<String> = menus.iter().flat_map(|m| m.items.iter()).filter_map(item_label).collect();
        let popup: Vec<String> = popup_entries(&menus).into_iter().filter(|l| !l.is_empty()).collect();
        for label in &native {
            assert!(popup.contains(label), "☰ menu lacks {label}");
        }
        assert!(popup.contains(&"About rogg".to_string()) && popup.contains(&"Open Recent".to_string()));
    }
```

`popup_entries(&[Menu]) -> Vec<String>` is a pure helper in `menus.rs`. It lists the labels the ☰ menu will show, in order, with submenu names and their items flattened, and `""` for separators. `popup_menu` builds the `PopupMenu` from the same walk. `item_label(&MenuItem) -> Option<String>` returns an action's or submenu's name.

`crates/rogg-app/tests/app.rs`:

```rust
#[gpui_kit::test]
fn window_options_follow_the_chrome_style(cx: &mut TestAppContext) {
    use gpui_kit::WindowDecorations;
    use rogg_ui::theme::ChromeStyle;
    let _dir = common::setup(cx);
    cx.update(|cx| {
        let mac = rogg_app::window::window_options(None, ChromeStyle::Mac, cx);
        assert!(mac.titlebar.as_ref().is_some_and(|t| t.appears_transparent && t.traffic_light_position.is_some()));
        let linux = rogg_app::window::window_options(None, ChromeStyle::Linux, cx);
        assert_eq!(linux.window_decorations, Some(WindowDecorations::Client));
        assert!(linux.titlebar.as_ref().is_some_and(|t| t.traffic_light_position.is_none()));
        let windows = rogg_app::window::window_options(None, ChromeStyle::Windows, cx);
        assert!(windows.titlebar.as_ref().is_some_and(|t| t.appears_transparent && t.traffic_light_position.is_none()));
    });
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `./dev test ui titlebar`, `./dev test app menus`, `./dev test app window_options`.
Expected: compile errors.

- [ ] **Step 3: Implement the title bar**

In `crates/rogg-ui/src/titlebar.rs`:
- Add `app_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>` with the `app_menu(f)` builder.
- `render` reads `let chrome = cx.rogg().chrome;`. On `ChromeStyle::Mac` the render is exactly today's.
- **Linux and Windows, leading block:** no `traffic` element. Instead an `app-menu` button sits before the sidebar toggle: a ☰ glyph or a hamburger icon in the existing icon style. It opens the dropdown using gpui-kit's `DropdownMenu` (`gpui_kit::component::button::Button::new("app-menu").ghost()…dropdown_menu(move |menu, window, cx| f(menu, window, cx))`), styled with the theme's muted colour and the same 25 px size as the other title-bar icon buttons.
- **Linux and Windows, right side:** after `title-actions`, add `window_controls(chrome, window.is_maximized(), p)`:

```rust
/// Minimise, maximise/restore and close, drawn by rogg for client-decorated windows.
fn window_controls(chrome: ChromeStyle, maximized: bool, p: Palette) -> AnyElement {
    let windows = chrome == ChromeStyle::Windows;
    let button = |id: &'static str, icon: IconName, area: WindowControlArea, close: bool| {
        let hover_bg = if close && windows { Color::rgb(0xc42b1c) } else { p.hover };
        div()
            .id(id)
            .test_support()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .when(windows, |d| d.w(px(46.)).h(px(size::TITLEBAR_H)))
            .when(!windows, |d| d.size(px(24.)).rounded_full())
            .text_color(p.muted)
            .window_control_area(area)
            .hover(move |s| if close && windows { s.bg(hover_bg).text_color(Color::rgb(0xffffff)) } else { s.bg(hover_bg) })
            .child(icon(icon).size(px(if windows { 14. } else { 12. })).text_color(p.muted))
            .on_click(move |_, window, _| match area {
                WindowControlArea::Min => window.minimize_window(),
                WindowControlArea::Max => window.zoom_window(),
                WindowControlArea::Close => window.remove_window(),
                WindowControlArea::Drag => {}
            })
    };
    div()
        .id("window-controls")
        .flex()
        .flex_none()
        .items_center()
        .when(!windows, |d| d.gap(px(8.)).pl(px(8.)))
        .child(button("window-minimize", IconName::Minimize, WindowControlArea::Min, false))
        .child(button("window-maximize", if maximized { IconName::Restore } else { IconName::Maximize }, WindowControlArea::Max, false))
        .child(button("window-close", IconName::Close, WindowControlArea::Close, true))
        .into_any_element()
}
```

- `remove_window` is the close path on gpui (`Window::remove_window`). On Windows, `window_control_area` also gives native snap-layout hit-testing.
- Add `IconName::{Minimize, Maximize, Restore}` with SVGs in the existing stroke style if they're missing. `Minus` may serve as minimise; add the others, and update `IconName::ALL` and the `assets` test.
- **Root padding:** when the style isn't Mac, the title bar root's `pr(px(10.))` becomes `pr(0)` on Windows, so the controls are flush, and stays `pr(px(10.))` on Linux.
- **Dragging:** the existing `drag_window` handlers (double-click maximises, drag moves) already apply to the drag areas. Keep them, and add `window_control_area(WindowControlArea::Drag)` to the `window-drag` filler for Windows.

`crates/rogg-ui/src/layout.rs`: add `pub fn resize_edges(content: AnyElement) -> impl IntoElement`. It wraps the window content in a relative container with four 5 px edge strips and four corner squares (absolute, transparent). Each calls `window.start_window_resize(edge)` on mouse down and shows the matching resize cursor. Use it only when `window.window_decorations()` is `Decorations::Client`, which the workspace checks at render.

- [ ] **Step 4: Implement the menu and window options**

`crates/rogg-app/src/menus.rs`:
- `popup_entries(menus)` and `popup_menu(menus, menu)`;
- walk `menus` in order: the first menu's (rogg) items go at the top level, and every later menu becomes a `PopupMenu::submenu(name, …)` entry;
- actions become `menu.menu(name, action.boxed_clone())`, so gpui-kit shows each item's key binding from the active keymap;
- separators become `menu.separator()`;
- the "Open Recent" submenu keeps its entries.

`crates/rogg-app/src/window.rs`:
- `window_options(state, chrome, cx)`:
  - Mac: today's options, unchanged;
  - Linux: `titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position: None })`, `window_decorations: Some(WindowDecorations::Client)`;
  - Windows: `TitlebarOptions { appears_transparent: true, traffic_light_position: None, title: Some("rogg".into()) }`, decorations left default.
- `app_owns_titlebar_drag: true` for every style.
- `open_main_window` passes `ChromeStyle::resolved()`.
- Update the `window_options` callers and tests (`crates/rogg-app/tests/session.rs` and others call `window_options(state, cx)`, so add the chrome argument there as `ChromeStyle::Mac`).

`crates/rogg-app/src/workspace/mod.rs` (`titlebar()` and `render`):
- when the style isn't Mac, pass `.app_menu(…)`, building from `menus::popup_menu(&menus::app_menus(&recent), menu)`, with `recent` from `RecentStore`;
- wrap the root content with `rogg_ui::layout::resize_edges` when `window.window_decorations()` is client.

`workspace/keys.rs` already handles `Minimize` and `Zoom`; the ☰ menu items dispatch those actions.

`crates/rogg-gallery`:
- with `--chrome linux|windows` (Task 2), the `tabs` page and the `app` page render the title bar in that style;
- the gallery passes a no-op `app_menu`, so the ☰ button shows;
- capture for review:
  - `./dev capture app dark compact` with `--chrome linux`;
  - the same with `--chrome windows`;
  - `tabs` with both.

`cmd_capture` in `dev` forwards extra args. Add an optional 4th positional `chrome` (default `mac`) and pass `--chrome "$chrome"`. Put the PNG paths in the report.

- [ ] **Step 5: Run the tests and look**

Run: `./dev test ui`, `./dev test app`, `./dev test gallery`, `./dev check` (visual-check 0.000 %, Mac style). Then:
- `./dev capture app dark compact linux`
- `./dev capture app dark compact windows`
- `./dev capture tabs light compact linux`

Open the PNGs and confirm:
- the ☰ button sits at the left;
- there's no traffic-light block;
- the controls are on the right: Windows flush rectangles, Linux round with gaps;
- the UI text is in Inter and the log in JetBrains Mono.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: Linux and Windows title bar with rogg-drawn window controls, ☰ menu and resize edges

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 5: macOS-only test gating and the Linux container (build and test)

**Files:**
- Create: `tools/linux/Containerfile`
- Modify:
  - `dev`, `justfile`;
  - `crates/rogg-ui/tests/logview_real_text.rs`, `crates/rogg-gallery/tests/capture.rs`, `crates/rogg-core/tests/memory.rs`;
  - any code the Linux build or tests flag.

**Interfaces:**
- Produces: `./dev linux-image | linux-build | linux-test | linux-shell`.
- The image tag is `rogg-linux:latest`. The named volumes are `rogg-cargo-registry` and `rogg-cargo-git`. The target dir is `target/linux`.

- [ ] **Step 1: Gate the macOS-only tests**

- `crates/rogg-ui/tests/logview_real_text.rs` (`harness = false`): wrap `main`'s body so that on non-macOS it prints `skipped: CoreText harness is macOS-only` and returns. The cases and helpers get `#[cfg(target_os = "macos")]` (or the whole file is `#![cfg_attr(not(target_os = "macos"), allow(dead_code))]` with `cfg` on the macOS-only imports). It must compile and pass on Linux and Windows.
- `crates/rogg-gallery/tests/capture.rs`: add `#![cfg(target_os = "macos")]` (headless Metal capture).
- `crates/rogg-core/tests/memory.rs`: add `#[cfg(target_os = "macos")]` on the RSS-reading test, if it uses macOS APIs; otherwise leave it, since it's `#[ignore]`d anyway.
- Run `./dev check` on macOS. Expected: green and unchanged.

- [ ] **Step 2: The Containerfile**

`tools/linux/Containerfile`:

```dockerfile
# rogg's Linux build, test and screenshot environment (spec §5.1). Debian bookworm, arm64 like Apple silicon hosts.
FROM debian:bookworm

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl git build-essential pkg-config cmake clang \
      libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev \
      libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcursor-dev libxrandr-dev libxi-dev \
      libfontconfig1-dev libfreetype6-dev libvulkan-dev libvulkan1 mesa-vulkan-drivers libssl-dev \
      xvfb xauth x11-utils imagemagick xdotool zsh \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
# The pinned toolchain comes from rust-toolchain.toml on first use; install rustup and that channel up front.
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0 --component clippy,rustfmt
# Software Vulkan (lavapipe) for wgpu under Xvfb.
ENV VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.aarch64.json WGPU_BACKEND=vulkan
WORKDIR /work
```

(If the ICD file name differs in the image, for example `lvp_icd.json`, fix it: `ls /usr/share/vulkan/icd.d/`. If gpui's build needs another `-dev` package, add it and say so in the report.)

- [ ] **Step 3: `./dev` commands**

Add to `dev` (zsh, macOS host):

```zsh
LINUX_IMAGE=rogg-linux:latest

need_container() {
  command -v container >/dev/null 2>&1 || die "Apple container is not installed — https://github.com/apple/container"
  container system status >/dev/null 2>&1 || run container system start
}

# Runs a shell command in the Linux image with the repo mounted and the cargo caches in named volumes.
in_linux() {
  need_container
  run container run --rm \
    --volume "$ROOT:/work" \
    --volume rogg-cargo-registry:/usr/local/cargo/registry \
    --volume rogg-cargo-git:/usr/local/cargo/git \
    --env CARGO_TARGET_DIR=/work/target/linux \
    --cpus 6 --memory 12G \
    --workdir /work "$LINUX_IMAGE" bash -lc "$1"
}

cmd_linux_image() { need_container; step "Linux image"; run container build --tag "$LINUX_IMAGE" --file tools/linux/Containerfile tools/linux; }
cmd_linux_build() { step "Linux build"; in_linux "cargo build --workspace --all-targets"; }
cmd_linux_test()  { step "Linux tests"; in_linux "cargo test --workspace"; }
cmd_linux_shell() { need_container; run container run --rm -it --volume "$ROOT:/work" --volume rogg-cargo-registry:/usr/local/cargo/registry --env CARGO_TARGET_DIR=/work/target/linux --workdir /work "$LINUX_IMAGE" bash; }
```

- Check the exact `container system` subcommand for the installed 1.1.0 (`container --help`). If it uses `container machine` instead, adapt `need_container`.
- Named volumes may need `container volume create <name>` first. Make `in_linux` create them idempotently.
- Add help lines under a new `Linux` section, add the dispatch cases, and add justfile recipes.

- [ ] **Step 4: Build and test in the container, then fix what fails**

Run: `./dev linux-image`, `./dev linux-build`, `./dev linux-test`.
Expected: build OK, all tests pass. Fix what fails:
- missing `-dev` packages go in the Containerfile;
- Unix-but-not-macOS assumptions in code or tests get the right `cfg` or a portable rewrite;
- tests that assume macOS paths or SF fonts are gated or made OS-neutral.

Every fix keeps macOS behaviour identical. List each fix in the report with its reason.

- [ ] **Step 5: Gate on both platforms and commit**

Run `./dev check` (macOS green) and `./dev linux-test` (green).

```bash
git add -A
git commit -m "build(linux): container image and ./dev linux-build/test; gate macOS-only tests

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 6: Linux screenshots of the running app (user review)

**Files:**
- Create: `tools/linux/shot.sh`
- Modify: `dev`, `justfile`

**Interfaces:**
- Produces: `./dev linux-shot [demo|<gallery page>]`, writing `target/linux/shots/<name>.png` (plus `demo-menu.png` for the demo).

- [ ] **Step 1: The screenshot driver**

`tools/linux/shot.sh` (runs inside the container):

```bash
#!/usr/bin/env bash
# Starts a virtual X display, runs rogg (or a gallery page) with software Vulkan, captures the screen, then quits.
# Usage: tools/linux/shot.sh demo | <gallery-page>
set -euo pipefail
what=${1:-demo}
out=/work/target/linux/shots
mkdir -p "$out"
export DISPLAY=:99
Xvfb :99 -screen 0 1440x900x24 -nolisten tcp &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done

if [[ $what == demo ]]; then
  cargo build -p rogg-app
  /work/target/linux/debug/rogg --demo &
else
  cargo build -p rogg-gallery
  /work/target/linux/debug/rogg-gallery --page "$what" &
fi
app=$!
# Wait for the window to map and render a few frames.
for _ in $(seq 100); do xdotool search --onlyvisible --name . >/dev/null 2>&1 && break; sleep 0.2; done
sleep 3
import -window root "$out/$what.png"
if [[ $what == demo ]]; then
  # Open the ☰ menu: click the app-menu button (the leading block, 13 px padding + half the 25 px button).
  xdotool mousemove 25 21 click 1
  sleep 1
  import -window root "$out/demo-menu.png"
fi
kill $app 2>/dev/null || true
wait $app 2>/dev/null || true
echo "shots in target/linux/shots"
```

(`chmod +x`. If the app-menu position differs, read it from the layout: the leading block starts at x=0, with `px(13.)` padding, and the ☰ button is the first child. Adjust the click to the button's centre.)

`dev`:

```zsh
cmd_linux_shot() { step "Linux screenshots"; in_linux "tools/linux/shot.sh ${1:-demo}"; ok "target/linux/shots/"; }
```

Add help, dispatch and justfile entries.

- [ ] **Step 2: Capture**

Run: `./dev linux-shot demo` and `./dev linux-shot controls`.

Expected:
- `target/linux/shots/demo.png` shows the demo window with Linux chrome: ☰, no traffic lights, round window controls, Inter and JetBrains Mono;
- `demo-menu.png` shows the ☰ menu open;
- `controls.png` shows the gallery controls page.

If the app doesn't start (for example wgpu can't find an adapter), diagnose:
- run with `RUST_LOG=wgpu=info`;
- check `vulkaninfo --summary` (install `vulkan-tools` if needed);
- check `VK_ICD_FILENAMES`.

Fix it in the Containerfile or script, and report.

- [ ] **Step 3: Commit, then hand off for the user's review (stop here)**

```bash
git add tools/linux/shot.sh dev justfile
git commit -m "build(linux): ./dev linux-shot screenshots of the running app under Xvfb

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

Show the user the three PNGs, and wait for their review before Task 7.

### Task 7: Windows cross-build

**Files:**
- Modify: `dev`, `justfile`, plus any code the Windows build flags (likely `platform.rs` details, path joins, `std::os::unix` uses left anywhere).

**Interfaces:**
- Produces: `./dev windows-build`; `./dev setup` offers cargo-xwin and the target.

- [ ] **Step 1: Setup and command**

In `cmd_setup`, after the Rust step:

```zsh
  step "Windows cross-build (optional)"
  if rustup target list --installed | grep -q x86_64-pc-windows-msvc && command -v cargo-xwin >/dev/null 2>&1; then
    ok "cargo-xwin and the x86_64-pc-windows-msvc target are installed"
  elif confirm "Install cargo-xwin and the x86_64-pc-windows-msvc target for ./dev windows-build?"; then
    run rustup target add x86_64-pc-windows-msvc
    run cargo install cargo-xwin --locked
  fi
```

Then add the command:

```zsh
cmd_windows_build() {
  need_rust
  command -v cargo-xwin >/dev/null 2>&1 || die "cargo-xwin is not installed — run ./dev setup"
  step "Windows cross-build"
  run cargo xwin build --workspace --target x86_64-pc-windows-msvc
}
```

Add help, dispatch and justfile entries.

Ask the user before running `./dev setup`'s install step. If they decline, `windows-build` cannot run; report BLOCKED.

- [ ] **Step 2: Build and fix**

Run: `./dev windows-build`. Fix what fails, keeping macOS and Linux unchanged:
- `windows-sys` API details in `platform.rs`;
- `std::os::unix` imports that aren't `cfg(unix)`;
- `libc` uses outside `cfg(unix)`;
- path separators in code (not tests);
- the zsh-only bits that are only in the dev script are unaffected.

`cargo xwin test --no-run --target x86_64-pc-windows-msvc` should also compile the test binaries. Run it and report.

- [ ] **Step 3: `check-all`**

```zsh
cmd_check_all() { cmd_check; cmd_linux_test; cmd_windows_build; ok "macOS, Linux and Windows checks passed"; }
```

Add help, dispatch and justfile entries.

- [ ] **Step 4: Gate and commit**

Run: `./dev check`, `./dev linux-test`, `./dev windows-build`. All three must be green.

```bash
git add -A
git commit -m "build(windows): cross-build with cargo-xwin; ./dev windows-build and check-all

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 8: Docs and the final gate

**Files:**
- Modify:
  - `README.md` (or create it, if missing: what rogg is, plus macOS, Linux and Windows sections);
  - `docs/superpowers/specs/2026-10-06-rogg-design.md` (a "Platforms" note in §3 and §5.9 keyboard: `secondary-`, the label formats, the macOS-only bindings);
  - `docs/superpowers/notes/2026-10-09-cross-platform-manual-checks.md` (create).

- [ ] **Step 1: README platform sections**

For each platform, write:
- build and run commands;
- where data lives (the spec §3 table);
- keys (⌘ vs Ctrl);
- what's verified (macOS full; Linux in the container with `./dev linux-test` / `linux-shot`; Windows cross-built only, not run).

Add the `./dev` command list additions.

- [ ] **Step 2: Manual checks note**

Write a note for the user covering:
- running the Linux build on a real desktop (Wayland and X11): window controls, resize from the edges, the ☰ menu, Ctrl shortcuts, data folders;
- the first Windows run: the same list, plus snap layouts on the maximise button, `%APPDATA%\rogg`, and piping into `rogg.exe -`.

- [ ] **Step 3: Final gate and commit**

Run: `./dev check-all`. Expected: macOS check green (visual-check 0.000 %), Linux tests green, Windows build OK.

```bash
git add -A
git commit -m "docs: Linux and Windows support — README, design spec, manual checks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Spec coverage

| Spec item | Task |
|---|---|
| §1 success 1 (macOS unchanged) | every task's `./dev check` gate; Task 8 `check-all` |
| §1 success 2 (Linux build, tests, screenshots) | 5, 6 |
| §1 success 3 (Windows cross-build plus tests) | 1 (Windows code paths), 7 |
| §1 success 4 (gallery captures in the other styles) | 4 |
| §3 OS layer: folders, process check, stdin, reveal, UI text | 1. Reveal stays gpui's `reveal_path`, which has a backend per OS |
| §3 `windows-sys` dependency | 1 |
| §4.1 `ChromeStyle`, `ROGG_CHROME`, `--chrome` | 2, 4 |
| §4.2 title bar, controls, resize | 4 |
| §4.3 ☰ menu built from one definition | 4 |
| §4.4 `secondary-` keys, macOS-only bindings, Ctrl+Q, labels | 3 |
| §4.5 fonts and credits; default log font mapping | 2 |
| §4.6 goldens unchanged | every task's gate |
| §5.1 Linux container and `./dev` commands | 5, 6 |
| §5.2 Windows cross-build and setup offer | 7 |
| §5.3 `build.rs`, macOS-only test gating, `check-all` | 1, 5, 7 |
| §6 testing list | 1–4 tests; 5–7 gates |
| §7 order | Tasks 1→8 |
| §8 out of scope | not implemented |
