# rogg M4 — Advanced Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish v1. Add:
- highlighting profiles: a model, built-ins, an editor, and per-tab application to both panes;
- a settings dialog whose changes apply immediately;
- session restore and `.rogg` files;
- export of filtered lines, sessions and the filter expression;
- "Open filtered as tab" (derived tabs, nested, with original line numbers);
- the statistics popover and a richer history popover;
- the M3 → M4 carry-over fixes chosen below.

**Architecture:**
- **`rogg-core`** gains the data models and the file work, all gpui-free:
  - settings (`settings.toml`) and profiles (`profiles/*.toml`, import/export `.roggprofile`), through TOML persistence with `.bak` recovery;
  - profile matching (`CompiledProfile::style`: for each effect, the first matching rule wins);
  - the session schema (`session.json` and `.rogg`, the same schema);
  - derived sources, materialised into a cache file plus an original-number map;
  - a background-friendly writer for filtered-line exports.
- **`rogg-ui`** gains:
  - bold and explicit-colour spans in the log view;
  - a configurable log font, log size and UI text scale;
  - chip colour overrides;
  - a clickable statistics area in the footer;
  - form controls: toggle, stepper, form row, swatches, text field, and editable rule and palette rows.
- **`rogg-app`** wires it all together:
  - `SettingsStore` and `ProfileStore` globals, applied live;
  - profile-styled row sources that replace `decorate.rs`;
  - the settings, profile editor, export, statistics and font picker dialogs;
  - derived tabs with breadcrumb navigation;
  - session save (every 30 s and on quit) and restore, window frame included;
  - `.rogg` open;
  - the new keys ⌘E, ⌘, and ⇧⌘T.

**Tech Stack:**
- Rust 1.99.0 (edition 2024).
- gpui-pre 0.3.8 through gpui-kit =0.7.1.
- One new pin: `toml =1.1.6` (already in `Cargo.lock` as a transitive dependency, so it builds offline).
- The Node + Playwright visual harness from M0.

**Spec:** `docs/superpowers/specs/2026-10-06-rogg-design.md` (§3.2–3.4, §4.9, §4.11, §4.12, §5.3, §5.5, §5.6, §5.8, §5.9, §6.1–6.6, §7.1, §8, §9 M4).

Carry-over folded in, from `docs/superpowers/notes/2026-10-07-m3-carryover-to-m4.md`:
- settings for the cache limit and the folder scan depth;
- idle live-follow polling (energy);
- a drop guard so a panicking stream capture still reports `done`;
- trailing bytes after the last gzip member are ignored, as `gzip -d` does;
- the missing app-level mid-scan truncation test.

The remaining carry-over items are listed under "Deferred from M4 (post-v1)" at the end.

**Decisions this plan makes (flagged for the plan review):**

1. **Derived tabs copy their lines into a cache file**, `streams/derived-<pid>-<n>.log`. Each line keeps its original number through a number map.
   - Spec §4.9 instead describes reading lines through the parent's mapping.
   - Copying lets every engine path (scan, search, export, nesting, session rebuild) work unchanged, at the cost of disk space for the shown lines.
   - The parent no longer has to stay mapped.
2. **Built-in profiles include a `Timestamp` rule** (a leading timestamp painted faint). This keeps today's look, which came from `decorate.rs`.
   - The spec's `ERROR` / `WARN` / `DEBUG` rules colour the **whole line**. The reference dialog says "Text" for these rules and "Match text" for `request_id=\S+`.
   - So the demo, which uses `Services`, now shows ERROR lines in the error colour and WARN lines in amber. The reference mockup rows do not.
   - The replica goldens are unaffected. The app captures change, and Task 20 reports that for the joint review.
3. **Colours in profiles are a theme token or a hex value.**
   - Tokens are `error`, `amber`, `muted`, `faint`, `accent`, `text`, `hit-0`…`hit-3` and `hit-0-bg`…`hit-3-bg`.
   - Built-ins use tokens, so they follow both themes exactly.
   - A hex colour gets an automatically derived light-theme variant (spec §6.1). The user can override it.
   - A row tint uses the rule colour at alpha `0x18`. The `error` token uses the palette's `error_row_tint`, exactly as today.
4. **"Bold" bolds the matched text**, not the whole line.
5. **"UI font size" (11–16 px, default 13) scales the UI text sizes.** Bar and row heights stay fixed. At 13 px every component renders exactly as today.
6. **The "Log font size" setting is the compact size.** Comfortable density adds 1 px, as in the reference (12 → 13). Row height keeps each density's ratio (23/12, 29/13).

## Global Constraints

- **Platform:** macOS only (Apple Silicon). No cross-platform abstractions (spec §2).
- **GUI:**
  - Use `gpui-pre =0.3.8` through `gpui-kit =0.7.1` only. Import gpui types from `gpui_kit`; never add a `gpui` dependency.
  - Crates that use `gpui_kit::actions!` or `#[derive(gpui_kit::Action)]` keep `extern crate gpui_kit as gpui;` at their root.
- **Toolchain:**
  - Rust 1.99.0, pinned by `rust-toolchain.toml`.
  - Prefix every cargo or just command with `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH";`.
  - Build and run the app with `-p rogg-app`.
- **Dependencies:**
  - Exactly one new third-party crate: `toml = "=1.1.6"`.
  - Pin it in the root `Cargo.toml` `[workspace.dependencies]` and use it with `.workspace = true` (spec §3.4).
  - Nothing else is added.
- **Boundaries (spec §3.2):**
  - `rogg-core` has no gpui dependency. It owns the profile, settings, session and history models, their files, derived-source materialisation and export writing.
  - `rogg-ui` renders plain data and reports intents through callbacks. It never touches files or the engine.
  - `rogg-app` is the only crate that depends on both.
- **Concurrency (spec §3.3):**
  - Export, derived-tab materialisation, derived-tab rebuilds on restore and session file writes for `.rogg` exports run off the UI thread.
  - The UI polls their channels at most once per frame (`POLL_INTERVAL` = 16 ms).
  - Closing a tab cancels its jobs.
  - Never join or block on a worker from the UI thread.
- **Persistence (spec §6.2–6.6):**
  - All state files live under `AppPaths`: `settings.toml`, `profiles/*.toml`, `session.json`, `history.json`, `recent.json`.
  - Writes are atomic (`<file>.partial` then rename).
  - An unparseable state file is renamed to `<file>.bak`, replaced with defaults and reported with a toast.
  - A `.rogg` or `.roggprofile` file the user opens or imports is **never** renamed. A broken one is reported with a toast.
  - Out-of-range values are clamped on load.
- **Changes apply immediately (spec §6.2):** settings, profile edits and a tab's profile choice redraw at once and are saved at once.
- **M1–M3 contracts still hold:**
  - A `FilterJob` channel that closes without `Finished` or `Cancelled` means the worker died.
  - Never drop a `FilterJob` on a rayon thread.
  - Slice raw lines to 64 KB before decoding or highlighting.
  - Every cache file is written through `Cache::write_entry` (writer-unique partial, then rename).
  - Never panic on input data.
- **Visuals:**
  - Use only `rogg_ui::theme` tokens; no gpui-kit default colours.
  - The replica goldens (`just visual-check`) must stay at **0.000%** on all 9 captures. `dialogs::profile`, `dialogs::appearance`, `dialogs::history`, `RuleRow`, `InfoBar`, `StatusBar`, `FilterBand` and the log view must render pixel-identically at default settings.
  - New controls and dialogs appear on the gallery's component pages (Controls, Dialogs), which are not golden-checked.
  - The user's approved deviations stand (memory `rogg-visual-decisions`):
    - the horizontal thumb shows only on hover or while scrolling;
    - the focus line is 1 px on the panel perimeter;
    - the log font is SF Mono through `.AppleSystemUIFontMonospaced`.
- **Display numbers:**
  - A file's first line is number 1. Engine line indices are 0-based.
  - A derived tab shows its parent's display numbers.
  - Convert only through `Numbers` (Task 16) at the `Tab` and rows boundary.
- **Copy:**
  - Tab label `<name> ⟩ filtered`. Info-bar note `Snapshot · HH:MM:SS`.
  - Toasts:
    - `Exported 1,204 lines to api-filtered.log`
    - `Saved session to incident.rogg`
    - `Filter expression copied`
    - `Settings were unreadable and have been reset; the old file was kept as settings.toml.bak`
    - `Session was unreadable and has been reset; the old file was kept as session.json.bak`
    - `Profile <file> was unreadable; the old file was kept as <file>.bak`
  - Footer progress `Exporting N%` / `Building tab N%`.
  - Dialog titles:
    - `Settings`
    - `Highlighting · <name>`
    - `Export`
    - `Filtered log statistics`
    - `Filter history`
    - `Choose a log font`
- **Commits** are conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

These are the five failure modes most likely to bite a user that the feature tests would not otherwise exercise. Each names the task that owns it; that task carries a test for it.

1. **A corrupt or hand-edited settings, profile or session file** (bad TOML or JSON, unknown values, out-of-range numbers, a profile without a name).
   - Expected: the app starts with defaults for that file only and keeps the old file as `.bak` with a toast. Clamped values load clamped, and other files are untouched.
   - Tests:
     - Task 2: `corrupt_settings_recover_and_values_clamp`.
     - Task 3: `a_broken_profile_file_is_set_aside_alone`.
     - Task 5: `corrupt_session_is_set_aside_but_rogg_files_are_not`.
     - Task 10: `unreadable_settings_toast_and_reset`.
2. **A profile rule with an invalid regex** (typed live in the editor, or in an imported file).
   - Expected: the rule is skipped and its error is shown under the pattern. The other rules still colour the rows, and nothing panics.
   - Tests:
     - Task 4: `invalid_rules_are_skipped_and_reported`.
     - Task 14: `an_invalid_pattern_shows_its_error_and_keeps_other_rules`.
3. **Restoring a session whose files moved or whose archive vanished.**
   - Expected: those tabs open in the error state with Retry. The other tabs restore with their filter, pins and selection. A derived tab whose parent is missing fails alone.
   - Test: Task 17, `restore_keeps_good_tabs_and_fails_missing_ones`.
4. **Exporting to a destination that cannot be written** (a read-only folder, a full disk), or closing the tab mid-export.
   - Expected: a toast with the error, no partial file left behind, and the UI never blocked.
   - Tests:
     - Task 6: `export_failure_leaves_no_partial`.
     - Task 18: `export_errors_toast_and_closing_cancels`.
5. **Line numbers in derived and nested derived tabs.**
   - Expected: Go to line, pins, Return/reveal, search jumps, export number prefixes and the session use the original numbers, never row indices.
   - Test: Task 16, `nested_derived_tabs_keep_original_numbers`.

---

## File Structure

```
Cargo.toml                         MOD  pin toml =1.1.6
crates/rogg-core/
  Cargo.toml                       MOD  toml
  src/persist.rs                   MOD  write_atomic, load_toml/save_toml, strict readers
  src/paths.rs                     MOD  settings / profiles / session paths
  src/error.rs                     MOD  CoreError::Parse
  src/clock.rs                     NEW  local_hms (snapshot time)
  src/settings.rs                  NEW  Settings model (spec §6.2)
  src/profile.rs                   NEW  Profile, LineRule, HitEntry, ColorSpec, built-ins, storage, import/export, derive_light
  src/style.rs                     NEW  CompiledProfile::style (rule matching per painted row)
  src/highlight.rs                 MOD  Finder / snapping shared with style.rs
  src/session.rs                   NEW  Session, TabState, SessionSource, DerivedSpec, .rogg
  src/derived.rs                   NEW  view_lines, materialize, rebuild
  src/export.rs                    NEW  export_lines (spec §6.5)
  src/view.rs                      MOD  is_anchor
  src/cache.rs                     MOD  settable limit, derived_path, derived files swept like streams
  src/stream.rs                    MOD  done-guard on the capture thread
  src/archive.rs                   MOD  trailing bytes after the last gzip member are ignored
crates/rogg-ui/src/
  logview/model.rs, text.rs, element.rs  MOD  Bold / Mark spans, log font family
  theme/{mod,metrics,kit}.rs       MOD  Fonts::log, ui text scale, metrics_for(font size)
  (components)                     MOD  text sizes through `t.ui(n)`
  filter.rs                        MOD  chip colour overrides
  chrome.rs                        MOD  StatusBar::on_stats
  form.rs                          NEW  Toggle, Stepper, FormRow, SectionLabel, Swatch, TextField, EditableRuleRow, PaletteRow
crates/rogg-gallery/src/           MOD  specimens for the new controls; app-page profile/appearance dialogs
crates/rogg-app/src/
  paths.rs                         MOD  settings / profiles / session
  settings.rs                      NEW  SettingsStore + live apply
  profiles.rs                      NEW  ProfileStore + RowStyler (paint resolution)
  numbers.rs                       NEW  Numbers (display ↔ line)
  rows.rs                          MOD  styler instead of decorate; Numbers; Mark hits
  decorate.rs                      DEL
  derived.rs                       NEW  DerivedSource, DerivedJob
  export.rs                        NEW  ExportJob
  session.rs                       NEW  capture / restore / autosave
  dialogs/{mod,settings,profiles,export,stats,fonts}.rs  NEW  dialog bodies
  document/{mod,filter}.rs         MOD  Source::Derived, numbers, idle polling
  tab.rs, tab_view.rs              MOD  profile, crumbs, statistics, open as tab, export progress
  workspace/{mod,chrome,keys}.rs   MOD  dialogs, settings apply, restore, .rogg open
  actions.rs, menus.rs, lib.rs, window.rs  MOD  new actions and keys, settings at start, window frame
  tests/{settings,profiles,export,derived,session,stats}.rs  NEW
docs/superpowers/notes/2026-10-07-m4-manual-checks.md        NEW
docs/superpowers/reviews/2026-10-07-m2-visual.md             MOD  "After M4" section
```

---

### Task 1: TOML persistence, state paths and the snapshot clock

**Files:**
- Modify: `Cargo.toml`, `crates/rogg-core/Cargo.toml`
- Modify: `crates/rogg-core/src/persist.rs`, `crates/rogg-core/src/paths.rs`, `crates/rogg-core/src/error.rs`, `crates/rogg-core/src/lib.rs`
- Create: `crates/rogg-core/src/clock.rs`

**Interfaces:**
- Consumes: the M1 `persist::{load_json, save_json, with_suffix}`.
- Produces:
  - `rogg_core::persist::write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError>`
  - `load_toml<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError>`, where the `bool` means recovered
  - `save_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError>`
  - `read_json_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError>` and `read_toml_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError>` (never rename; parse errors are `CoreError::Parse`)
  - `CoreError::Parse { path: PathBuf, message: String }`
  - `rogg_core::paths::{settings_path, profiles_dir, session_path}() -> PathBuf`
  - `rogg_core::clock::local_hms(SystemTime) -> String` (`"14:32:08"`)

- [ ] **Step 1: Add the pin**

Root `Cargo.toml`, `[workspace.dependencies]`, append:

```toml
toml = "=1.1.6"
```

`crates/rogg-core/Cargo.toml`, `[dependencies]`, append:

```toml
toml.workspace = true
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo build -p rogg-core --offline`
Expected: builds; `toml 1.1.6` is already in `Cargo.lock`.

- [ ] **Step 2: Write the failing tests**

Append to the `tests` module of `crates/rogg-core/src/persist.rs`:

```rust
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Conf {
        #[serde(default)]
        name: String,
        #[serde(default)]
        size: u32,
    }

    #[test]
    fn toml_round_trips_and_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/conf.toml");
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (Conf::default(), false));
        let conf = Conf { name: "x".into(), size: 3 };
        save_toml(&path, &conf).unwrap();
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (conf, false));
        std::fs::write(&path, "name = [unclosed").unwrap();
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (Conf::default(), true));
        assert!(with_suffix(&path, ".bak").exists() && !path.exists());
    }

    #[test]
    fn strict_readers_report_and_never_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.rogg");
        std::fs::write(&path, "{ nope").unwrap();
        match read_json_strict::<State>(&path) {
            Err(CoreError::Parse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("expected a parse error, got {other:?}"),
        }
        assert!(path.exists(), "a file the user opened is never renamed");
        std::fs::write(&path, "size = \"big\"").unwrap();
        assert!(matches!(read_toml_strict::<Conf>(&path), Err(CoreError::Parse { .. })));
        assert!(matches!(read_toml_strict::<Conf>(&dir.path().join("missing.toml")), Err(CoreError::Io { .. })));
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep/f.txt");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert!(!with_suffix(&path, ".partial").exists());
    }
```

Create `crates/rogg-core/src/clock.rs` with only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn parts(s: &str) -> (u32, u32, u32) {
        let v: Vec<u32> = s.split(':').map(|p| p.parse().unwrap()).collect();
        (v[0], v[1], v[2])
    }

    #[test]
    fn formats_hh_mm_ss_and_advances_with_time() {
        let t0 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let a = local_hms(t0);
        assert_eq!(a.len(), 8);
        let (h, m, s) = parts(&a);
        assert!(h < 24 && m < 60 && s < 60);
        let (_, m1, s1) = parts(&local_hms(t0 + Duration::from_secs(61)));
        assert_eq!(s1, (s + 1) % 60);
        assert_eq!(m1, (m + 1 + (s + 1) / 60) % 60);
    }
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core persist`
Expected: compile errors (`load_toml`, `read_json_strict`, `write_atomic` and `CoreError::Parse` do not exist).

- [ ] **Step 3: Implement**

`crates/rogg-core/src/error.rs`: add a variant after `Archive`:

```rust
    /// A file the user opened or imported (`.rogg`, `.roggprofile`) is not valid for its format.
    #[error("{}: {message}", path.display())]
    Parse { path: PathBuf, message: String },
```

Replace the body of `crates/rogg-core/src/persist.rs` above `#[cfg(test)]` with:

```rust
//! Small state files: load with corruption recovery and save atomically (spec §6.6). JSON for history, recent files
//! and sessions; TOML for settings and profiles (spec §3.4).

use crate::error::CoreError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// `dir/name.json` + `".bak"` → `dir/name.json.bak`.
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes `bytes` to `<file>.partial`, then renames it over `path`; creates the parent directory. State files have
/// one writer (the UI thread), so the fixed partial name is never shared.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
    }
    let partial = with_suffix(path, ".partial");
    std::fs::write(&partial, bytes).map_err(CoreError::io(&partial))?;
    std::fs::rename(&partial, path).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        CoreError::Io { path: path.to_path_buf(), source: e }
    })
}

/// Reads `path` with `parse`. A missing file yields `T::default()`; an unparseable one (including invalid UTF-8) is
/// renamed to `<file>.bak` and yields `T::default()` with `recovered = true`; other I/O errors are returned.
fn load_with<T: Default>(path: &Path, parse: impl FnOnce(&[u8]) -> Option<T>) -> Result<(T, bool), CoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((T::default(), false)),
        Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
    };
    match parse(&bytes) {
        Some(value) => Ok((value, false)),
        None => {
            std::fs::rename(path, with_suffix(path, ".bak")).map_err(CoreError::io(path))?;
            Ok((T::default(), true))
        }
    }
}

pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    load_with(path, |bytes| serde_json::from_slice(bytes).ok())
}

pub fn load_toml<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    load_with(path, |bytes| std::str::from_utf8(bytes).ok().and_then(|s| toml::from_str(s).ok()))
}

fn invalid_data(path: &Path, e: impl std::fmt::Display) -> CoreError {
    CoreError::Io { path: path.to_path_buf(), source: std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()) }
}

/// Writes `value` as pretty JSON, atomically.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    // Not every state serializes: a path that is not valid UTF-8 has no JSON form.
    let json = serde_json::to_string_pretty(value).map_err(|e| invalid_data(path, e))?;
    write_atomic(path, json.as_bytes())
}

/// Writes `value` as TOML, atomically.
pub fn save_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let text = toml::to_string_pretty(value).map_err(|e| invalid_data(path, e))?;
    write_atomic(path, text.as_bytes())
}

fn parse_error(path: &Path, message: impl std::fmt::Display) -> CoreError {
    CoreError::Parse { path: path.to_path_buf(), message: message.to_string() }
}

/// Reads a JSON file the user chose; never renames it.
pub fn read_json_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let bytes = std::fs::read(path).map_err(CoreError::io(path))?;
    serde_json::from_slice(&bytes).map_err(|e| parse_error(path, e))
}

/// Reads a TOML file the user chose; never renames it.
pub fn read_toml_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let bytes = std::fs::read(path).map_err(CoreError::io(path))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| parse_error(path, e))?;
    toml::from_str(text).map_err(|e| parse_error(path, e))
}
```

The existing tests in `persist.rs` keep working unchanged: `save_json` still leaves no `.partial`, and `load_json` still moves bad files aside.

`crates/rogg-core/src/paths.rs`: add after `recent_path`:

```rust
pub fn settings_path() -> PathBuf {
    app_support_dir().join("settings.toml")
}

/// One `<name>.toml` per highlighting profile (spec §6.1).
pub fn profiles_dir() -> PathBuf {
    app_support_dir().join("profiles")
}

pub fn session_path() -> PathBuf {
    app_support_dir().join("session.json")
}
```

and a test:

```rust
    #[test]
    fn m4_state_paths_are_in_app_support() {
        assert!(settings_path().ends_with("Application Support/rogg/settings.toml"));
        assert!(profiles_dir().ends_with("Application Support/rogg/profiles"));
        assert!(session_path().ends_with("Application Support/rogg/session.json"));
    }
```

`crates/rogg-core/src/clock.rs`, above the tests:

```rust
//! Local wall-clock time for labels (the `Snapshot · HH:MM:SS` note of a derived tab, spec §5.6).

use std::time::{SystemTime, UNIX_EPOCH};

/// `HH:MM:SS` of `time` in the local time zone; `00:00:00` if the time cannot be converted.
pub fn local_hms(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as libc::time_t;
    // SAFETY: `localtime_r` only reads `secs` and writes into `tm`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return "00:00:00".into();
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}
```

`crates/rogg-core/src/lib.rs`: add `pub mod clock;` in alphabetical order.

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core persist paths clock && cargo test -p rogg-core && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: all pass, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/rogg-core
git commit -m "feat(core): TOML persistence, strict readers, M4 state paths and local time labels

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Settings model

**Files:**
- Create: `crates/rogg-core/src/settings.rs`
- Modify: `crates/rogg-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `load_toml`, `save_toml`; `filter::MAX_CONTEXT`; `folder::DEFAULT_SCAN_DEPTH`.
- Produces:
  - `ThemeSetting::{System, Light, Dark}` and `DensitySetting::{Compact, Comfortable}` (serde lowercase).
  - `AppearanceSettings { theme, ui_font_size: u32, log_font: String, log_font_size: u32, density }`.
  - `BehaviorSettings { wrap: bool, follow_selection: bool, context: u32, regex: bool, case_sensitive: bool, restore_session: bool, cache_limit_gb: u64, scan_depth: u32, default_profile: String }`.
  - `Settings { version, appearance, behavior }` with:
    - `Settings::load(&Path) -> Result<(Settings, bool), CoreError>` (the result is clamped);
    - `save(&self, &Path)`;
    - `clamped(self) -> Settings`;
    - `cache_limit_bytes(&self) -> u64`.
  - Constants:
    - `UI_FONT_SIZES: RangeInclusive<u32> = 11..=16`, `DEFAULT_UI_FONT_SIZE = 13`
    - `LOG_FONT_SIZES = 9..=20`, `DEFAULT_LOG_FONT_SIZE = 12`
    - `LOG_FONTS: [&str; 4] = ["SF Mono", "Menlo", "Monaco", "Courier New"]`, `DEFAULT_LOG_FONT = "SF Mono"`
    - `CACHE_LIMITS_GB: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 200, 500]`, `DEFAULT_CACHE_LIMIT_GB = 20`
    - `SCAN_DEPTHS = 1..=10`
  - `step_cache_limit(gb: u64, up: bool) -> u64`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/settings.rs` with just the tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let s = Settings::default();
        assert_eq!(s.appearance.theme, ThemeSetting::System);
        assert_eq!((s.appearance.ui_font_size, s.appearance.log_font_size), (13, 12));
        assert_eq!(s.appearance.log_font, "SF Mono");
        assert_eq!(s.appearance.density, DensitySetting::Compact);
        let b = &s.behavior;
        assert!(!b.wrap && b.follow_selection && !b.regex && !b.case_sensitive && b.restore_session);
        assert_eq!((b.context, b.cache_limit_gb, b.scan_depth), (0, 20, 4));
        assert_eq!(b.default_profile, "Default");
        assert_eq!(s.cache_limit_bytes(), crate::cache::DEFAULT_LIMIT_BYTES);
    }

    #[test]
    fn round_trip_through_toml_keeps_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut s = Settings::default();
        s.appearance.theme = ThemeSetting::Dark;
        s.appearance.log_font = "Menlo".into();
        s.appearance.density = DensitySetting::Comfortable;
        s.behavior.context = 7;
        s.behavior.default_profile = "Services".into();
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("theme = \"dark\"") && text.contains("[behavior]"), "{text}");
        assert_eq!(Settings::load(&path).unwrap(), (Settings { version: 1, ..s }, false));
    }

    #[test]
    fn corrupt_settings_recover_and_values_clamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[appearance]\nui_font_size = 40\nlog_font_size = 2\nlog_font = \"\"\n[behavior]\ncontext = 99\ncache_limit_gb = 0\nscan_depth = 50\ndefault_profile = \"\"\n").unwrap();
        let (s, recovered) = Settings::load(&path).unwrap();
        assert!(!recovered);
        assert_eq!((s.appearance.ui_font_size, s.appearance.log_font_size), (16, 9));
        assert_eq!(s.appearance.log_font, "SF Mono");
        assert_eq!((s.behavior.context, s.behavior.cache_limit_gb, s.behavior.scan_depth), (10, 1, 10));
        assert_eq!(s.behavior.default_profile, "Default");
        std::fs::write(&path, "theme = \"purple\"\n[appearance\n").unwrap();
        let (s, recovered) = Settings::load(&path).unwrap();
        assert!(recovered && s == Settings::default());
        assert!(dir.path().join("settings.toml.bak").exists());
        std::fs::write(&path, "[appearance]\ntheme = \"purple\"\n").unwrap();
        assert!(Settings::load(&path).unwrap().1, "an unknown theme is unparseable, so the file is set aside");
    }

    #[test]
    fn cache_limit_steps_through_presets() {
        assert_eq!(step_cache_limit(20, true), 50);
        assert_eq!(step_cache_limit(20, false), 10);
        assert_eq!(step_cache_limit(500, true), 500);
        assert_eq!(step_cache_limit(1, false), 1);
        assert_eq!(step_cache_limit(30, true), 50, "a hand-edited value moves to the next preset");
        assert_eq!(step_cache_limit(30, false), 20);
    }
}
```

Add `pub mod settings;` to `lib.rs`.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core settings`
Expected: compile errors (the types do not exist).

- [ ] **Step 2: Implement**

Above the tests in `settings.rs`:

```rust
//! User settings (spec §6.2), stored as `settings.toml`. Every field has a default, so a partial or older file loads;
//! values out of range are clamped.

use crate::error::CoreError;
use crate::filter::MAX_CONTEXT;
use crate::persist::{load_toml, save_toml};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use std::path::Path;

pub const UI_FONT_SIZES: RangeInclusive<u32> = 11..=16;
pub const DEFAULT_UI_FONT_SIZE: u32 = 13;
/// Log font sizes (compact density; comfortable adds 1 px as in the reference).
pub const LOG_FONT_SIZES: RangeInclusive<u32> = 9..=20;
pub const DEFAULT_LOG_FONT_SIZE: u32 = 12;
/// The curated monospace fonts that ship with macOS (spec §6.2); "Other…" picks any installed one.
pub const LOG_FONTS: [&str; 4] = ["SF Mono", "Menlo", "Monaco", "Courier New"];
pub const DEFAULT_LOG_FONT: &str = "SF Mono";
pub const CACHE_LIMITS_GB: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 200, 500];
pub const DEFAULT_CACHE_LIMIT_GB: u64 = 20;
pub const SCAN_DEPTHS: RangeInclusive<u32> = 1..=10;
pub const DEFAULT_PROFILE_NAME: &str = "Default";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSetting {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DensitySetting {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeSetting,
    pub ui_font_size: u32,
    pub log_font: String,
    pub log_font_size: u32,
    pub density: DensitySetting,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::System,
            ui_font_size: DEFAULT_UI_FONT_SIZE,
            log_font: DEFAULT_LOG_FONT.into(),
            log_font_size: DEFAULT_LOG_FONT_SIZE,
            density: DensitySetting::Compact,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorSettings {
    pub wrap: bool,
    pub follow_selection: bool,
    pub context: u32,
    pub regex: bool,
    pub case_sensitive: bool,
    pub restore_session: bool,
    pub cache_limit_gb: u64,
    pub scan_depth: u32,
    pub default_profile: String,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            wrap: false,
            follow_selection: true,
            context: 0,
            regex: false,
            case_sensitive: false,
            restore_session: true,
            cache_limit_gb: DEFAULT_CACHE_LIMIT_GB,
            scan_depth: crate::folder::DEFAULT_SCAN_DEPTH as u32,
            default_profile: DEFAULT_PROFILE_NAME.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub appearance: AppearanceSettings,
    pub behavior: BehaviorSettings,
}

/// The next preset above (or below) `gb`; a value between presets moves to the neighbouring preset.
pub fn step_cache_limit(gb: u64, up: bool) -> u64 {
    if up {
        CACHE_LIMITS_GB.iter().copied().find(|&p| p > gb).unwrap_or(CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1])
    } else {
        CACHE_LIMITS_GB.iter().rev().copied().find(|&p| p < gb).unwrap_or(CACHE_LIMITS_GB[0])
    }
}

impl Settings {
    /// Every value within its range; empty names replaced by their defaults.
    pub fn clamped(mut self) -> Self {
        let a = &mut self.appearance;
        a.ui_font_size = a.ui_font_size.clamp(*UI_FONT_SIZES.start(), *UI_FONT_SIZES.end());
        a.log_font_size = a.log_font_size.clamp(*LOG_FONT_SIZES.start(), *LOG_FONT_SIZES.end());
        if a.log_font.trim().is_empty() {
            a.log_font = DEFAULT_LOG_FONT.into();
        }
        let b = &mut self.behavior;
        b.context = b.context.min(MAX_CONTEXT);
        b.cache_limit_gb = b.cache_limit_gb.clamp(CACHE_LIMITS_GB[0], CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1]);
        b.scan_depth = b.scan_depth.clamp(*SCAN_DEPTHS.start(), *SCAN_DEPTHS.end());
        if b.default_profile.trim().is_empty() {
            b.default_profile = DEFAULT_PROFILE_NAME.into();
        }
        self
    }

    pub fn cache_limit_bytes(&self) -> u64 {
        self.behavior.cache_limit_gb << 30
    }

    /// Loads `path`; a missing file gives the defaults, an unparseable one is set aside as `.bak` (`true`).
    pub fn load(path: &Path) -> Result<(Settings, bool), CoreError> {
        let (settings, recovered) = load_toml::<Settings>(path)?;
        Ok((settings.clamped(), recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_toml(path, &Settings { version: 1, ..self.clone() })
    }
}
```

The round-trip test expects `version: 1` after loading. `Settings::default()` has `version: 0`, and the clamped load keeps whatever the file had.

- [ ] **Step 3: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core settings && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): settings model with defaults, clamping and recovery

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Profile model, built-ins and storage

**Files:**
- Create: `crates/rogg-core/src/profile.rs`
- Modify: `crates/rogg-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `save_toml`, `read_toml_strict`, `with_suffix`.
- Produces (all in `rogg_core::profile`):
  - `ColorSpec(pub String)` (serde transparent) with:
    - `ColorSpec::token(&str)`;
    - `ColorSpec::hex(u32)`, where the input is `0xRRGGBBAA` and the output is `#rrggbb` when opaque, else `#rrggbbaa`;
    - `ColorSpec::parse(&self) -> Option<ColorValue>`.
  - `ColorValue::{Token(&'static str), Rgba(u32)}`; `COLOR_TOKENS: [&str; 14]`.
  - `parse_hex(&str) -> Option<u32>`, accepting `#rgb`, `#rrggbb` and `#rrggbbaa`.
  - `derive_light(rgba: u32, background: bool) -> u32`.
  - `Panes::{Full, Filtered, Both}` with `includes(filtered: bool) -> bool` and `label() -> &'static str`.
  - `LineRule { pattern, regex, case_sensitive, match_color, line_color, row_tint, bold, panes, enabled, color: ColorSpec, light: Option<ColorSpec> }` with `LineRule::new(pattern)` and `summary() -> String`.
  - `HitEntry { fg, bg, light_fg: Option<ColorSpec>, light_bg: Option<ColorSpec> }` with `HitEntry::slot(n)`.
  - `Profile { name, rules: Vec<LineRule>, hit_palette: Vec<HitEntry> }`.
  - Built-ins: `builtin_default()`, `builtin_services()`, `builtins()`; `DEFAULT_PROFILE = "Default"`, `SERVICES_PROFILE = "Services"`, `TIMESTAMP_PATTERN`.
  - Storage:
    - `file_name(name) -> String`;
    - `load_dir(&Path) -> Result<ProfilesLoad { profiles, recovered: Vec<PathBuf> }, CoreError>`;
    - `save(&Path dir, &Profile) -> Result<PathBuf, CoreError>`;
    - `delete(&Path dir, name) -> Result<(), CoreError>`;
    - `import(&Path) -> Result<Profile, CoreError>`;
    - `export(&Path, &Profile) -> Result<(), CoreError>`;
    - `unique_name(name, taken: &[String]) -> String`.
  - Constants: `MAX_PALETTE = 12`, `TINT_ALPHA: u8 = 0x18`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/profile.rs` containing only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_tokens_and_hex() {
        assert_eq!(ColorSpec::token("error").parse(), Some(ColorValue::Token("error")));
        assert_eq!(ColorSpec("#e0c283".into()).parse(), Some(ColorValue::Rgba(0xe0c283ff)));
        assert_eq!(ColorSpec("#8f352e10".into()).parse(), Some(ColorValue::Rgba(0x8f352e10)));
        assert_eq!(ColorSpec("#abc".into()).parse(), Some(ColorValue::Rgba(0xaabbccff)));
        assert_eq!(ColorSpec("purple".into()).parse(), None);
        assert_eq!(ColorSpec("#12345".into()).parse(), None);
        assert_eq!(ColorSpec::hex(0xe0c283ff).0, "#e0c283");
        assert_eq!(ColorSpec::hex(0x8f352e10).0, "#8f352e10");
    }

    fn hsl(rgba: u32) -> (f32, f32, f32) {
        let [r, g, b, _] = rgba.to_be_bytes();
        rgb_to_hsl(r as f32 / 255., g as f32 / 255., b as f32 / 255.)
    }

    #[test]
    fn light_variants_keep_hue_and_alpha_and_stay_readable() {
        let text = derive_light(0xefaaa2ff, false);
        let (h0, _, _) = hsl(0xefaaa2ff);
        let (h1, _, l1) = hsl(text);
        assert!((h0 - h1).abs() < 2., "hue {h0} vs {h1}");
        assert!((0.25..=0.45).contains(&l1), "text lightness {l1}");
        assert_eq!(text & 0xff, 0xff);
        let bg = derive_light(0x74403780, true);
        let (_, _, l2) = hsl(bg);
        assert!((0.80..=0.92).contains(&l2), "background lightness {l2}");
        assert_eq!(bg & 0xff, 0x80, "alpha is kept");
    }

    #[test]
    fn builtins_follow_the_spec() {
        let d = builtin_default();
        assert_eq!(d.name, "Default");
        let patterns: Vec<&str> = d.rules.iter().map(|r| r.pattern.as_str()).collect();
        assert_eq!(patterns, [TIMESTAMP_PATTERN, r"\bERROR\b", r"\bWARN\b", r"\bDEBUG\b"]);
        let error = &d.rules[1];
        assert!(error.line_color && error.row_tint && !error.match_color && error.color == ColorSpec::token("error"));
        assert_eq!(d.rules[2].color, ColorSpec::token("amber"));
        assert_eq!(d.rules[3].color, ColorSpec::token("muted"));
        assert_eq!(d.hit_palette, (0..4).map(HitEntry::slot).collect::<Vec<_>>());
        let s = builtin_services();
        assert_eq!(s.rules.last().unwrap().pattern, r"request_id=\S+");
        assert!(s.rules.last().unwrap().match_color);
        assert_eq!(s.rules[1].summary(), "Line text + row tint · both panels");
        assert_eq!(s.rules.last().unwrap().summary(), "Match text · both panels");
    }

    #[test]
    fn profiles_round_trip_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = builtin_services();
        p.rules[0].light = Some(ColorSpec::hex(0x112233ff));
        p.rules[2].panes = Panes::Filtered;
        p.hit_palette.push(HitEntry { fg: ColorSpec::hex(0xffffffff), bg: ColorSpec::hex(0x000000ff), light_fg: None, light_bg: None });
        let path = save(dir.path(), &p).unwrap();
        assert_eq!(path, dir.path().join("Services.toml"));
        let loaded = load_dir(dir.path()).unwrap();
        assert_eq!(loaded.profiles.iter().find(|q| q.name == "Services"), Some(&p));
    }

    #[test]
    fn a_missing_dir_is_seeded_with_the_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = dir.path().join("profiles");
        let loaded = load_dir(&profiles).unwrap();
        assert_eq!(loaded.profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Default", "Services"]);
        assert!(profiles.join("Default.toml").exists() && profiles.join("Services.toml").exists());
        std::fs::remove_file(profiles.join("Services.toml")).unwrap();
        let again = load_dir(&profiles).unwrap();
        assert_eq!(again.profiles.len(), 1, "a deleted built-in stays deleted once the folder exists");
        std::fs::remove_file(profiles.join("Default.toml")).unwrap();
        assert_eq!(load_dir(&profiles).unwrap().profiles[0], builtin_default(), "Default is always available");
    }

    #[test]
    fn a_broken_profile_file_is_set_aside_alone() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &builtin_default()).unwrap();
        std::fs::write(dir.path().join("Mine.toml"), "name = \"Mine\"\n[[rules]]\npattern = 3\n").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
        let loaded = load_dir(dir.path()).unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.recovered, vec![dir.path().join("Mine.toml")]);
        assert!(dir.path().join("Mine.toml.bak").exists());
        assert!(dir.path().join("notes.txt").exists());
    }

    #[test]
    fn missing_fields_default_and_an_empty_name_comes_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Plain.toml"), "[[rules]]\npattern = \"x\"\n").unwrap();
        let p = load_dir(dir.path()).unwrap().profiles.into_iter().find(|p| p.name == "Plain").unwrap();
        let r = &p.rules[0];
        assert!(r.enabled && r.match_color && !r.regex && r.panes == Panes::Both);
        assert_eq!(p.hit_palette.len(), 4);
    }

    #[test]
    fn import_export_and_names() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("shared.roggprofile");
        export(&out, &builtin_services()).unwrap();
        assert_eq!(import(&out).unwrap(), builtin_services());
        std::fs::write(&out, "rules = 5").unwrap();
        assert!(matches!(import(&out), Err(CoreError::Parse { .. })));
        assert!(out.exists(), "an import never renames the user's file");
        let taken = vec!["Default".to_string(), "Services".to_string(), "Services 2".to_string()];
        assert_eq!(unique_name("Services", &taken), "Services 3");
        assert_eq!(unique_name("New", &taken), "New");
        assert_eq!(file_name("a/b: c"), "a_b_ c.toml");
        delete(dir.path(), "nothing here").unwrap();
    }
}
```

Add `pub mod profile;` to `lib.rs`.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core profile`
Expected: compile errors.

- [ ] **Step 2: Implement**

Above the tests in `profile.rs`:

```rust
//! Highlighting profiles (spec §6.1): a named, ordered list of line rules plus the filter-match palette, stored as
//! one TOML file per profile in `profiles/`, importable and exportable as `.roggprofile`.

use crate::error::CoreError;
use crate::persist::{read_toml_strict, save_toml, with_suffix};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = "Default";
pub const SERVICES_PROFILE: &str = "Services";
pub const EXPORT_EXT: &str = "roggprofile";
/// Most filter-match colours a profile holds.
pub const MAX_PALETTE: usize = 12;
/// Alpha of a row tint made from a rule colour.
pub const TINT_ALPHA: u8 = 0x18;
/// A leading `YYYY-MM-DD HH:MM:SS[.fff]` timestamp: the built-ins paint it faint, as the reference does.
pub const TIMESTAMP_PATTERN: &str = r"^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:[.,]\d{1,9})?";

/// Theme tokens a colour may name instead of a hex value; they follow the active theme. `hit-N` is the palette's
/// filter-match text colour N, `hit-N-bg` its background.
pub const COLOR_TOKENS: [&str; 14] = [
    "text", "muted", "faint", "accent", "error", "amber", "hit-0", "hit-1", "hit-2", "hit-3", "hit-0-bg", "hit-1-bg", "hit-2-bg", "hit-3-bg",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorValue {
    Token(&'static str),
    /// `0xRRGGBBAA`.
    Rgba(u32),
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` → `0xRRGGBBAA`.
pub fn parse_hex(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match hex.len() {
        3 => format!("{}ff", hex.chars().flat_map(|c| [c, c]).collect::<String>()),
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}

/// A colour as written in a profile file: a theme token or a hex value.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorSpec(pub String);

impl ColorSpec {
    pub fn token(name: &str) -> Self {
        ColorSpec(name.to_string())
    }

    pub fn hex(rgba: u32) -> Self {
        if rgba & 0xff == 0xff { ColorSpec(format!("#{:06x}", rgba >> 8)) } else { ColorSpec(format!("#{rgba:08x}")) }
    }

    pub fn parse(&self) -> Option<ColorValue> {
        let s = self.0.trim();
        COLOR_TOKENS.iter().find(|t| **t == s).map(|t| ColorValue::Token(t)).or_else(|| parse_hex(s).map(ColorValue::Rgba))
    }
}

pub(crate) fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    if max == min {
        return (0., 0., l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2. - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    (h * 60., s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0. {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
    let p = 2. * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.);
        if t < 1. / 6. {
            p + (q - p) * 6. * t
        } else if t < 0.5 {
            q
        } else if t < 2. / 3. {
            p + (q - p) * (2. / 3. - t) * 6.
        } else {
            p
        }
    };
    let h = h / 360.;
    (channel(h + 1. / 3.), channel(h), channel(h - 1. / 3.))
}

/// The light-theme variant of a dark-theme colour (spec §6.1: derived automatically, can be overridden): hue and alpha
/// are kept and the lightness mirrored — into 25–45% for text, so it reads on a light background, or 80–92% for
/// backgrounds, so marks stay soft.
pub fn derive_light(rgba: u32, background: bool) -> u32 {
    let [r, g, b, a] = rgba.to_be_bytes();
    let (h, s, l) = rgb_to_hsl(r as f32 / 255., g as f32 / 255., b as f32 / 255.);
    let l = if background { (1. - l).clamp(0.80, 0.92) } else { (1. - l).clamp(0.25, 0.45) };
    let (r, g, b) = hsl_to_rgb(h, s.min(0.8), l);
    let byte = |v: f32| (v * 255.).round().clamp(0., 255.) as u8;
    u32::from_be_bytes([byte(r), byte(g), byte(b), a])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Panes {
    Full,
    Filtered,
    #[default]
    Both,
}

impl Panes {
    pub fn includes(self, filtered: bool) -> bool {
        match self {
            Panes::Both => true,
            Panes::Full => !filtered,
            Panes::Filtered => filtered,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Panes::Full => "full log",
            Panes::Filtered => "filtered log",
            Panes::Both => "both panels",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct LineRule {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Effects (spec §6.1): colour the matched text, colour the whole line, tint the row, bold the matched text.
    pub match_color: bool,
    pub line_color: bool,
    pub row_tint: bool,
    pub bold: bool,
    pub panes: Panes,
    pub enabled: bool,
    /// Dark-theme colour (or a token, which follows both themes).
    pub color: ColorSpec,
    /// Light-theme override of a hex `color`; None derives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<ColorSpec>,
}

impl Default for LineRule {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            regex: false,
            case_sensitive: false,
            match_color: true,
            line_color: false,
            row_tint: false,
            bold: false,
            panes: Panes::Both,
            enabled: true,
            color: ColorSpec::token("accent"),
            light: None,
        }
    }
}

impl LineRule {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), ..Self::default() }
    }

    /// `Line text + row tint · both panels`, as the editor lists rules (reference `.profile-rule`).
    pub fn summary(&self) -> String {
        let effects: Vec<&str> = [("match text", self.match_color), ("line text", self.line_color), ("row tint", self.row_tint), ("bold", self.bold)]
            .into_iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name)
            .collect();
        let mut text = if effects.is_empty() { "no effect".to_string() } else { effects.join(" + ") };
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        format!("{text} · {}", self.panes.label())
    }
}

/// One filter-match colour slot (spec §6.1 `hit_palette`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct HitEntry {
    pub fg: ColorSpec,
    pub bg: ColorSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_fg: Option<ColorSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_bg: Option<ColorSpec>,
}

impl HitEntry {
    /// The theme palette's slot `n` (0–3).
    pub fn slot(n: usize) -> Self {
        Self { fg: ColorSpec(format!("hit-{}", n % 4)), bg: ColorSpec(format!("hit-{}-bg", n % 4)), light_fg: None, light_bg: None }
    }
}

impl Default for HitEntry {
    fn default() -> Self {
        Self::slot(0)
    }
}

fn default_palette() -> Vec<HitEntry> {
    (0..4).map(HitEntry::slot).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub rules: Vec<LineRule>,
    #[serde(default = "default_palette")]
    pub hit_palette: Vec<HitEntry>,
}

fn timestamp_rule() -> LineRule {
    LineRule { pattern: TIMESTAMP_PATTERN.into(), regex: true, case_sensitive: true, color: ColorSpec::token("faint"), ..LineRule::default() }
}

fn level_rule(word: &str, color: &str, tint: bool) -> LineRule {
    LineRule {
        pattern: format!(r"\b{word}\b"),
        regex: true,
        case_sensitive: true,
        match_color: false,
        line_color: true,
        row_tint: tint,
        color: ColorSpec::token(color),
        ..LineRule::default()
    }
}

/// Spec §6.1 Default: ERROR (error text + row tint), WARN (amber), DEBUG (muted); plus the faint timestamp.
pub fn builtin_default() -> Profile {
    Profile {
        name: DEFAULT_PROFILE.into(),
        rules: vec![timestamp_rule(), level_rule("ERROR", "error", true), level_rule("WARN", "amber", false), level_rule("DEBUG", "muted", false)],
        hit_palette: default_palette(),
    }
}

/// Spec §6.1 Services, the reference example: ERROR, WARN, `request_id=\S+`; plus the faint timestamp.
pub fn builtin_services() -> Profile {
    let request_id = LineRule { pattern: r"request_id=\S+".into(), regex: true, case_sensitive: true, color: ColorSpec::token("hit-2"), ..LineRule::default() };
    Profile {
        name: SERVICES_PROFILE.into(),
        rules: vec![timestamp_rule(), level_rule("ERROR", "error", true), level_rule("WARN", "amber", false), request_id],
        hit_palette: default_palette(),
    }
}

pub fn builtins() -> Vec<Profile> {
    vec![builtin_default(), builtin_services()]
}

/// The file a profile is stored in: its name with every character other than a letter, digit, space, `-` or `_`
/// replaced by `_`, plus `.toml`.
pub fn file_name(name: &str) -> String {
    let stem: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).collect();
    format!("{}.toml", if stem.trim().is_empty() { "profile" } else { stem.as_str() })
}

/// `name`, or `name 2`, `name 3`… — the first one not in `taken`.
pub fn unique_name(name: &str, taken: &[String]) -> String {
    let name = if name.trim().is_empty() { "Profile" } else { name.trim() };
    if !taken.iter().any(|t| t == name) {
        return name.to_string();
    }
    (2..).map(|n| format!("{name} {n}")).find(|candidate| !taken.contains(candidate)).unwrap_or_default()
}

pub struct ProfilesLoad {
    /// Sorted by name with Default first; Default is always present.
    pub profiles: Vec<Profile>,
    /// Files that could not be read and were moved to `<file>.bak`.
    pub recovered: Vec<PathBuf>,
}

/// Reads every `*.toml` in `dir`. A missing `dir` is created and seeded with the built-ins; an unparseable file is
/// moved to `.bak` (spec §6.6); a file without a name takes its file stem; duplicate names keep the first file
/// (by file name); a missing Default is supplied from the built-in (not written back).
pub fn load_dir(dir: &Path) -> Result<ProfilesLoad, CoreError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
        for p in builtins() {
            save(dir, &p)?;
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(CoreError::io(dir))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml") && p.is_file())
        .collect();
    files.sort();
    let mut profiles: Vec<Profile> = Vec::new();
    let mut recovered = Vec::new();
    for path in files {
        let parsed = std::fs::read(&path)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| toml::from_str::<Profile>(&s).ok());
        match parsed {
            Some(mut p) => {
                if p.name.trim().is_empty() {
                    p.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                }
                if p.hit_palette.is_empty() {
                    p.hit_palette = default_palette();
                }
                p.hit_palette.truncate(MAX_PALETTE);
                if !profiles.iter().any(|q| q.name == p.name) {
                    profiles.push(p);
                }
            }
            None => {
                std::fs::rename(&path, with_suffix(&path, ".bak")).map_err(CoreError::io(&path))?;
                recovered.push(path);
            }
        }
    }
    if !profiles.iter().any(|p| p.name == DEFAULT_PROFILE) {
        profiles.push(builtin_default());
    }
    profiles.sort_by(|a, b| (a.name != DEFAULT_PROFILE, a.name.to_lowercase()).cmp(&(b.name != DEFAULT_PROFILE, b.name.to_lowercase())));
    Ok(ProfilesLoad { profiles, recovered })
}

/// Writes `profile` to `dir/<file_name>`; returns the path.
pub fn save(dir: &Path, profile: &Profile) -> Result<PathBuf, CoreError> {
    let path = dir.join(file_name(&profile.name));
    save_toml(&path, profile)?;
    Ok(path)
}

/// Removes the file of the profile named `name` (nothing to do when it is gone).
pub fn delete(dir: &Path, name: &str) -> Result<(), CoreError> {
    let path = dir.join(file_name(name));
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(CoreError::Io { path, source: e }),
        _ => Ok(()),
    }
}

/// Reads a `.roggprofile` the user picked; a missing name comes from the file stem. Never renames the file.
pub fn import(path: &Path) -> Result<Profile, CoreError> {
    let mut p: Profile = read_toml_strict(path)?;
    if p.name.trim().is_empty() {
        p.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    }
    if p.hit_palette.is_empty() {
        p.hit_palette = default_palette();
    }
    p.hit_palette.truncate(MAX_PALETTE);
    Ok(p)
}

pub fn export(path: &Path, profile: &Profile) -> Result<(), CoreError> {
    save_toml(path, profile)
}
```

`file_name("a/b: c")` turns both `/` and `:` into `_`, giving `"a_b_ c.toml"`, which is what the test expects.

- [ ] **Step 3: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core profile && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): highlighting profile model, built-ins, storage, import and export

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Profile matching per painted row

**Files:**
- Create: `crates/rogg-core/src/style.rs`
- Modify: `crates/rogg-core/src/highlight.rs` (share `Finder` and `snap_to_char_boundaries`), `crates/rogg-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 `Profile`, `LineRule`; M1 `highlight::{Finder, snap_to_char_boundaries, MAX_SPANS_PER_TERM}`.
- Produces:
  - `rogg_core::style::RuleError { rule: usize, message: String }`.
  - `CompiledProfile::compile(&Profile) -> (CompiledProfile, Vec<RuleError>)`, `CompiledProfile::is_empty()` and `CompiledProfile::style(&self, display: &str, filtered: bool) -> LineStyle`.
  - `LineStyle { line_color: Option<usize>, tint: Option<usize>, match_spans: Vec<(Range<usize>, usize)>, bold: Vec<Range<usize>> }`. The `usize` values are rule indices into `Profile::rules`.

- [ ] **Step 1: Share the finder**

In `crates/rogg-core/src/highlight.rs`:
- change `enum Finder` to `pub(crate) enum Finder`;
- change `fn build` to `pub(crate) fn build` and `fn ranges` to `pub(crate) fn ranges`;
- change `fn snap_to_char_boundaries` to `pub(crate) fn snap_to_char_boundaries`.

Add to `impl Finder`:

```rust
    /// Whether the pattern matches anywhere in `display` (an empty match counts: `^` matches every line).
    pub(crate) fn is_match(&self, display: &str) -> bool {
        match self {
            Finder::Plain(ac) => ac.is_match(display),
            Finder::Regex(re) => re.is_match(display.as_bytes()),
        }
    }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/rogg-core/src/style.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{LineRule, Panes, Profile, builtin_default};

    fn profile(rules: Vec<LineRule>) -> Profile {
        Profile { name: "t".into(), rules, hit_palette: Vec::new() }
    }

    fn rule(pattern: &str, f: impl FnOnce(&mut LineRule)) -> LineRule {
        let mut r = LineRule::new(pattern);
        f(&mut r);
        r
    }

    #[test]
    fn the_first_matching_rule_wins_for_each_effect() {
        let p = profile(vec![
            rule("disk", |r| {
                r.match_color = false;
                r.line_color = true;
            }),
            rule("full", |r| {
                r.match_color = false;
                r.line_color = true;
                r.row_tint = true;
            }),
        ]);
        let (c, errors) = CompiledProfile::compile(&p);
        assert!(errors.is_empty());
        let s = c.style("disk full", false);
        assert_eq!((s.line_color, s.tint), (Some(0), Some(1)), "line colour from rule 0, tint from the first rule with a tint");
        assert_eq!(c.style("nothing", false), LineStyle::default());
    }

    #[test]
    fn match_spans_prefer_earlier_rules_and_bold_is_separate() {
        let p = profile(vec![rule("timeout", |_| {}), rule("out", |r| r.bold = true), rule("time", |r| r.match_color = false)]);
        let (c, _) = CompiledProfile::compile(&p);
        let s = c.style("a timeout, out", false);
        assert_eq!(s.match_spans, vec![(2..9, 0), (11..14, 1)]);
        assert_eq!(s.bold, vec![6..9, 11..14]);
    }

    #[test]
    fn panes_disabled_and_empty_rules_are_respected() {
        let p = profile(vec![
            rule("a", |r| r.panes = Panes::Filtered),
            rule("b", |r| r.enabled = false),
            rule("", |_| {}),
            rule("c", |r| r.panes = Panes::Full),
        ]);
        let (c, _) = CompiledProfile::compile(&p);
        assert_eq!(c.style("abc", true).match_spans, vec![(0..1, 0)]);
        assert_eq!(c.style("abc", false).match_spans, vec![(2..3, 3)]);
    }

    #[test]
    fn invalid_rules_are_skipped_and_reported() {
        let p = profile(vec![rule("(oops", |r| r.regex = true), rule("ok", |_| {})]);
        let (c, errors) = CompiledProfile::compile(&p);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, 0);
        assert!(errors[0].message.contains("oops"), "{}", errors[0].message);
        assert_eq!(c.style("ok (oops", false).match_spans, vec![(0..2, 1)]);
    }

    #[test]
    fn case_and_regex_flags_apply() {
        let p = profile(vec![rule("error", |r| r.case_sensitive = true), rule(r"\d+ms", |r| r.regex = true)]);
        let (c, _) = CompiledProfile::compile(&p);
        assert_eq!(c.style("ERROR error 250ms", false).match_spans, vec![(6..11, 0), (12..17, 1)]);
    }

    #[test]
    fn the_default_profile_styles_a_reference_line() {
        let (c, errors) = CompiledProfile::compile(&builtin_default());
        assert!(errors.is_empty());
        let s = c.style("2026-10-05 14:32:08.142  ERROR  client       upstream timeout", false);
        assert_eq!(s.match_spans, vec![(0..23, 0)], "the timestamp");
        assert_eq!((s.line_color, s.tint), (Some(1), Some(1)), "ERROR colours the line and tints the row");
        let warn = c.style("2026-10-05 14:32:08.142  WARN   client  retry", false);
        assert_eq!((warn.line_color, warn.tint), (Some(2), None));
        assert_eq!(c.style("ERRORS were few", false).line_color, None, "whole words only");
    }

    #[test]
    fn spans_stay_on_char_boundaries_and_are_capped() {
        let p = profile(vec![rule("é", |_| {}), rule(r"(?-u)\xA9", |r| r.regex = true)]);
        let (c, _) = CompiledProfile::compile(&p);
        for (r, _) in c.style("aéb", false).match_spans {
            assert!("aéb".is_char_boundary(r.start) && "aéb".is_char_boundary(r.end));
        }
        let line = "a ".repeat(10_000);
        let (c, _) = CompiledProfile::compile(&profile(vec![rule("a", |_| {})]));
        let covered: usize = c.style(&line, false).match_spans.iter().map(|(r, _)| r.len()).sum();
        assert_eq!(covered, crate::highlight::MAX_SPANS_PER_TERM);
    }
}
```

Add `pub mod style;` to `lib.rs`.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core style`
Expected: compile errors.

- [ ] **Step 3: Implement**

Above the tests in `style.rs`:

```rust
//! Applying a highlighting profile to a painted row (spec §6.1). Rules run top to bottom over the row's display text;
//! for each effect the first matching enabled rule wins: one rule colours the line, one tints the row, and where match
//! colours overlap the earlier rule keeps the bytes. Only painted rows are styled, so the cost is per visible line.

use crate::highlight::{Finder, MAX_SPANS_PER_TERM, snap_to_char_boundaries};
use crate::profile::{LineRule, Profile};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleError {
    pub rule: usize,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineStyle {
    /// The rule whose colour paints the whole line.
    pub line_color: Option<usize>,
    /// The rule whose colour tints the row.
    pub tint: Option<usize>,
    /// Byte ranges painted in a rule's colour (ascending, non-overlapping).
    pub match_spans: Vec<(Range<usize>, usize)>,
    /// Byte ranges drawn bold (ascending, non-overlapping).
    pub bold: Vec<Range<usize>>,
}

struct Compiled {
    index: usize,
    rule: LineRule,
    finder: Finder,
}

pub struct CompiledProfile {
    rules: Vec<Compiled>,
}

fn runs<T: Copy + PartialEq>(owner: &[T], none: T) -> Vec<(Range<usize>, T)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < owner.len() {
        let value = owner[i];
        let start = i;
        while i < owner.len() && owner[i] == value {
            i += 1;
        }
        if value != none {
            out.push((start..i, value));
        }
    }
    out
}

impl CompiledProfile {
    /// Compiles every enabled rule with a pattern; rules that do not compile are skipped and reported.
    pub fn compile(profile: &Profile) -> (Self, Vec<RuleError>) {
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for (index, rule) in profile.rules.iter().enumerate() {
            if !rule.enabled || rule.pattern.is_empty() {
                continue;
            }
            match Finder::build(&rule.pattern, rule.regex, rule.case_sensitive) {
                Ok(finder) => rules.push(Compiled { index, rule: rule.clone(), finder }),
                Err(e) => errors.push(RuleError { rule: index, message: e.to_string() }),
            }
        }
        (Self { rules }, errors)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The style of one row's display text in the full (`filtered == false`) or the filtered log.
    pub fn style(&self, display: &str, filtered: bool) -> LineStyle {
        let mut style = LineStyle::default();
        if display.is_empty() {
            return style;
        }
        let mut color_owner: Option<Vec<u32>> = None;
        let mut bold_owner: Option<Vec<bool>> = None;
        for c in &self.rules {
            let r = &c.rule;
            if !r.panes.includes(filtered) {
                continue;
            }
            let wants_line = (r.line_color && style.line_color.is_none()) || (r.row_tint && style.tint.is_none());
            if !(wants_line || r.match_color || r.bold) || !c.finder.is_match(display) {
                continue;
            }
            if r.line_color && style.line_color.is_none() {
                style.line_color = Some(c.index);
            }
            if r.row_tint && style.tint.is_none() {
                style.tint = Some(c.index);
            }
            if r.match_color || r.bold {
                for range in c.finder.ranges(display).filter(|m| !m.is_empty()).take(MAX_SPANS_PER_TERM) {
                    let range = snap_to_char_boundaries(display, range);
                    if r.match_color {
                        let owner = color_owner.get_or_insert_with(|| vec![u32::MAX; display.len()]);
                        for slot in &mut owner[range.clone()] {
                            if *slot == u32::MAX {
                                *slot = c.index as u32;
                            }
                        }
                    }
                    if r.bold {
                        bold_owner.get_or_insert_with(|| vec![false; display.len()])[range].fill(true);
                    }
                }
            }
        }
        if let Some(owner) = color_owner {
            style.match_spans = runs(&owner, u32::MAX).into_iter().map(|(r, i)| (r, i as usize)).collect();
        }
        if let Some(owner) = bold_owner {
            style.bold = runs(&owner, false).into_iter().map(|(r, _)| r).collect();
        }
        style
    }
}
```

`Finder::build` returns `CoreError::InvalidRegex { term, message }`, whose display is `invalid regex in term `(oops`: …`, so the error message contains the pattern, as the test expects.

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core style highlight && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): apply highlighting profiles to painted rows

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Session model and `.rogg` files

**Files:**
- Create: `crates/rogg-core/src/session.rs`
- Modify: `crates/rogg-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `load_json`, `save_json`, `read_json_strict`; M3 `SourceRef`; M1 `FilterSpec`, `MAX_CONTEXT`.
- Produces (in `rogg_core::session`):
  - `SESSION_VERSION = 1`, `ROGG_EXT = "rogg"`, `DEFAULT_SPLIT: f32 = 1.1 / 2.1`.
  - `WindowState { x, y, width, height: f32, maximized: bool, sidebar_visible: bool, sidebar_width: Option<f32> }`.
  - `SessionSource::{File { path }, Member { source: SourceRef }, Derived(Box<DerivedSpec>)}`, serde internally tagged `"kind"`.
  - `DerivedSpec { parent: SessionSource, filter: FilterSpec, pins: Vec<u64>, lines: Option<Vec<u64>>, snapshot: Option<String> }`.
  - `TabState { source, profile: Option<String>, filter: FilterSpec, pins: Vec<u64>, selection: Option<u64>, top: [Option<u64>; 2], split: f32, wrap: [bool; 2], follow: bool }` with `TabState::new(source)`.
  - `Session { version, window: Option<WindowState>, tabs: Vec<TabState>, active: usize }` with:
    - `load(&Path) -> Result<(Session, bool), CoreError>`;
    - `save(&self, &Path)`;
    - `read_rogg(&Path) -> Result<Session, CoreError>`, which is strict;
    - `sanitized(self) -> Session`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/session.rs` containing:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::source::SourceRef;

    fn sample() -> Session {
        let member = SourceRef::member(SourceRef::File("/b/incident.zip".into()), "logs/api.log");
        let filter = FilterSpec { terms: vec![Term::include("error"), Term::new("health", TermState::Exclude)], regex: false, case_sensitive: true, context: 2 };
        let derived = DerivedSpec { parent: SessionSource::Member { source: member.clone() }, filter: filter.clone(), pins: vec![4], lines: None, snapshot: None };
        let mut file = TabState::new(SessionSource::File { path: "/var/log/app.log".into() });
        file.filter = filter;
        file.pins = vec![1, 9];
        file.selection = Some(12);
        file.top = [Some(3), None];
        file.wrap = [true, false];
        file.follow = false;
        file.profile = Some("Services".into());
        Session {
            version: SESSION_VERSION,
            window: Some(WindowState { x: 10., y: 20., width: 1440., height: 900., maximized: false, sidebar_visible: true, sidebar_width: Some(240.) }),
            tabs: vec![file, TabState::new(SessionSource::Member { source: member }), TabState::new(SessionSource::Derived(Box::new(derived)))],
            active: 2,
        }
    }

    #[test]
    fn sessions_round_trip_through_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        sample().save(&path).unwrap();
        assert_eq!(Session::load(&path).unwrap(), (sample(), false));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"kind\": \"derived\""), "{text}");
        assert_eq!(Session::read_rogg(&path).unwrap(), sample(), ".rogg uses the same schema");
    }

    #[test]
    fn corrupt_session_is_set_aside_but_rogg_files_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(&path, "{\"tabs\": [").unwrap();
        assert_eq!(Session::load(&path).unwrap(), (Session::default(), true));
        assert!(dir.path().join("session.json.bak").exists());
        let rogg = dir.path().join("x.rogg");
        std::fs::write(&rogg, "{\"tabs\": [").unwrap();
        assert!(matches!(Session::read_rogg(&rogg), Err(CoreError::Parse { .. })));
        assert!(rogg.exists());
    }

    #[test]
    fn loading_sanitizes_out_of_range_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut s = sample();
        s.active = 9;
        s.tabs[0].filter.context = 99;
        s.tabs[0].split = 7.;
        s.save(&path).unwrap();
        let (loaded, _) = Session::load(&path).unwrap();
        assert_eq!(loaded.active, 2);
        assert_eq!(loaded.tabs[0].filter.context, crate::filter::MAX_CONTEXT);
        assert!((0.05..=0.95).contains(&loaded.tabs[0].split));
        let empty = Session { active: 3, ..Session::default() }.sanitized();
        assert_eq!(empty.active, 0);
    }

    #[test]
    fn minimal_tab_entries_load_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("min.rogg");
        std::fs::write(&path, r#"{"tabs":[{"source":{"kind":"file","path":"/x.log"}}]}"#).unwrap();
        let s = Session::read_rogg(&path).unwrap();
        let t = &s.tabs[0];
        assert!(t.follow && t.pins.is_empty() && t.selection.is_none() && t.filter == FilterSpec::default());
        assert!((t.split - DEFAULT_SPLIT).abs() < 1e-6);
    }
}
```

Add `pub mod session;` to `lib.rs`.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core session`
Expected: compile errors.

- [ ] **Step 2: Implement**

`FilterSpec` derives `Serialize`/`Deserialize` without field defaults. The test's minimal tab omits `filter`, which works because `TabState.filter` has `#[serde(default)]`.

Above the tests:

```rust
//! Session restore and `.rogg` files (spec §6.4, §6.5): the window and every tab's state as JSON. `session.json` is
//! written on quit and every 30 s; a `.rogg` file is the same schema, exported by the user and restored when opened.

use crate::error::CoreError;
use crate::filter::{FilterSpec, MAX_CONTEXT};
use crate::persist::{load_json, read_json_strict, save_json};
use crate::source::SourceRef;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SESSION_VERSION: u32 = 1;
pub const ROGG_EXT: &str = "rogg";
/// The panes' default split (`rogg_ui::layout::MainWindowParts::DEFAULT_SPLIT`).
pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;

fn yes() -> bool {
    true
}

fn default_split() -> f32 {
    DEFAULT_SPLIT
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default = "yes")]
    pub sidebar_visible: bool,
    #[serde(default)]
    pub sidebar_width: Option<f32>,
}

/// Where a restored tab's text comes from. Streams (stdin, FIFOs) and in-memory texts are never stored (spec §6.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SessionSource {
    File { path: PathBuf },
    Member { source: SourceRef },
    Derived(Box<DerivedSpec>),
}

/// An "Open filtered as tab" tab (spec §4.9, §6.4): rebuilt from its parent and filter, or — when the parent was live —
/// from the explicit list of the parent's 0-based lines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedSpec {
    pub parent: SessionSource,
    pub filter: FilterSpec,
    #[serde(default)]
    pub pins: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<u64>>,
    /// `HH:MM:SS` when the parent was live (the `Snapshot · …` note).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabState {
    pub source: SessionSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub filter: FilterSpec,
    /// Pinned lines, 0-based.
    #[serde(default)]
    pub pins: Vec<u64>,
    /// Selected display number.
    #[serde(default)]
    pub selection: Option<u64>,
    /// Display number of the first visible line of the full and the filtered log.
    #[serde(default)]
    pub top: [Option<u64>; 2],
    #[serde(default = "default_split")]
    pub split: f32,
    #[serde(default)]
    pub wrap: [bool; 2],
    #[serde(default = "yes")]
    pub follow: bool,
}

impl TabState {
    pub fn new(source: SessionSource) -> Self {
        Self { source, profile: None, filter: FilterSpec::default(), pins: Vec::new(), selection: None, top: [None, None], split: DEFAULT_SPLIT, wrap: [false, false], follow: true }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub window: Option<WindowState>,
    #[serde(default)]
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active: usize,
}

fn sanitize_filter(filter: &mut FilterSpec) {
    filter.context = filter.context.min(MAX_CONTEXT);
}

fn sanitize_source(source: &mut SessionSource) {
    if let SessionSource::Derived(spec) = source {
        sanitize_filter(&mut spec.filter);
        sanitize_source(&mut spec.parent);
    }
}

impl Session {
    /// Values a hand-edited file may hold out of range are brought back in: the active index, context radii, splits.
    pub fn sanitized(mut self) -> Self {
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        for tab in &mut self.tabs {
            sanitize_filter(&mut tab.filter);
            sanitize_source(&mut tab.source);
            if !tab.split.is_finite() {
                tab.split = DEFAULT_SPLIT;
            }
            tab.split = tab.split.clamp(0.05, 0.95);
        }
        self
    }

    /// `session.json`: a missing file is an empty session; an unparseable one is set aside as `.bak` (`true`).
    pub fn load(path: &Path) -> Result<(Session, bool), CoreError> {
        let (session, recovered) = load_json::<Session>(path)?;
        Ok((session.sanitized(), recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &Session { version: SESSION_VERSION, ..self.clone() })
    }

    /// A `.rogg` file the user opened: errors are reported, the file is never renamed.
    pub fn read_rogg(path: &Path) -> Result<Session, CoreError> {
        read_json_strict::<Session>(path).map(Session::sanitized)
    }
}
```

`sessions_round_trip_through_json` passes a `sample()` with `version: SESSION_VERSION`, so the comparison holds.

- [ ] **Step 3: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core session && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): session and .rogg schema with recovery and sanitising

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 6: Derived sources and filtered-line export (core)

**Files:**
- Create: `crates/rogg-core/src/derived.rs`, `crates/rogg-core/src/export.rs`
- Modify: `crates/rogg-core/src/cache.rs` (settable limit, `derived_path`, derived files swept like streams), `crates/rogg-core/src/view.rs` (`is_anchor`), `crates/rogg-core/src/lib.rs`

**Interfaces:**
- Consumes:
  - Task 5 `DerivedSpec`, `SessionSource`.
  - M3:
    - `open_source`, `OpenedSource { text, leases, inflating, watch }`, `InflatePoll::{Running, Done(Result<(), CoreError>)}`;
    - `Cache::{write_entry, streams_dir, dir}`, `cache::partial_path`.
  - M1:
    - `run_filter`, `CompiledFilter`, `FilteredView`, `DEFAULT_CHUNK_LINES`;
    - `stats::{group_digits, noun}`.
- Produces:
  - `Cache::set_limit(&self, bytes: u64)`; `Cache::limit()` now reads an atomic.
  - `Cache::derived_path(&self) -> PathBuf`, which returns `streams/derived-<pid>-<n>.log`. `sweep_streams` also removes dead processes' `derived-*` files.
  - `FilteredView::is_anchor(&self, line: u64) -> bool`: true for a matched or pinned line.
  - `rogg_core::derived`:
    - `Derived { path: PathBuf, text: LogText, numbers: Vec<u64> }`;
    - `view_lines(text, spec, pins, cancel) -> Result<Vec<u64>, CoreError>`;
    - `materialize(text, lines, number_of: impl Fn(u64) -> u64, cache, cancel, progress: impl FnMut(u64, u64)) -> Result<Derived, CoreError>`;
    - `open_full(src: &SourceRef, cache, cancel) -> Result<(LogText, Vec<CacheLease>), CoreError>`;
    - `rebuild(spec: &DerivedSpec, cache: &Arc<Cache>, cancel, progress) -> Result<Rebuilt { derived: Derived, leases: Vec<CacheLease> }, CoreError>`.
  - `rogg_core::export`:
    - `ExportOptions { context: bool, gaps: bool, numbers: bool }`;
    - `export_lines(text, view, number_of, options, dst, cancel, progress) -> Result<u64, CoreError>`, which returns the number of log lines written;
    - `gap_marker(omitted: u64) -> String`.

- [ ] **Step 1: Write the failing tests**

Append to the tests of `crates/rogg-core/src/cache.rs`:

```rust
    #[test]
    fn the_limit_can_change_and_derived_files_are_swept_like_streams() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), 10);
        cache.set_limit(1 << 30);
        assert_eq!(cache.limit(), 1 << 30);
        let a = cache.derived_path();
        let b = cache.derived_path();
        assert_ne!(a, b);
        assert!(a.starts_with(cache.streams_dir()));
        assert!(a.file_name().unwrap().to_string_lossy().starts_with(&format!("derived-{}-", std::process::id())));
        std::fs::create_dir_all(cache.streams_dir()).unwrap();
        let dead = dead_pid();
        std::fs::write(cache.streams_dir().join(format!("derived-{dead}-0.log")), "x").unwrap();
        std::fs::write(&a, "x").unwrap();
        assert_eq!(cache.sweep_streams(), 1);
        assert!(a.exists());
        assert_eq!(stream_owner("derived-42-7.log"), Some(42));
    }
```

Append to the tests of `crates/rogg-core/src/view.rs`:

```rust
    #[test]
    fn anchors_are_matches_and_pins() {
        let v = reference(1);
        assert!(v.is_anchor(12) && v.is_anchor(7));
        assert!(!v.is_anchor(8), "a context line");
    }
```

Create `crates/rogg-core/tests/derived.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::cache::Cache;
use rogg_core::derived::{materialize, rebuild, view_lines};
use rogg_core::filter::FilterSpec;
use rogg_core::session::{DerivedSpec, SessionSource};
use rogg_core::text::LogText;
use std::collections::BTreeSet;

fn lines(text: &LogText) -> Vec<String> {
    (0..text.line_count()).map(|i| text.line_text(i).into_owned()).collect()
}

#[test]
fn view_lines_are_matches_pins_and_context() {
    let text = LogText::from_bytes(b"a0\nb1\na2\nb3\nb4\nb5\na6\nb7".to_vec());
    let spec = FilterSpec { context: 1, ..FilterSpec::from_expression("a") };
    let shown = view_lines(&text, &spec, &BTreeSet::from([4]), &CancelToken::new()).unwrap();
    assert_eq!(shown, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    let no_context = view_lines(&text, &FilterSpec::from_expression("a"), &BTreeSet::from([4]), &CancelToken::new()).unwrap();
    assert_eq!(no_context, vec![0, 2, 4, 6]);
}

#[test]
fn materialize_copies_lines_and_keeps_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let text = LogText::from_bytes(b"zero\r\none\ntwo\nthree".to_vec());
    let mut seen = Vec::new();
    let d = materialize(&text, &[1, 3], |l| 1000 + l, &cache, &CancelToken::new(), |done, total| seen.push((done, total))).unwrap();
    assert_eq!(lines(&d.text), ["one", "three"]);
    assert_eq!(d.numbers, vec![1001, 1003]);
    assert!(d.path.starts_with(cache.streams_dir()));
    assert_eq!(seen.last(), Some(&(2, 2)));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(materialize(&text, &[0], |l| l, &cache, &cancel, |_, _| {}), Err(rogg_core::CoreError::Cancelled)));
    let leftovers = std::fs::read_dir(cache.streams_dir()).unwrap().count();
    assert_eq!(leftovers, 1, "only the finished derived file; the cancelled one left no partial");
}

#[test]
fn nested_rebuild_composes_original_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let log = dir.path().join("app.log");
    std::fs::write(&log, "x error a\nok\nerror b\nfine\nerror a again\n").unwrap();
    let first = DerivedSpec { parent: SessionSource::File { path: log.clone() }, filter: FilterSpec::from_expression("error"), pins: vec![], lines: None, snapshot: None };
    let nested = DerivedSpec { parent: SessionSource::Derived(Box::new(first)), filter: FilterSpec::from_expression("a"), pins: vec![], lines: None, snapshot: None };
    let r = rebuild(&nested, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(lines(&r.derived.text), ["x error a", "error a again"]);
    assert_eq!(r.derived.numbers, vec![1, 5], "display numbers of the original file");
    let derived_files = std::fs::read_dir(cache.streams_dir()).unwrap().count();
    assert_eq!(derived_files, 1, "the intermediate level's file is removed once the nested one is written");
}

#[test]
fn explicit_lines_win_over_the_filter_and_missing_parents_fail() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let log = dir.path().join("live.log");
    std::fs::write(&log, "a\nb\nc\n").unwrap();
    let spec = DerivedSpec { parent: SessionSource::File { path: log.clone() }, filter: FilterSpec::from_expression("zzz"), pins: vec![], lines: Some(vec![2, 0, 9]), snapshot: Some("14:32:08".into()) };
    let r = rebuild(&spec, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(lines(&r.derived.text), ["a", "c"], "sorted, and lines past the end are dropped");
    assert_eq!(r.derived.numbers, vec![1, 3]);
    let gone = DerivedSpec { parent: SessionSource::File { path: dir.path().join("gone.log") }, ..spec };
    assert!(matches!(rebuild(&gone, &cache, &CancelToken::new(), |_, _| {}), Err(rogg_core::CoreError::Io { .. })));
}

#[test]
fn rebuild_reads_a_whole_gz_parent() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let body: String = (0..50_000).map(|i| format!("line {i} {}\n", if i % 10_000 == 0 { "hit" } else { "-" })).collect();
    let gz = dir.path().join("big.log.gz");
    let mut enc = flate2::write::GzEncoder::new(std::fs::File::create(&gz).unwrap(), flate2::Compression::fast());
    std::io::Write::write_all(&mut enc, body.as_bytes()).unwrap();
    enc.finish().unwrap();
    let spec = DerivedSpec { parent: SessionSource::File { path: gz }, filter: FilterSpec::from_expression("hit"), pins: vec![], lines: None, snapshot: None };
    let r = rebuild(&spec, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(r.derived.numbers, vec![1, 10_001, 20_001, 30_001, 40_001]);
}
```

`flate2` is a regular dependency of `rogg-core`, so integration tests can use it.

Create `crates/rogg-core/tests/export.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::export::{ExportOptions, export_lines};
use rogg_core::text::LogText;
use rogg_core::view::FilteredView;
use std::collections::BTreeSet;
use std::sync::Arc;

fn fixture() -> (LogText, FilteredView) {
    let text = LogText::from_bytes((0..10).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n").into_bytes());
    let view = FilteredView::build(Arc::new(vec![2, 6]), &BTreeSet::from([8]), 1, 10, |_| false);
    (text, view)
}

fn export(options: ExportOptions) -> (u64, String) {
    let (text, view) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("out.log");
    let n = export_lines(&text, &view, |l| l + 1, options, &dst, &CancelToken::new(), |_, _| {}).unwrap();
    (n, std::fs::read_to_string(&dst).unwrap())
}

#[test]
fn exports_context_gaps_and_numbers() {
    let (n, out) = export(ExportOptions { context: true, gaps: true, numbers: true });
    assert_eq!(n, 8);
    assert_eq!(out, " 2  l1\n 3  l2\n 4  l3\n··· 1 line omitted\n 6  l5\n 7  l6\n 8  l7\n 9  l8\n10  l9\n");
}

#[test]
fn without_context_only_matches_and_pins_are_written() {
    let (n, out) = export(ExportOptions { context: false, gaps: true, numbers: false });
    assert_eq!(n, 3);
    assert_eq!(out, "l2\n··· 3 lines omitted\nl6\n··· 1 line omitted\nl8\n");
    let (_, plain) = export(ExportOptions { context: false, gaps: false, numbers: false });
    assert_eq!(plain, "l2\nl6\nl8\n");
}

#[test]
fn export_failure_leaves_no_partial() {
    use std::os::unix::fs::PermissionsExt;
    let (text, view) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = export_lines(&text, &view, |l| l + 1, ExportOptions { context: true, gaps: false, numbers: false }, &locked.join("out.log"), &CancelToken::new(), |_, _| {});
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(rogg_core::CoreError::Io { .. })));
    assert_eq!(std::fs::read_dir(&locked).unwrap().count(), 0);
    let cancel = CancelToken::new();
    cancel.cancel();
    let dst = dir.path().join("c.log");
    assert!(matches!(export_lines(&text, &view, |l| l, ExportOptions { context: true, gaps: true, numbers: true }, &dst, &cancel, |_, _| {}), Err(rogg_core::CoreError::Cancelled)));
    assert!(!dst.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "only `locked`: no partial is left");
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test derived --test export`
Expected: compile errors.

- [ ] **Step 2: Implement the cache and view changes**

`crates/rogg-core/src/cache.rs`:
- Change the field `limit: u64` to `limit: std::sync::atomic::AtomicU64`.
- `new` stores `AtomicU64::new(limit)`.
- `limit()` returns `self.limit.load(Ordering::Relaxed)`.
- `evict()` calls `self.evict_to(self.limit())`.

Add:

```rust
    /// Changes the size limit (settings, spec §6.2); the next `evict` trims to it.
    pub fn set_limit(&self, bytes: u64) {
        self.limit.store(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    /// A fresh path for a derived tab's text (spec §4.9): `streams/derived-<pid>-<n>.log`. Like stream captures it is
    /// never evicted while the app runs, deleted when its tab closes, and swept once its process is gone.
    pub fn derived_path(&self) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.streams_dir().join(format!("derived-{}-{n}.log", std::process::id()))
    }
```

Change `stream_owner` to accept both prefixes:

```rust
/// The writer of a `stream-<pid>-<n>.log` capture or a `derived-<pid>-<n>.log` tab text.
fn stream_owner(name: &str) -> Option<i32> {
    name.strip_prefix("stream-").or_else(|| name.strip_prefix("derived-"))?.split('-').next()?.parse().ok()
}
```

`crates/rogg-core/src/view.rs`, in `impl FilteredView`:

```rust
    /// A matched or pinned line (shown for itself, not as context).
    pub fn is_anchor(&self, line: u64) -> bool {
        self.matches.binary_search(&line).is_ok() || self.pins.contains(&line)
    }
```

- [ ] **Step 3: Implement `derived.rs`**

```rust
//! Derived sources (spec §4.9): a filtered view frozen into a text of its own. The shown lines are copied into a cache
//! file (`streams/derived-<pid>-<n>.log`) that the tab maps like any log, and `numbers` keeps each line's original
//! display number. (Spec §4.9 reads the lines through the parent's mapping instead; copying lets scans, search, export,
//! nesting and session rebuilds work unchanged, at the cost of disk space for the shown lines.)

use crate::cache::{Cache, CacheLease};
use crate::cancel::CancelToken;
use crate::engine::{DEFAULT_CHUNK_LINES, run_filter};
use crate::error::CoreError;
use crate::filter::{CompiledFilter, FilterSpec};
use crate::session::{DerivedSpec, SessionSource};
use crate::source::{InflatePoll, SourceRef, open_source};
use crate::text::LogText;
use crate::view::FilteredView;
use std::collections::BTreeSet;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Lines between cancellation checks and progress reports while copying.
const COPY_BATCH: usize = 65_536;

pub struct Derived {
    /// The cache file holding the copied lines (delete it when the tab closes).
    pub path: PathBuf,
    pub text: LogText,
    /// Display number of each line in the original source.
    pub numbers: Vec<u64>,
}

pub struct Rebuilt {
    pub derived: Derived,
    /// Cache entries of the root source (an archive member, a decompressed `.gz`) that the rebuild read.
    pub leases: Vec<CacheLease>,
}

/// The lines the filtered log shows for `spec` and `pins` over `text` — matches, pins and context, as the app's view
/// composes them (spec §4.7).
pub fn view_lines(text: &LogText, spec: &FilterSpec, pins: &BTreeSet<u64>, cancel: &CancelToken) -> Result<Vec<u64>, CoreError> {
    let filter = CompiledFilter::compile(spec)?;
    let result = run_filter(text, &filter, DEFAULT_CHUNK_LINES, cancel)?;
    let view = FilteredView::build(Arc::new(result.matches), pins, spec.context, text.line_count(), |n| filter.is_excluded(text.line(n)));
    Ok(view.shown_lines().as_ref().clone())
}

/// Copies `lines` (0-based, ascending) of `text` into a new derived cache file, then maps and indexes it.
/// `number_of(line)` is a parent line's display number; progress reports lines copied of the total.
pub fn materialize(
    text: &LogText,
    lines: &[u64],
    number_of: impl Fn(u64) -> u64,
    cache: &Cache,
    cancel: &CancelToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<Derived, CoreError> {
    let streams = cache.streams_dir();
    std::fs::create_dir_all(&streams).map_err(CoreError::io(&streams))?;
    let path = cache.derived_path();
    let total = lines.len() as u64;
    cache.write_entry(&path, |file| {
        let mut out = BufWriter::with_capacity(1 << 20, file);
        for (k, &line) in lines.iter().enumerate() {
            if k % COPY_BATCH == 0 {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                progress(k as u64, total);
            }
            out.write_all(text.line(line)).and_then(|_| out.write_all(b"\n")).map_err(CoreError::io(&path))?;
        }
        out.flush().map_err(CoreError::io(&path))
    })?;
    progress(total, total);
    let opened = LogText::open(&path, cache.dir(), cancel, |_| {});
    let text = match opened {
        Ok(text) => text,
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
    };
    Ok(Derived { path, text, numbers: lines.iter().map(|&l| number_of(l)).collect() })
}

/// Opens `src` completely: a `.gz` that `open_source` returns while still decompressing is waited for and reopened
/// whole (a rebuild needs every line).
pub fn open_full(src: &SourceRef, cache: &Arc<Cache>, cancel: &CancelToken) -> Result<(LogText, Vec<CacheLease>), CoreError> {
    let mut opened = open_source(src, cache, cancel, |_| {})?;
    let Some(mut inflating) = opened.inflating.take() else { return Ok((opened.text, opened.leases)) };
    loop {
        if cancel.is_cancelled() {
            inflating.cancel();
            return Err(CoreError::Cancelled);
        }
        match inflating.poll() {
            InflatePoll::Running => std::thread::sleep(Duration::from_millis(20)),
            InflatePoll::Done(result) => {
                result?;
                break;
            }
        }
    }
    let dst = inflating.dst().to_path_buf();
    let text = LogText::open(&dst, cache.dir(), cancel, |_| {})?;
    Ok((text, opened.leases))
}

/// Rebuilds a derived tab from its definition (spec §6.4): opens the root source, applies each level's explicit lines
/// or filter, and writes the last level; intermediate levels' files are removed once used.
pub fn rebuild(spec: &DerivedSpec, cache: &Arc<Cache>, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<Rebuilt, CoreError> {
    let (parent, numbers, leases, temp) = match &spec.parent {
        SessionSource::File { path } => {
            let (text, leases) = open_full(&SourceRef::File(path.clone()), cache, cancel)?;
            (text, None, leases, None)
        }
        SessionSource::Member { source } => {
            let (text, leases) = open_full(source, cache, cancel)?;
            (text, None, leases, None)
        }
        SessionSource::Derived(inner) => {
            let r = rebuild(inner, cache, cancel, |_, _| {})?;
            (r.derived.text, Some(r.derived.numbers), r.leases, Some(r.derived.path))
        }
    };
    let count = parent.line_count();
    let lines = match &spec.lines {
        Some(lines) => {
            let mut lines: Vec<u64> = lines.iter().copied().filter(|&l| l < count).collect();
            lines.sort_unstable();
            lines.dedup();
            lines
        }
        None => view_lines(&parent, &spec.filter, &spec.pins.iter().copied().collect(), cancel)?,
    };
    let number_of = |l: u64| numbers.as_ref().and_then(|n| n.get(l as usize).copied()).unwrap_or(l + 1);
    let derived = materialize(&parent, &lines, number_of, cache, cancel, &mut progress);
    if let Some(path) = temp {
        let _ = std::fs::remove_file(path);
    }
    Ok(Rebuilt { derived: derived?, leases })
}
```

Check `Inflating::cancel(&self)` and `dst(&self)` exist (M3: `pub fn cancel(&self)`, `pub fn dst(&self) -> &Path`). `progress` is `impl FnMut`; pass `&mut progress`, because `&mut F` implements `FnMut`.

- [ ] **Step 4: Implement `export.rs`**

```rust
//! Exporting filtered lines to a file (spec §6.5): the shown lines, optionally without context lines, with gap markers
//! and original line numbers. Written to a writer-unique partial file and renamed, so a failure leaves nothing behind.

use crate::cache::partial_path;
use crate::cancel::CancelToken;
use crate::error::CoreError;
use crate::stats::noun;
use crate::text::LogText;
use crate::view::FilteredView;
use std::io::{BufWriter, Write};
use std::path::Path;

const BATCH: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Include context lines (off: only matched and pinned lines).
    pub context: bool,
    /// A `··· N lines omitted` line where lines were skipped.
    pub gaps: bool,
    /// Prefix each line with its original display number.
    pub numbers: bool,
}

pub fn gap_marker(omitted: u64) -> String {
    format!("··· {omitted} {} omitted", noun(omitted, "line", "lines"))
}

/// Writes the view's lines of `text` to `dst`; returns the number of log lines written (gap markers not counted).
pub fn export_lines(
    text: &LogText,
    view: &FilteredView,
    number_of: impl Fn(u64) -> u64,
    options: ExportOptions,
    dst: &Path,
    cancel: &CancelToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, CoreError> {
    let shown = view.shown_lines();
    let keep = |line: u64| options.context || view.is_anchor(line);
    let width = if options.numbers { shown.iter().rev().find(|&&l| keep(l)).map_or(1, |&l| number_of(l).to_string().len()) } else { 0 };
    let partial = partial_path(dst);
    let result = (|| -> Result<u64, CoreError> {
        let file = std::fs::File::create(&partial).map_err(CoreError::io(dst))?;
        let mut out = BufWriter::with_capacity(1 << 20, file);
        let total = shown.len() as u64;
        let mut written = 0u64;
        let mut prev: Option<u64> = None;
        for (k, &line) in shown.iter().enumerate() {
            if k % BATCH == 0 {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                progress(k as u64, total);
            }
            if !keep(line) {
                continue;
            }
            let io = CoreError::io(dst);
            if options.gaps
                && let Some(p) = prev
                && line > p + 1
            {
                writeln!(out, "{}", gap_marker(line - p - 1)).map_err(io)?;
            }
            if options.numbers {
                write!(out, "{:>width$}  ", number_of(line)).map_err(CoreError::io(dst))?;
            }
            out.write_all(text.line(line)).and_then(|_| out.write_all(b"\n")).map_err(CoreError::io(dst))?;
            written += 1;
            prev = Some(line);
        }
        out.flush().map_err(CoreError::io(dst))?;
        progress(total, total);
        Ok(written)
    })();
    match result {
        Ok(n) => std::fs::rename(&partial, dst).map(|_| n).map_err(|e| {
            let _ = std::fs::remove_file(&partial);
            CoreError::Io { path: dst.to_path_buf(), source: e }
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}
```

The first gap check runs before the first kept line. When context is off, gaps appear between anchors (line 2 → 6 omits 3), as the test expects. Add `pub mod derived;` and `pub mod export;` to `lib.rs`.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core && cargo clippy -p rogg-core --all-targets -- -D warnings`
Expected: all pass, clippy clean.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): derived sources, filtered-line export and a settable cache limit

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Log view styling: bold and explicit marks, log font, chip colours, statistics click

**Files:**
- Modify: `crates/rogg-ui/src/logview/model.rs`, `crates/rogg-ui/src/logview/text.rs`, `crates/rogg-ui/src/logview/element.rs`
- Modify: `crates/rogg-ui/src/theme/mod.rs`, `crates/rogg-ui/src/theme/metrics.rs`
- Modify: `crates/rogg-ui/src/filter.rs`, `crates/rogg-ui/src/chrome.rs`
- Test: `crates/rogg-ui/tests/logview.rs`, `crates/rogg-ui/tests/chrome.rs`, `crates/rogg-ui/tests/filter.rs`, unit tests in `text.rs`, `metrics.rs`, `theme/mod.rs`

**Interfaces:**
- Consumes: M0–M3 `SpanStyle`, `Segment`, `segments`, `RoggTheme`, `Density::metrics`, `FilterBand`, `FilterChip`, `StatusBar`.
- Produces:
  - `SpanStyle::Mark { fg: Color, bg: Color }`: a filter hit with explicit colours. It behaves like `Hit`: the first hit wins, and it is drawn as a background mark.
  - `SpanStyle::Bold`, and `Segment.bold: bool`.
  - `Fonts.log: SharedString`: the log text font (default `MONO_FONT`). The line-number gutter keeps `Fonts.mono`.
  - `RoggTheme::with_log_font(self, family: SharedString, compact_size: f32) -> Self`.
  - `Density::base_font_size(self) -> f32` (12 or 13) and `Density::metrics_for(self, compact_size: f32) -> Metrics`. `metrics()` equals `metrics_for(12.)`.
  - `FilterBand::chip_colors(self, colors: Option<Vec<ChipColor>>) -> Self`: per-slot chip colours, used modulo their count.
  - `StatusBar::on_stats(self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self`: the statistics area becomes clickable, with id `footer-stats`.

- [ ] **Step 1: Write the failing tests**

In the tests of `crates/rogg-ui/src/logview/text.rs`, add `bold: false` to every existing `Segment { … }` literal. Then append:

```rust
    #[test]
    fn marks_act_like_hits_and_bold_is_tracked() {
        let fg = Color::rgb(0x112233);
        let bg = Color::rgb(0x445566);
        let spans = vec![
            Span { range: 0..4, style: SpanStyle::Mark { fg, bg } },
            Span { range: 2..6, style: SpanStyle::Hit(1) },
            Span { range: 3..8, style: SpanStyle::Bold },
        ];
        let segs = segments(8, &spans, p().log_text, &p());
        assert_eq!(
            segs,
            vec![
                Segment { range: 0..3, fg, hit_bg: Some(bg), outline: false, bold: false },
                Segment { range: 3..4, fg, hit_bg: Some(bg), outline: false, bold: true },
                Segment { range: 4..6, fg: p().hits[1].fg, hit_bg: Some(p().hits[1].bg), outline: false, bold: true },
                Segment { range: 6..8, fg: p().log_text, hit_bg: None, outline: false, bold: true },
            ]
        );
    }

    #[test]
    fn later_foreground_spans_override_a_line_colour() {
        let line = Color::rgb(0xff0000);
        let faint = p().faint;
        let spans = vec![Span { range: 0..10, style: SpanStyle::Fg(line) }, Span { range: 0..3, style: SpanStyle::Faint }];
        let segs = segments(10, &spans, p().log_text, &p());
        assert_eq!(segs[0].range, 0..3);
        assert_eq!(segs[0].fg, faint);
        assert_eq!((segs[1].range.clone(), segs[1].fg), (3..10, line));
    }
```

In `crates/rogg-ui/src/theme/metrics.rs`, add a tests module, or append to an existing one:

```rust
#[cfg(test)]
mod scale_tests {
    use super::*;

    #[test]
    fn metrics_scale_with_the_log_font() {
        assert_eq!(Density::Compact.metrics_for(12.), Density::Compact.metrics());
        assert_eq!(Density::Comfortable.metrics_for(12.), Density::Comfortable.metrics());
        let m = Density::Compact.metrics_for(14.);
        assert_eq!((m.log_font_size, m.row_height, m.pin_top), (px(14.), px(27.), px(7.)));
        let c = Density::Comfortable.metrics_for(14.);
        assert_eq!((c.log_font_size, c.row_height), (px(15.), px(33.)));
        assert_eq!(c.pane_header_height, px(39.), "headers keep their height");
        let small = Density::Compact.metrics_for(9.);
        assert_eq!((small.row_height, small.pin_top), (px(17.), px(2.)));
    }
}
```

The expected values follow from `row = round(font × base_row / base_font)` and `pin_top = base_pin + round((row − base_row) / 2)`:

| Case | Font | Row height | `pin_top` |
|---|---|---|---|
| Compact, 14 px | 14 | round(26.83) = 27 | 5 + 2 = 7 |
| Comfortable, 14 + 1 px | 15 | round(33.46) = 33 | — |
| Compact, 9 px | 9 | round(17.25) = 17 | 5 − 3 = 2 |

In `crates/rogg-ui/src/theme/mod.rs` tests:

```rust
    #[test]
    fn log_font_overrides_family_and_metrics_only() {
        let t = RoggTheme::new(Appearance::Dark, Density::Compact).with_log_font("Menlo".into(), 14.);
        assert_eq!(t.fonts.log.as_ref(), "Menlo");
        assert_eq!(t.fonts.mono.as_ref(), MONO_FONT, "chips, dialogs and the gutter keep SF Mono");
        assert_eq!(t.metrics, Density::Compact.metrics_for(14.));
        assert_eq!(RoggTheme::new(Appearance::Dark, Density::Compact).fonts.log.as_ref(), MONO_FONT);
    }
```

In `crates/rogg-ui/tests/chrome.rs`:

```rust
#[gpui_kit::test]
fn the_statistics_area_is_clickable_only_with_a_handler(cx: &mut TestAppContext) {
    common::setup(cx);
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = clicks.clone();
    let w = common::open(cx, 900., 60., move |_, _| {
        let c = c.clone();
        StatusBar::new("UTF-8 · LF".into())
            .stats(vec![StatPart::Strong("6".into()), StatPart::Text(" matches".into())])
            .on_stats(move |_, _| c.set(c.get() + 1))
            .into_any_element()
    });
    common::click(cx, w, "footer-stats");
    assert_eq!(clicks.get(), 1);
}
```

Add the needed imports to that file: `StatPart`, `StatusBar` and `gpui_kit::prelude::*`, if not already there.

In `crates/rogg-ui/tests/filter.rs`, add a pure test of the chip colour choice:

```rust
#[test]
fn chip_colours_can_be_overridden_per_slot() {
    use rogg_ui::filter::chip_color;
    use rogg_ui::theme::{ChipColor, Color, Palette};
    let p = Palette::dark();
    assert_eq!(chip_color(5, None, &p), p.chips[1]);
    let custom = vec![ChipColor { border: Color::rgb(0x111111), bg: Color::rgb(0x222222), term: Color::rgb(0x333333) }];
    assert_eq!(chip_color(5, Some(&custom), &p), custom[0]);
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui`
Expected: compile errors (`Mark`, `Bold`, `bold`, `metrics_for`, `with_log_font`, `on_stats`, `chip_color`).

- [ ] **Step 2: Model and segments**

`crates/rogg-ui/src/logview/model.rs`, in `SpanStyle`, add after `Hit(usize)`:

```rust
    /// A filter match with explicit colours (a highlighting profile's palette); drawn like `Hit`.
    Mark { fg: Color, bg: Color },
    /// Bold text (a profile rule's bold effect).
    Bold,
```

`crates/rogg-ui/src/logview/text.rs`:
- add `pub bold: bool` to `Segment`;
- change `resolve` to return `None` for `Hit(_) | Mark { .. } | Bold | SearchHit`;
- replace `segments` with this version (same sweep, now with marks and bold):

```rust
fn hit_style(spans: &[Span], hits: &std::collections::BTreeSet<usize>, p: &Palette) -> Option<(Color, Color)> {
    match spans[*hits.iter().next()?].style {
        SpanStyle::Hit(slot) => {
            let h = p.hits[slot % p.hits.len()];
            Some((h.fg, h.bg))
        }
        SpanStyle::Mark { fg, bg } => Some((fg, bg)),
        _ => None,
    }
}

fn push_segment(out: &mut Vec<Segment>, seg: Segment) {
    if let Some(last) = out.last_mut()
        && last.range.end == seg.range.start
        && last.fg == seg.fg
        && last.hit_bg == seg.hit_bg
        && last.outline == seg.outline
        && last.bold == seg.bold
    {
        last.range.end = seg.range.end;
        return;
    }
    out.push(seg);
}

/// Splits `0..len` into maximal runs of identical style. Later foreground spans override earlier ones, a filter hit or
/// mark overrides the foreground (the first one wins), search hits add an outline and bold spans make the run bold.
///
/// O(n log n) event sweep: each span's start and end are visited once, in position order.
pub fn segments(len: usize, spans: &[Span], base: Color, p: &Palette) -> Vec<Segment> {
    use std::collections::BTreeSet;
    if len == 0 {
        return Vec::new();
    }
    let mut events = Vec::with_capacity(spans.len() * 2);
    for (idx, span) in spans.iter().enumerate() {
        let (start, end) = (span.range.start.min(len), span.range.end.min(len));
        if start < end {
            events.push((start, true, idx));
            events.push((end, false, idx));
        }
    }
    // Starts before ends at the same position; every event at a position is applied before the next run is emitted.
    events.sort_by_key(|e| (e.0, !e.1));
    let mut fg: BTreeSet<usize> = BTreeSet::new();
    let mut hits: BTreeSet<usize> = BTreeSet::new();
    let (mut outlines, mut bolds) = (0usize, 0usize);
    let mut out = Vec::new();
    let mut emit = |range: std::ops::Range<usize>, fg: &BTreeSet<usize>, hits: &BTreeSet<usize>, outlines: usize, bolds: usize, out: &mut Vec<Segment>| {
        let base_fg = fg.iter().next_back().and_then(|&i| resolve(spans[i].style, p)).unwrap_or(base);
        let (color, hit_bg) = match hit_style(spans, hits, p) {
            Some((hit_fg, hit_bg)) => (hit_fg, Some(hit_bg)),
            None => (base_fg, None),
        };
        push_segment(out, Segment { range, fg: color, hit_bg, outline: outlines > 0, bold: bolds > 0 });
    };
    let mut last = 0;
    let mut i = 0;
    while i < events.len() {
        let pos = events[i].0;
        if last < pos {
            emit(last..pos, &fg, &hits, outlines, bolds, &mut out);
        }
        while i < events.len() && events[i].0 == pos {
            let (_, start, idx) = events[i];
            match spans[idx].style {
                SpanStyle::Hit(_) | SpanStyle::Mark { .. } => {
                    if start {
                        hits.insert(idx);
                    } else {
                        hits.remove(&idx);
                    }
                }
                SpanStyle::SearchHit => outlines = if start { outlines + 1 } else { outlines.saturating_sub(1) },
                SpanStyle::Bold => bolds = if start { bolds + 1 } else { bolds.saturating_sub(1) },
                SpanStyle::Faint | SpanStyle::Muted | SpanStyle::Error | SpanStyle::Amber | SpanStyle::Fg(_) => {
                    if start {
                        fg.insert(idx);
                    } else {
                        fg.remove(&idx);
                    }
                }
            }
            i += 1;
        }
        last = pos;
    }
    if last < len {
        emit(last..len, &fg, &hits, outlines, bolds, &mut out);
    }
    out
}
```

`emit` takes `out` as a parameter, so the closure borrows nothing mutably across the loop. If clippy flags the closure's argument count, turn it into a nested `fn` that also takes `spans`, `base` and `p`.

Search the crate for other `SpanStyle` matches and add the two new variants wherever a match must be exhaustive: `grep -rn "SpanStyle::" crates/rogg-ui/src crates/rogg-gallery/src`.

- [ ] **Step 3: Fonts and metrics**

`crates/rogg-ui/src/theme/metrics.rs`, in `impl Density`, replace `metrics` with:

```rust
    /// The density's log font size in the reference (compact 12 px, comfortable 13 px).
    pub fn base_font_size(self) -> f32 {
        match self {
            Density::Compact => 12.,
            Density::Comfortable => 13.,
        }
    }

    pub fn metrics(self) -> Metrics {
        self.metrics_for(12.)
    }

    /// Metrics for the log font size setting `compact_size` (spec §6.2): comfortable adds 1 px, as the reference does;
    /// the row height keeps the density's ratio to its font (23/12, 29/13) in whole pixels, and the pin glyph stays
    /// centred. Pane headers keep their height.
    pub fn metrics_for(self, compact_size: f32) -> Metrics {
        let (base_row, header, base_pin) = match self {
            Density::Compact => (23., 35., 5.),
            Density::Comfortable => (29., 39., 8.),
        };
        let font = compact_size + (self.base_font_size() - 12.);
        let row = (font * base_row / self.base_font_size()).round();
        Metrics { row_height: px(row), log_font_size: px(font), pane_header_height: px(header), pin_top: px(base_pin + ((row - base_row) / 2.).round()) }
    }
```

`crates/rogg-ui/src/theme/mod.rs`:
- add `pub log: SharedString` to `Fonts` (doc: "Log text in both panes; the line-number gutter keeps `mono`");
- set `log: MONO_FONT.into()` in `RoggTheme::new`;
- add:

```rust
    /// The log font family and compact size from the settings (spec §6.2).
    pub fn with_log_font(mut self, family: SharedString, compact_size: f32) -> Self {
        self.fonts.log = family;
        self.metrics = self.density.metrics_for(compact_size);
        self
    }
```

- [ ] **Step 4: The element**

`crates/rogg-ui/src/logview/element.rs`:

1. Add a small struct next to `shape_row`:

```rust
/// Fonts a row is shaped with: the log text (regular and bold), the line-number gutter and the gap-row label.
struct RowFonts {
    log: Font,
    log_bold: Font,
    gutter: Font,
    ui: Font,
}
```

2. Change `shape_row(rows, row, theme, mono: &Font, ui: &Font, wrap_width, window)` to `shape_row(rows, row, theme, fonts: &RowFonts, wrap_width, window)`:
   - text runs: `segs.iter().map(|s| run(s.range.len(), if s.bold { &fonts.log_bold } else { &fonts.log }, s.fg, None))`;
   - number runs: `&fonts.gutter`;
   - gap label: `&fonts.ui`.
3. In `prepaint`, replace `let mono = font(theme.fonts.mono.clone()); let ui = font(theme.fonts.ui.clone());` with:

```rust
        let log = font(theme.fonts.log.clone());
        let fonts = RowFonts { log_bold: log.clone().bold(), log, gutter: font(theme.fonts.mono.clone()), ui: font(theme.fonts.ui.clone()) };
```

   Pass `&fonts` at both `shape_row` call sites. Use `&fonts.log` where `resolve_font(&mono)` measured the advance (the horizontal extent follows the log font).
4. In the mouse handler that builds `let mono = font(pre.theme.fonts.mono.clone());` for `index_at`, use `pre.theme.fonts.log` instead: hit-testing must use the font the text was shaped with.

- [ ] **Step 5: Chips and the footer**

`crates/rogg-ui/src/filter.rs`:

```rust
/// The colours of chip `index`: the override list (a profile's palette) when given, else the theme's four slots.
pub fn chip_color(index: usize, overrides: Option<&[ChipColor]>, p: &Palette) -> ChipColor {
    match overrides {
        Some(list) if !list.is_empty() => list[index % list.len()],
        _ => p.chips[index % p.chips.len()],
    }
}
```

- Add `color: Option<ChipColor>` to `FilterChip`, with a builder `pub fn color(mut self, c: Option<ChipColor>) -> Self`.
- In `render`, use `let slot = self.color.unwrap_or(p.chips[self.index % p.chips.len()]);`.
- Add `chip_colors: Option<Vec<ChipColor>>` to `FilterBand`, with:

```rust
    /// Per-slot chip colours (the tab's highlighting palette); None uses the theme's.
    pub fn chip_colors(mut self, colors: Option<Vec<ChipColor>>) -> Self {
        self.chip_colors = colors;
        self
    }
```

- Where the band builds each `FilterChip`, chain `.color(self.chip_colors.as_deref().map(|c| chip_color(i, Some(c), &p)))`, where `p` is the palette already bound in that render.
- Import `ChipColor` and `Palette` from `crate::theme`.

`crates/rogg-ui/src/chrome.rs`, `StatusBar`:
- add `on_stats: Option<Callback>`, initialised to `None`, and the builder:

```rust
    /// Makes the statistics clickable (opens the statistics popover, spec §4.11).
    pub fn on_stats(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stats = Some(Rc::new(f));
        self
    }
```

- in `render`, give the stats container an id and the click handler only when set. This keeps the pixels unchanged:

```rust
            .child(
                div()
                    .id("footer-stats")
                    .test_support()
                    .flex()
                    .items_center()
                    .when_some(self.on_stats, |d, f| d.cursor_pointer().on_click(move |_, window, cx| f(window, cx)))
                    .children(self.stats.into_iter().map(|part| match part {
                        StatPart::Strong(s) => div().font_weight(FontWeight::MEDIUM).text_color(p.text).child(s),
                        StatPart::Text(s) => div().child(s),
                    })),
            )
```

- [ ] **Step 6: Run the tests and the golden check**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; just visual-check`
Expected: 0.000% differing pixels on all 9 replica captures. The default fonts, metrics and chip colours are unchanged.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-ui crates/rogg-gallery
git commit -m "feat(ui): bold and palette marks in log rows, configurable log font, chip colour overrides, clickable statistics

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: UI font size (text scale)

**Files:**
- Modify: `crates/rogg-ui/src/theme/mod.rs`, `crates/rogg-ui/src/theme/kit.rs`
- Modify: every rogg-ui component that sets a text size outside the log view: `chrome.rs`, `titlebar.rs`, `sidebar.rs`, `filter.rs`, `dialogs.rs`, `empty.rs`, `toast.rs`, `controls/{buttons,find_field,segmented,select}.rs`, and `layout.rs` if it sets text sizes
- Test: unit tests in `theme/mod.rs`; `crates/rogg-ui/tests/theme_install.rs`

**Interfaces:**
- Consumes: Task 7 `Fonts`.
- Produces:
  - `TextScale(pub f32)` (Copy) with `TextScale::px(self, n: f32) -> Pixels`.
  - `RoggTheme::text_scale(&self) -> TextScale`, which is `ui_size / 13`.
  - `RoggTheme::with_ui_font_size(self, size: f32) -> Self`.
  - Every non-log text size in rogg-ui goes through `TextScale::px`. Line heights, bar heights and paddings stay fixed. At 13 px nothing changes.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-ui/src/theme/mod.rs` tests:

```rust
    #[test]
    fn text_scale_follows_the_ui_font_size() {
        let t = RoggTheme::new(Appearance::Dark, Density::Compact);
        assert_eq!(t.text_scale().px(11.), px(11.), "13 px is the reference size: nothing scales");
        let big = t.clone().with_ui_font_size(16.);
        assert_eq!(big.fonts.ui_size, px(16.));
        assert!((f32::from(big.text_scale().px(13.)) - 16.).abs() < 1e-4);
        assert!((f32::from(big.text_scale().px(10.)) - 10. * 16. / 13.).abs() < 1e-4);
    }
```

`crates/rogg-ui/tests/theme_install.rs`, a new test:

```rust
#[gpui_kit::test]
fn the_kit_font_size_follows_the_ui_font_size(cx: &mut TestAppContext) {
    common::setup(cx);
    cx.update(|cx| {
        rogg_ui::theme::RoggTheme::new(rogg_ui::theme::Appearance::Dark, rogg_ui::theme::Density::Compact).with_ui_font_size(15.).install(cx);
        assert_eq!(gpui_kit::component::Theme::global(cx).font_size, gpui_kit::px(15.));
    });
}
```

Before writing this test, check how `theme_install.rs` reads the kit theme, and use the same accessor.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui theme`
Expected: compile errors.

- [ ] **Step 2: Implement the scale**

`theme/mod.rs`:

```rust
/// UI text sizes relative to the reference's 13 px system font (spec §6.2 "UI font size"); `px(11.)` at 13 px stays 11.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextScale(pub f32);

impl TextScale {
    pub fn px(self, n: f32) -> Pixels {
        px(n * self.0)
    }
}

/// The reference UI font size.
pub const BASE_UI_FONT_SIZE: f32 = 13.;
```

In `impl RoggTheme`:

```rust
    pub fn text_scale(&self) -> TextScale {
        TextScale(f32::from(self.fonts.ui_size) / BASE_UI_FONT_SIZE)
    }

    pub fn with_ui_font_size(mut self, size: f32) -> Self {
        self.fonts.ui_size = px(size);
        self
    }
```

`theme/kit.rs`: set `t.font_size = theme.text_scale().px(13.);`. Clone the scale into the `update` closure, because the closure is `move`.

- [ ] **Step 3: Route component text sizes through the scale**

List the sites: `grep -rn "text_size(px(" crates/rogg-ui/src | grep -v "/logview/"`.

For each component render:
- bind `let s = cx.rogg().text_scale();` next to the existing `let p = …` / `let t = …`;
- replace `.text_size(px(N.))` with `.text_size(s.px(N.))`.

`TextScale` is `Copy`, so it can be used inside `.map(|…| …)` closures. Leave untouched:
- the log view's gutter (10 px) and gap label (9 px), which are log text;
- every `line_height`, `h`, `w`, padding and icon size.

`CompactSelect` measures its options with `shape_line(…, px(10.), …)`. Change that measurement to `s.px(10.)` as well, so the box fits the scaled label.

- [ ] **Step 4: Run the tests and the golden check**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings && just visual-check`
Expected: tests pass; `just visual-check` reports 0.000% on all 9 captures.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): scale UI text with the UI font size setting

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Form controls for the settings and profile dialogs

**Files:**
- Create: `crates/rogg-ui/src/form.rs`
- Modify: `crates/rogg-ui/src/lib.rs`
- Modify: `crates/rogg-gallery/src/pages.rs`, `crates/rogg-gallery/src/gallery.rs` (an `Inputs` field for a specimen text field)
- Test: `crates/rogg-ui/tests/form.rs`; `crates/rogg-gallery/tests/pages.rs`

**Interfaces:**
- Consumes: Task 8 `TextScale`; M0 `Color`, `Palette`, `IconName`, `Callback`, `DialogFrame`.
- Produces, in `rogg_ui::form`:
  - `FormRow::new(id, label)` with `.hint(text)` and `.control(impl IntoElement)`.
  - `SectionLabel::new(text)`.
  - `Toggle::new(id, on)` with `.on_click(f)`.
  - `Stepper::new(id, value)` with `.bounds(can_down, can_up)` and `.on_step(f: Fn(bool, &mut Window, &mut App))` (`true` is up). Child ids are `<id>-down` and `<id>-up`.
  - `Swatch::new(id, color)` with `.selected(bool)` and `.on_click(f)`.
  - `TextField::new(id, input: Entity<InputState>)` with `.mono(bool)`, `.width(f32)` and `.error(bool)`.
  - `EditableRuleRow::new(index, swatch, pattern, summary)` with `.enabled(bool)`, `.selected(bool)`, `.error(Option<SharedString>)`, `.can_move(up: bool, down: bool)`, `.on_select(f)`, `.on_toggle(f)` and `.on_move(f: Fn(bool /*up*/, …))`. Ids: `profile-rule-{i}`, `profile-rule-{i}-toggle`, `profile-rule-{i}-up`, `profile-rule-{i}-down`, `profile-rule-{i}-error`.
  - `PaletteSlot::new(index, fg, bg)` with `.selected(bool)` and `.on_click(f)`. Id: `palette-slot-{i}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-ui/tests/form.rs`:

```rust
mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div};
use rogg_ui::form::{EditableRuleRow, FormRow, PaletteSlot, Stepper, Toggle};
use rogg_ui::theme::Color;
use std::cell::RefCell;
use std::rc::Rc;

fn log() -> Rc<RefCell<Vec<String>>> {
    Rc::new(RefCell::new(Vec::new()))
}

#[gpui_kit::test]
fn toggle_and_stepper_report_clicks_and_respect_bounds(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b) = (e.clone(), e.clone());
        div()
            .child(FormRow::new("row-wrap", "Wrap lines").control(Toggle::new("wrap", false).on_click(move |_, _| a.borrow_mut().push("toggle".into()))))
            .child(FormRow::new("row-size", "Log font size").control(Stepper::new("size", "20 px").bounds(true, false).on_step(move |up, _, _| b.borrow_mut().push(format!("step {up}")))))
            .into_any_element()
    });
    common::click(cx, w, "wrap");
    common::click(cx, w, "size-down");
    common::click(cx, w, "size-up");
    assert_eq!(*events.borrow(), ["toggle", "step false"], "the up button at the bound does nothing");
    assert!(common::exists(cx, w, "row-size"));
}

#[gpui_kit::test]
fn rule_rows_select_toggle_move_and_show_errors(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b, c) = (e.clone(), e.clone(), e.clone());
        EditableRuleRow::new(1, Color::rgb(0xefaaa2), "ERROR", "Line text + row tint · both panels")
            .enabled(true)
            .selected(true)
            .error(Some("invalid regex".into()))
            .can_move(true, false)
            .on_select(move |_, _| a.borrow_mut().push("select".into()))
            .on_toggle(move |_, _| b.borrow_mut().push("toggle".into()))
            .on_move(move |up, _, _| c.borrow_mut().push(format!("move {up}")))
            .into_any_element()
    });
    common::click(cx, w, "profile-rule-1-toggle");
    common::click(cx, w, "profile-rule-1-up");
    common::click(cx, w, "profile-rule-1-down");
    common::click(cx, w, "profile-rule-1");
    assert_eq!(*events.borrow(), ["toggle", "move true", "select"], "controls do not also select the row; down is disabled");
    assert!(common::exists(cx, w, "profile-rule-1-error"));
}

#[gpui_kit::test]
fn palette_slots_are_clickable(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 400., 100., move |_, _| {
        let a = e.clone();
        PaletteSlot::new(2, Color::rgb(0xacd7ea), Color::rgb(0x315064)).selected(true).on_click(move |_, _| a.borrow_mut().push("slot".into())).into_any_element()
    });
    common::click(cx, w, "palette-slot-2");
    assert_eq!(*events.borrow(), ["slot"]);
}
```

Clicking a row's toggle or move button must not also fire the row's `on_select`. Each control stops propagation (`cx.stop_propagation()` inside its click handler).

In `crates/rogg-gallery/tests/pages.rs`, add a test following the file's existing pattern. It asserts that the Controls page renders `specimen-toggle-on`, `specimen-stepper` and `specimen-text-field`, and that the Dialogs page renders `profile-rule-0` and `palette-slot-0`.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --test form`
Expected: compile errors.

- [ ] **Step 2: Implement `form.rs`**

```rust
//! Form controls for the settings and profile-editor dialogs (spec §5.8, §6.1, §6.2), in the reference dialog's style:
//! `.profile-rule` rows, `button[aria-pressed]` states, `.swatch` squares.

use crate::controls::pressed;
use crate::theme::{ActiveRoggTheme, Color};
use crate::Callback;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Entity, FontWeight, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

/// A clickable child that never lets the click reach the row underneath.
fn stop<E: InteractiveElement + StatefulInteractiveElement>(el: E, f: Option<Callback>) -> E {
    el.on_click(move |_, window, cx| {
        cx.stop_propagation();
        if let Some(f) = &f {
            f(window, cx)
        }
    })
}

/// A dialog row: label (and an optional hint) on the left, a control on the right, a 1 px top border.
#[derive(IntoElement)]
pub struct FormRow {
    id: SharedString,
    label: SharedString,
    hint: Option<SharedString>,
    control: Option<AnyElement>,
}

impl FormRow {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), hint: None, control: None }
    }

    pub fn hint(mut self, text: impl Into<SharedString>) -> Self {
        self.hint = Some(text.into());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }
}

impl RenderOnce for FormRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(10.))
            .border_t_1()
            .border_color(p.border)
            .text_size(s.px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(self.label)
                    .when_some(self.hint, |d, h| d.child(div().text_size(s.px(10.)).text_color(p.faint).child(h))),
            )
            .child(div().ml_auto().flex_none().flex().items_center().gap(px(6.)).children(self.control))
    }
}

/// `.small-label`: an uppercase 10 px section title.
#[derive(IntoElement)]
pub struct SectionLabel {
    text: SharedString,
}

impl SectionLabel {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for SectionLabel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        div().mt(px(16.)).mb(px(4.)).text_size(s.px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(p.muted).child(self.text.to_uppercase())
    }
}

/// An on/off switch: a 28×16 pill, accent when on.
#[derive(IntoElement)]
pub struct Toggle {
    id: SharedString,
    on: bool,
    on_click: Option<Callback>,
}

impl Toggle {
    pub fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Self { id: id.into(), on, on_click: None }
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let knob = div().size(px(10.)).rounded(px(5.)).bg(if self.on { p.accent } else { p.muted });
        let el = div()
            .id(self.id)
            .test_support()
            .flex()
            .items_center()
            .w(px(28.))
            .h(px(16.))
            .px(px(2.))
            .rounded(px(8.))
            .border_1()
            .cursor_pointer()
            .map(|d| if self.on { d.justify_end().bg(p.accent_bg).border_color(p.accent_border) } else { d.bg(p.raised).border_color(p.border) })
            .child(knob);
        stop(el, self.on_click)
    }
}

fn step_button(id: SharedString, glyph: &'static str, enabled: bool, f: Option<Callback>, cx: &App) -> impl IntoElement {
    let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
    let el = div()
        .id(id)
        .test_support()
        .flex()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(px(5.))
        .border_1()
        .border_color(p.border)
        .text_size(s.px(12.))
        .text_color(if enabled { p.text } else { p.faint })
        .child(glyph)
        .when(enabled, |d| d.cursor_pointer().hover(move |st| st.bg(p.hover)));
    stop(el, if enabled { f } else { None })
}

/// `−  value  +` with step buttons; a button at its bound is faint and inert.
#[derive(IntoElement)]
pub struct Stepper {
    id: SharedString,
    value: SharedString,
    can_down: bool,
    can_up: bool,
    on_step: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

impl Stepper {
    pub fn new(id: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self { id: id.into(), value: value.into(), can_down: true, can_up: true, on_step: None }
    }

    pub fn bounds(mut self, can_down: bool, can_up: bool) -> Self {
        self.can_down = can_down;
        self.can_up = can_up;
        self
    }

    pub fn on_step(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (p, s) = (cx.rogg().palette, cx.rogg().text_scale());
        let step = |up: bool| -> Option<Callback> {
            self.on_step.clone().map(|f| Rc::new(move |w: &mut Window, cx: &mut App| f(up, w, cx)) as Callback)
        };
        div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(px(6.))
            .child(step_button(format!("{}-down", self.id).into(), "−", self.can_down, step(false), cx))
            .child(div().min_w(px(64.)).flex().justify_center().text_size(s.px(12.)).text_color(p.text).child(self.value))
            .child(step_button(format!("{}-up", self.id).into(), "+", self.can_up, step(true), cx))
    }
}

/// A 21 px colour square (reference `.swatch`); selected shows the accent ring.
#[derive(IntoElement)]
pub struct Swatch {
    id: SharedString,
    color: Color,
    selected: bool,
    on_click: Option<Callback>,
}

impl Swatch {
    pub fn new(id: impl Into<SharedString>, color: Color) -> Self {
        Self { id: id.into(), color, selected: false, on_click: None }
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Swatch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let el = div()
            .id(self.id)
            .test_support()
            .flex_none()
            .size(px(21.))
            .rounded(px(4.))
            .border_1()
            .border_color(if self.selected { p.accent } else { p.swatch_border })
            .bg(self.color)
            .when(self.on_click.is_some(), |d| d.cursor_pointer());
        stop(el, self.on_click)
    }
}

/// A one-line input in the dialog style; `error` paints its border in the error colour.
#[derive(IntoElement)]
pub struct TextField {
    id: SharedString,
    input: Entity<InputState>,
    mono: bool,
    width: Option<f32>,
    error: bool,
}

impl TextField {
    pub fn new(id: impl Into<SharedString>, input: Entity<InputState>) -> Self {
        Self { id: id.into(), input, mono: false, width: None, error: false }
    }

    pub fn mono(mut self, on: bool) -> Self {
        self.mono = on;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn error(mut self, on: bool) -> Self {
        self.error = on;
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let family = if self.mono { t.fonts.mono.clone() } else { t.fonts.ui.clone() };
        div()
            .id(self.id)
            .test_support()
            .px(px(8.))
            .py(px(4.))
            .bg(p.bg)
            .border_1()
            .border_color(if self.error { p.error } else { p.border })
            .rounded(px(4.))
            .when_some(self.width, |d, w| d.w(px(w)))
            .child(Input::new(&self.input).appearance(false).p_0().font_family(family).text_size(s.px(12.)).text_color(p.text))
    }
}

/// A rule in the profile editor: swatch, pattern (mono), summary, then the enable switch and move buttons.
#[derive(IntoElement)]
pub struct EditableRuleRow {
    index: usize,
    swatch: Color,
    pattern: SharedString,
    summary: SharedString,
    enabled: bool,
    selected: bool,
    error: Option<SharedString>,
    can_up: bool,
    can_down: bool,
    on_select: Option<Callback>,
    on_toggle: Option<Callback>,
    on_move: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

impl EditableRuleRow {
    pub fn new(index: usize, swatch: Color, pattern: impl Into<SharedString>, summary: impl Into<SharedString>) -> Self {
        Self {
            index,
            swatch,
            pattern: pattern.into(),
            summary: summary.into(),
            enabled: true,
            selected: false,
            error: None,
            can_up: true,
            can_down: true,
            on_select: None,
            on_toggle: None,
            on_move: None,
        }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn error(mut self, error: Option<SharedString>) -> Self {
        self.error = error;
        self
    }

    pub fn can_move(mut self, up: bool, down: bool) -> Self {
        self.can_up = up;
        self.can_down = down;
        self
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn on_toggle(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    pub fn on_move(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_move = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for EditableRuleRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let i = self.index;
        let mover = |up: bool| -> Option<Callback> { self.on_move.clone().map(|f| Rc::new(move |w: &mut Window, cx: &mut App| f(up, w, cx)) as Callback) };
        let on_select = self.on_select.clone();
        div()
            .id(SharedString::from(format!("profile-rule-{i}")))
            .test_support()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(p.border)
            .when(self.selected, |d| d.bg(p.selection))
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                if let Some(f) = &on_select {
                    f(window, cx)
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(10.))
                    .px(px(4.))
                    .text_size(s.px(12.))
                    .when(!self.enabled, |d| d.opacity(0.5))
                    .child(div().flex_none().size(px(21.)).rounded(px(4.)).border_1().border_color(p.swatch_border).bg(self.swatch))
                    .child(div().min_w_0().truncate().font_family(t.fonts.mono.clone()).child(self.pattern))
                    .child(div().ml_auto().flex_none().text_size(s.px(11.)).text_color(p.muted).child(self.summary))
                    .child(Toggle::new(SharedString::from(format!("profile-rule-{i}-toggle")), self.enabled).on_click({
                        let f = self.on_toggle.clone();
                        move |w, cx| {
                            if let Some(f) = &f {
                                f(w, cx)
                            }
                        }
                    }))
                    .child(step_button(format!("profile-rule-{i}-up").into(), "↑", self.can_up, mover(true), cx))
                    .child(step_button(format!("profile-rule-{i}-down").into(), "↓", self.can_down, mover(false), cx)),
            )
            .when_some(self.error, |d, e| {
                d.child(div().id(SharedString::from(format!("profile-rule-{i}-error"))).test_support().px(px(4.)).pb(px(8.)).text_size(s.px(10.)).text_color(p.error).child(e))
            })
    }
}

/// One filter-match colour slot: a `term` sample in its colours; selected shows the accent border.
#[derive(IntoElement)]
pub struct PaletteSlot {
    index: usize,
    fg: Color,
    bg: Color,
    selected: bool,
    on_click: Option<Callback>,
}

impl PaletteSlot {
    pub fn new(index: usize, fg: Color, bg: Color) -> Self {
        Self { index, fg, bg, selected: false, on_click: None }
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PaletteSlot {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let (p, s) = (t.palette, t.text_scale());
        let el = div()
            .id(SharedString::from(format!("palette-slot-{}", self.index)))
            .test_support()
            .flex_none()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(5.))
            .border_1()
            .map(|d| if self.selected { pressed(d, &p).bg(self.bg) } else { d.border_color(p.border).bg(self.bg) })
            .font_family(t.fonts.mono.clone())
            .text_size(s.px(12.))
            .text_color(self.fg)
            .cursor_pointer()
            .child(format!("term {}", self.index + 1));
        stop(el, self.on_click)
    }
}
```

`pressed` sets the text colour to the accent. Call `.text_color(self.fg)` after `map` (as written above), so the slot shows its own colour. Make `controls::pressed` `pub(crate)` if it is not already; it is defined in `controls/mod.rs`.

Add `pub mod form;` to `crates/rogg-ui/src/lib.rs`.

- [ ] **Step 3: Gallery specimens**

In `crates/rogg-gallery/src/gallery.rs`, add `pub form_text: Entity<InputState>` to `Inputs`, created like the other inputs with the default value `#e0c283`.

In `crates/rogg-gallery/src/pages.rs`:
- `controls(...)`: add a row with:
  - `Toggle::new("specimen-toggle-on", true)` and `Toggle::new("specimen-toggle-off", false)`;
  - `Stepper::new("specimen-stepper", "12 px")`;
  - a `Swatch` row for every `COLOR_TOKENS`-like palette colour (`p.error`, `p.amber`, `p.muted`, `p.faint`, `p.accent`, `p.hits[0..4].fg`), with the first selected;
  - `TextField::new("specimen-text-field", view.inputs.form_text.clone()).mono(true).width(110.)`.
- `dialogs_page(...)`: add a `DialogFrame::new("Highlighting · Services")` specimen with:
  - three `EditableRuleRow`s (index 0 selected, index 2 with an `.error(Some("invalid regex: unclosed group".into()))`);
  - a `SectionLabel::new("Filter-match palette")`;
  - four `PaletteSlot`s coloured from `p.hits`.

  Add a `DialogFrame::new("Settings")` specimen with:
  - `SectionLabel::new("Appearance")`;
  - `FormRow`s with a `Segmented` (theme), a `Stepper` (UI font size), a `Toggle` (wrap) and a `Stepper` (cache limit, hint `Extracted archive members and conversions`).

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings && just visual-check`
Expected: PASS, and 0.000% on the replica captures. The new specimens are on component pages only.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui crates/rogg-gallery
git commit -m "feat(ui): form controls for the settings and profile dialogs, with gallery specimens

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 10: Settings store, live appearance and behaviour defaults

**Files:**
- Create: `crates/rogg-app/src/settings.rs`, `crates/rogg-app/tests/settings.rs`
- Modify:
  - `crates/rogg-app/src/paths.rs`, `crates/rogg-app/src/lib.rs`
  - `crates/rogg-app/src/workspace/{mod,keys,chrome}.rs`
  - `crates/rogg-app/src/sources.rs`
  - `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`
  - `crates/rogg-app/src/cache.rs`
  - `crates/rogg-gallery/src/gallery.rs`

**Interfaces:**
- Consumes:
  - Task 2 `Settings` and friends.
  - Task 6 `Cache::set_limit`.
  - Task 7 `RoggTheme::with_log_font`, Task 8 `RoggTheme::with_ui_font_size`.
- Produces:
  - `AppPaths { history, recent, cache, settings: PathBuf, profiles: PathBuf, session: PathBuf }`.
  - `rogg_app::settings`:
    - `SettingsStore { pub recovered, pub last_error }` (Global) with `SettingsStore::load(PathBuf)`, `SettingsStore::get(&App) -> Settings`, `SettingsStore::update(&mut App, impl FnOnce(&mut Settings)) -> (Settings, Settings)` (before, after; clamped and saved);
    - `appearance_for(ThemeSetting, WindowAppearance) -> Appearance` and `density_for(DensitySetting) -> Density`;
    - `log_font_family(&str) -> SharedString` and `available_log_fonts(&App) -> Vec<String>`;
    - `theme_for(&Settings, WindowAppearance) -> RoggTheme`;
    - `apply_theme(theme: ThemeSetting, WindowAppearance, &mut App) -> bool` (`true` when it installed a new theme);
    - `apply_cache_limit(&mut App)`.
  - `Workspace`:
    - `change_settings(&mut self, f: impl FnOnce(&mut Settings), window, cx)`;
    - `set_theme(ThemeSetting, window, cx)`, `set_density(DensitySetting, window, cx)`;
    - `pin_theme(&mut self, ThemeSetting, window, cx)`: in memory only, for the gallery;
    - `apply_appearance(&mut self, window, cx)`.

    `workspace::keys::ThemeChoice` and `keys::appearance_for` are removed.
  - `TabInit { doc, selection, labels, wrap: [bool; 2], follow: bool, split: Option<f32>, top: [Option<u64>; 2] }`:
    - `Default` has `follow: true`;
    - `TabInit::from_settings(&Settings) -> TabInit`.
  - `TreeJob::scan(node, dir, depth: usize)`.
  - `tab_view::context_options(context: u32) -> (Vec<SharedString>, usize)`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/settings.rs`:

```rust
mod common;
use gpui_kit::{AppContext, TestAppContext, px};
use rogg_app::args::Args;
use rogg_app::cache::CacheStore;
use rogg_app::settings::SettingsStore;
use rogg_core::settings::{DensitySetting, ThemeSetting};
use rogg_ui::theme::{ActiveRoggTheme, Appearance};

#[gpui_kit::test]
fn appearance_settings_apply_live_and_persist(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.change_settings(
                |s| {
                    s.appearance.theme = ThemeSetting::Dark;
                    s.appearance.log_font_size = 14;
                    s.appearance.density = DensitySetting::Comfortable;
                    s.appearance.log_font = "Menlo".into();
                    s.appearance.ui_font_size = 15;
                },
                window,
                cx,
            )
        })
    })
    .unwrap();
    cx.update(|cx| {
        let t = cx.rogg();
        assert_eq!(t.appearance, Appearance::Dark);
        assert_eq!((t.metrics.log_font_size, t.metrics.row_height), (px(15.), px(33.)));
        assert_eq!(t.fonts.log.as_ref(), "Menlo");
        assert_eq!(t.fonts.ui_size, px(15.));
    });
    let text = std::fs::read_to_string(dir.path().join("settings.toml")).unwrap();
    assert!(text.contains("theme = \"dark\"") && text.contains("log_font = \"Menlo\""), "{text}");
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(rogg_app::actions::ThemeLight), cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(cx.rogg().appearance, Appearance::Light);
        assert_eq!(SettingsStore::get(cx).appearance.theme, ThemeSetting::Light, "View → Theme is a setting too");
    });
}

#[gpui_kit::test]
fn new_tabs_take_the_behavior_defaults(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["error one", "two"]);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update(|cx| {
        SettingsStore::update(cx, |s| {
            s.behavior.wrap = true;
            s.behavior.follow_selection = false;
            s.behavior.context = 2;
            s.behavior.regex = true;
            s.behavior.case_sensitive = true;
        });
    });
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, cx| ws.active_tab().is_some_and(|t| t.read(cx).is_ready(cx))));
    ws.read_with(cx, |ws, cx| {
        let tab = ws.active_tab().unwrap();
        let t = tab.read(cx);
        assert!(t.full.read(cx).wrap() && t.filtered.read(cx).wrap() && !t.follow);
        let spec = &t.doc.read(cx).filter.spec;
        assert!(spec.regex && spec.case_sensitive && spec.context == 2 && spec.terms.is_empty());
    });
}

#[gpui_kit::test]
fn cache_limit_and_scan_depth_apply(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.change_settings(
                |s| {
                    s.behavior.cache_limit_gb = 5;
                    s.behavior.scan_depth = 1;
                },
                window,
                cx,
            )
        })
    })
    .unwrap();
    cx.update(|cx| assert_eq!(CacheStore::get(cx).limit(), 5 << 30));
    let root = dir.path().join("logs");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("top.log"), "x\n").unwrap();
    std::fs::write(root.join("a/b/deep.log"), "y\n").unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![root], window, cx))).unwrap();
    common::wait_until(cx, |cx| common::exists(cx, w, "tree-row-1"));
    assert!(!common::exists(cx, w, "tree-row-2"), "depth 1 lists only the folder's own files");
}

#[gpui_kit::test]
fn unreadable_settings_toast_and_reset(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.toml"), "[appearance\n").unwrap();
    cx.update(|cx| rogg_app::init_with(cx, rogg_app::paths::AppPaths::under(dir.path())));
    let (_w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("settings.toml.bak")), "{:?}", ws.toasts));
    assert!(dir.path().join("settings.toml.bak").exists());
}
```

Append unit tests to `crates/rogg-app/src/tab_view.rs` (create a `#[cfg(test)] mod tests` if there is none):

```rust
#[cfg(test)]
mod tests {
    use super::context_options;

    #[test]
    fn context_options_grow_for_settings_beyond_three() {
        assert_eq!(context_options(2), (vec!["None".into(), "±1 line".into(), "±2 lines".into(), "±3 lines".into()], 2));
        let (options, selected) = context_options(7);
        assert_eq!((options.len(), selected), (5, 4));
        assert_eq!(options[4].as_ref(), "±7 lines");
    }
}
```

Append to the tests in `crates/rogg-app/src/settings.rs` (created in Step 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_follows_the_settings() {
        let mut s = Settings::default();
        assert_eq!(theme_for(&s, WindowAppearance::Dark).appearance, Appearance::Dark);
        assert_eq!(theme_for(&s, WindowAppearance::Light).appearance, Appearance::Light);
        s.appearance.theme = ThemeSetting::Dark;
        s.appearance.log_font = "SF Mono".into();
        let t = theme_for(&s, WindowAppearance::Light);
        assert_eq!(t.appearance, Appearance::Dark);
        assert_eq!(t.fonts.log.as_ref(), rogg_ui::theme::MONO_FONT, "SF Mono is the system monospaced font");
        assert_eq!(t, RoggTheme::new(Appearance::Dark, Density::Compact), "the defaults are the reference theme");
    }
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test settings`
Expected: compile errors.

- [ ] **Step 2: Paths and the store**

`crates/rogg-app/src/paths.rs`:
- add the fields `settings`, `profiles` and `session`;
- `system()` fills them from `rogg_core::paths::{settings_path, profiles_dir, session_path}`;
- `under(dir)` uses `dir.join("settings.toml")`, `dir.join("profiles")` and `dir.join("session.json")`.

Create `crates/rogg-app/src/settings.rs`:

```rust
//! User settings (spec §6.2) as an app global: loaded at start, saved on every change and applied at once.

use crate::cache::{CacheStore, evict_soon};
use gpui_kit::{App, BorrowAppContext, Global, SharedString, WindowAppearance};
use rogg_core::settings::{DEFAULT_LOG_FONT, DensitySetting, LOG_FONTS, Settings, ThemeSetting};
use rogg_ui::theme::{Appearance, Density, MONO_FONT, RoggTheme};
use std::path::PathBuf;

pub struct SettingsStore {
    settings: Settings,
    path: PathBuf,
    /// The file was unreadable and was moved to `settings.toml.bak` (a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for SettingsStore {}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        match Settings::load(&path) {
            Ok((settings, recovered)) => Self { settings, path, recovered, last_error: None },
            Err(e) => Self { settings: Settings::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The current settings; the defaults when the store is not installed.
    pub fn get(cx: &App) -> Settings {
        cx.try_global::<SettingsStore>().map(|s| s.settings.clone()).unwrap_or_default()
    }

    /// Changes, clamps and saves the settings; returns them before and after.
    pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) -> (Settings, Settings) {
        if !cx.has_global::<SettingsStore>() {
            let before = Settings::default();
            let mut after = before.clone();
            f(&mut after);
            return (before, after.clamped());
        }
        cx.update_global::<SettingsStore, _>(|store, _| {
            let before = store.settings.clone();
            let mut after = before.clone();
            f(&mut after);
            store.settings = after.clamped();
            store.last_error = store.settings.save(&store.path).err().map(|e| e.to_string());
            (before, store.settings.clone())
        })
    }
}

pub fn appearance_for(choice: ThemeSetting, window: WindowAppearance) -> Appearance {
    match choice {
        ThemeSetting::Dark => Appearance::Dark,
        ThemeSetting::Light => Appearance::Light,
        ThemeSetting::System => match window {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
            _ => Appearance::Light,
        },
    }
}

pub fn density_for(density: DensitySetting) -> Density {
    match density {
        DensitySetting::Compact => Density::Compact,
        DensitySetting::Comfortable => Density::Comfortable,
    }
}

/// The font family for a log font setting. "SF Mono" is the system monospaced font, whose family name is
/// `.AppleSystemUIFontMonospaced` (the name "SF Mono" silently falls back to Helvetica in gpui).
pub fn log_font_family(name: &str) -> SharedString {
    if name == DEFAULT_LOG_FONT { MONO_FONT.into() } else { SharedString::from(name.to_string()) }
}

/// The curated log fonts installed on this Mac (spec §6.2: fonts that are not available are hidden). SF Mono always is.
pub fn available_log_fonts(cx: &App) -> Vec<String> {
    let installed = cx.text_system().all_font_names();
    LOG_FONTS.iter().filter(|f| **f == DEFAULT_LOG_FONT || installed.iter().any(|n| n == *f)).map(|f| f.to_string()).collect()
}

/// The theme for `settings`, with `theme` choosing light or dark (the settings' choice, or a pinned one).
pub fn theme_with(settings: &Settings, theme: ThemeSetting, window: WindowAppearance) -> RoggTheme {
    let a = &settings.appearance;
    RoggTheme::new(appearance_for(theme, window), density_for(a.density))
        .with_log_font(log_font_family(&a.log_font), a.log_font_size as f32)
        .with_ui_font_size(a.ui_font_size as f32)
}

pub fn theme_for(settings: &Settings, window: WindowAppearance) -> RoggTheme {
    theme_with(settings, settings.appearance.theme, window)
}

/// Installs the theme for the current settings (with `theme` deciding light or dark) unless it is already active;
/// returns whether it changed.
pub fn apply_theme(theme: ThemeSetting, window: WindowAppearance, cx: &mut App) -> bool {
    let next = theme_with(&SettingsStore::get(cx), theme, window);
    if cx.try_global::<RoggTheme>() == Some(&next) {
        return false;
    }
    next.install(cx);
    true
}

/// Applies the cache size limit and trims the cache to it in the background.
pub fn apply_cache_limit(cx: &mut App) {
    CacheStore::get(cx).set_limit(SettingsStore::get(cx).cache_limit_bytes());
    evict_soon(cx);
}
```

`crates/rogg-app/src/lib.rs`:
- add `pub mod settings;`;
- in `init_with`, replace the theme lines with:

```rust
    gpui_kit::init(cx);
    let settings = settings::SettingsStore::load(paths.settings.clone());
    settings::theme_for(settings.settings(), cx.window_appearance()).install(cx);
    cx.set_global(settings);
    install_globals(cx, paths);
```

In `install_globals`:
- start with `if !cx.has_global::<settings::SettingsStore>() { cx.set_global(settings::SettingsStore::load(paths.settings.clone())); }`, for hosts such as the gallery that call it directly;
- create the cache with `settings::SettingsStore::get(cx).cache_limit_bytes()` instead of `DEFAULT_LIMIT_BYTES`.

- [ ] **Step 3: Workspace and keys**

`crates/rogg-app/src/workspace/keys.rs`:
- delete `ThemeChoice` and `appearance_for`;
- map the actions to `ThemeSetting::{System, Light, Dark}` and `DensitySetting::{Compact, Comfortable}`;
- replace `set_theme`, `set_density` and `apply_theme` with:

```rust
    pub fn set_theme(&mut self, theme: ThemeSetting, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_override = None;
        self.change_settings(|s| s.appearance.theme = theme, window, cx);
    }

    pub fn set_density(&mut self, density: DensitySetting, window: &mut Window, cx: &mut Context<Self>) {
        self.change_settings(|s| s.appearance.density = density, window, cx);
    }

    /// Pins light or dark without saving it (the gallery owns its theme).
    pub fn pin_theme(&mut self, theme: ThemeSetting, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_override = Some(theme);
        self.apply_appearance(window, cx);
    }

    /// Changes the settings and applies what changed at once (spec §6.2).
    pub fn change_settings(&mut self, f: impl FnOnce(&mut Settings), window: &mut Window, cx: &mut Context<Self>) {
        let (before, after) = SettingsStore::update(cx, f);
        if before.appearance != after.appearance {
            self.apply_appearance(window, cx);
        }
        if before.behavior.cache_limit_gb != after.behavior.cache_limit_gb {
            crate::settings::apply_cache_limit(cx);
        }
        cx.notify();
    }

    /// Installs the theme for the settings (or the pinned theme) under the window's appearance.
    pub fn apply_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = self.theme_override.unwrap_or(SettingsStore::get(cx).appearance.theme);
        crate::settings::apply_theme(theme, window.appearance(), cx);
        cx.notify();
    }
```

Task 11 makes `apply_appearance` restyle the tabs.

`crates/rogg-app/src/workspace/mod.rs`:
- replace the fields `theme: ThemeChoice` and `density: Density` with `pub(crate) theme_override: Option<ThemeSetting>`, initialised to `None`;
- make the appearance observer call `if ws.theme_override.unwrap_or(SettingsStore::get(cx).appearance.theme) == ThemeSetting::System { ws.apply_appearance(window, cx); }`;
- after the existing recovered toasts in `Workspace::new`, add:

```rust
        if cx.try_global::<crate::settings::SettingsStore>().is_some_and(|s| s.recovered) {
            ws.toast("Settings were unreadable and have been reset; the old file was kept as settings.toml.bak", cx);
        }
```

`crates/rogg-gallery/src/gallery.rs` `new_app`: replace `ThemeChoice` with `rogg_core::settings::ThemeSetting`, and call `ws.pin_theme(choice, window, cx)` instead of `set_theme`. This way the gallery never writes a settings file.

- [ ] **Step 4: Behaviour defaults for new tabs**

`crates/rogg-app/src/tab.rs`. Replace `#[derive(Clone, Debug, Default)] pub struct TabInit { … }` with:

```rust
#[derive(Clone, Debug)]
pub struct TabInit {
    pub doc: DocumentInit,
    /// Display number to select in both panes once the text loads (not reported in the status bar until the user
    /// selects, as in the reference).
    pub selection: Option<u64>,
    pub labels: TabLabels,
    /// Wrap of the full and the filtered log.
    pub wrap: [bool; 2],
    /// Follow selection (spec §5.6).
    pub follow: bool,
    /// Split ratio; None keeps the default.
    pub split: Option<f32>,
    /// Display number to scroll to the top of each pane once the text loads (session restore).
    pub top: [Option<u64>; 2],
}

impl Default for TabInit {
    fn default() -> Self {
        Self { doc: DocumentInit::default(), selection: None, labels: TabLabels::default(), wrap: [false, false], follow: true, split: None, top: [None, None] }
    }
}

impl TabInit {
    /// A tab for a file the user opened: filter flags, context, wrap and follow from the settings (spec §6.2).
    pub fn from_settings(settings: &rogg_core::settings::Settings) -> Self {
        let b = &settings.behavior;
        let spec = rogg_core::filter::FilterSpec { terms: Vec::new(), regex: b.regex, case_sensitive: b.case_sensitive, context: b.context };
        Self { doc: DocumentInit { spec, ..DocumentInit::default() }, wrap: [b.wrap, b.wrap], follow: b.follow_selection, ..Self::default() }
    }
}
```

In `Tab::new`:
- set `follow: init.follow` and `split: init.split.unwrap_or(MainWindowParts::DEFAULT_SPLIT)`;
- keep `init.top` in a new field `pending_top: [Option<u64>; 2]`;
- after building the struct, apply the wrap:

```rust
        if init.wrap[0] {
            tab.full.update(cx, |s, cx| s.set_wrap(true, cx));
        }
        if init.wrap[1] {
            tab.filtered.update(cx, |s, cx| s.set_wrap(true, cx));
        }
```

In `sync_rows`, inside the `if loaded` branch, before the pending-selection handling, scroll each pane to its restored top line:

```rust
        if loaded {
            for (pane, top) in [Pane::Full, Pane::Filtered].into_iter().zip(std::mem::take(&mut self.pending_top)) {
                let state = self.state(pane).clone();
                if let Some(row) = top.and_then(|n| state.read(cx).rows().nearest_row(n)) {
                    state.update(cx, |s, cx| s.scroll_to_row(row, ScrollTo::Top, cx));
                }
            }
        }
```

`crates/rogg-app/src/workspace/chrome.rs`: the three `TabInit::default()` call sites (`open_stream`, `open_file`, `open_source_ref`) become `TabInit::from_settings(&crate::settings::SettingsStore::get(cx))`. `open_folder` passes the depth: `TreeJob::scan(root, dir, crate::settings::SettingsStore::get(cx).behavior.scan_depth as usize)`.

`crates/rogg-app/src/sources.rs`:

```rust
    pub fn scan(node: NodeId, dir: PathBuf, depth: usize) -> TreeJob {
        Self::spawn(node, move |token| TreeResult::Scanned(scan_folder(&dir, depth, token)))
    }
```

Remove the now-unused `DEFAULT_SCAN_DEPTH` import.

`crates/rogg-app/src/tab_view.rs`. Replace the band's `.context(...)` arguments with `context_options`, and keep the existing presets selectable:

```rust
/// The context select's options: the reference presets, plus the tab's own radius when settings chose more than ±3.
pub(crate) fn context_options(context: u32) -> (Vec<SharedString>, usize) {
    let mut options: Vec<SharedString> = CONTEXT_OPTIONS.iter().map(|o| SharedString::from(*o)).collect();
    if (context as usize) < CONTEXT_OPTIONS.len() {
        return (options, context as usize);
    }
    options.push(format!("±{context} lines").into());
    (options, CONTEXT_OPTIONS.len())
}
```

Use it as follows:

```rust
            let (context_items, context_selected) = context_options(spec.context);
            // … .context(context_items, context_selected)
            // … .on_context(tab_act_ix(tab, |t, i, _, cx| if i < CONTEXT_OPTIONS.len() { t.doc.update(cx, |d, cx| d.set_context(i as u32, cx)) }))
```

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. The existing tests use `TabInit::default()` or struct-update syntax, which still compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app crates/rogg-gallery
git commit -m "feat(app): settings store with live appearance, new-tab defaults, cache limit and scan depth

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Profile store and profile-styled rows

**Files:**
- Create: `crates/rogg-app/src/profiles.rs`, `crates/rogg-app/tests/profiles.rs`
- Delete: `crates/rogg-app/src/decorate.rs`
- Modify:
  - `crates/rogg-app/src/rows.rs`, `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`
  - `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/demo.rs`
  - `crates/rogg-app/src/workspace/{mod,keys}.rs`
  - `crates/rogg-app/tests/search.rs` (the `full_rows` signature)

**Interfaces:**
- Consumes:
  - Task 3 `Profile`, `ColorSpec`, `ColorValue`, `derive_light`, `TINT_ALPHA`, `builtin_default`.
  - Task 4 `CompiledProfile`, `RuleError`.
  - Task 7 `SpanStyle::{Mark, Bold}`, `FilterBand::chip_colors`.
  - Task 10 `SettingsStore`, `Workspace::apply_appearance`.
- Produces:
  - `rogg_app::profiles::ProfileStore` (Global) with:
    - `load(PathBuf)`;
    - `revision(&App) -> u64`;
    - `names() -> Vec<String>`, `get(&str) -> &Profile` (falls back to Default), `contains(&str) -> bool`;
    - `compiled(&str) -> Arc<CompiledProfile>`, `errors(&str) -> &[RuleError]`;
    - `put(old_name: &str, Profile)`, `add(Profile) -> String`, `remove(&str) -> bool`, `edit(&str, impl FnOnce(&mut Profile))`;
    - the public fields `recovered: Vec<PathBuf>` and `last_error`.
  - `resolve_color(&ColorSpec, light: Option<&ColorSpec>, background: bool, &Palette, Appearance) -> Color`.
  - `ProfilePaint { rules: Vec<RulePaint { color, tint }>, hits: Vec<HitColor>, chips: Vec<ChipColor> }` with `ProfilePaint::resolve(&Profile, &Palette, Appearance)`.
  - `RowStyler` with:
    - `new(&Profile, Arc<CompiledProfile>, &Palette, Appearance)`, `from_profile(&Profile, &Palette, Appearance)`, `for_profile(&App, &str)`;
    - `decorate(&self, display: &str, filtered: bool) -> (Vec<Span>, Option<RowTint>)`;
    - `hit(slot) -> SpanStyle`, `palette_len()`, `chips() -> Vec<ChipColor>`, `rule_color(i) -> Color`.
  - `FullRows { text, base, pins, search, styler: Option<Arc<RowStyler>> }` and `FilteredRows { …, styler }`.
  - `Document::full_rows(&self, styler: Option<Arc<RowStyler>>)` and `filtered_rows(&self, styler)`.
  - `Tab`:
    - `pub profile: SharedString`;
    - `refresh_style(&mut self, cx)`;
    - `set_profile(&mut self, SharedString, cx)`;
    - `styler() -> Option<Arc<RowStyler>>`.
  - `TabInit.profile: Option<SharedString>`.
  - `Workspace::restyle_tabs(&mut self, cx)`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/profiles.rs`:

```rust
mod common;
use gpui_kit::{AppContext, BorrowAppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::profiles::ProfileStore;
use rogg_app::tab::Tab;
use rogg_core::profile::{ColorSpec, HitEntry, LineRule, Profile, derive_light};
use rogg_core::settings::ThemeSetting;
use rogg_ui::logview::{LogLine, LogRow, RowTint, SpanStyle};
use rogg_ui::theme::{ActiveRoggTheme, Color};

const ERROR_LINE: &str = "2026-10-05 14:32:08.142  ERROR  disk full";

fn open(cx: &mut TestAppContext, lines: &[&str]) -> (tempfile::TempDir, gpui_kit::AnyWindowHandle, Entity<rogg_app::workspace::Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    (dir, w, ws, tab)
}

fn full_row(tab: &Entity<Tab>, cx: &mut TestAppContext, i: usize) -> LogLine {
    tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(i) {
        LogRow::Line(l) => l,
        LogRow::Gap { .. } => panic!("gap"),
    })
}

fn add_profile(cx: &mut TestAppContext, p: Profile) {
    cx.update(|cx| cx.update_global::<ProfileStore, _>(|s, _| s.add(p)));
}

#[gpui_kit::test]
fn rows_follow_the_tab_profile(cx: &mut TestAppContext) {
    let (_dir, _w, _ws, tab) = open(cx, &[ERROR_LINE, "2026-10-05 14:32:09.000  INFO   ok"]);
    let p = cx.update(|cx| cx.rogg().palette);
    let row = full_row(&tab, cx, 0);
    assert_eq!(row.tint, Some(RowTint::Color(p.error_row_tint)));
    assert!(row.spans.iter().any(|s| s.range == (0..23) && s.style == SpanStyle::Fg(p.faint)));
    assert!(row.spans.iter().any(|s| s.style == SpanStyle::Fg(p.error)), "ERROR colours the line");
    assert_eq!(tab.read_with(cx, |t, _| t.profile.clone()).as_ref(), "Default");
    add_profile(cx, Profile { name: "Plain".into(), rules: vec![], hit_palette: vec![HitEntry::slot(0)] });
    tab.update(cx, |t, cx| t.set_profile("Plain".into(), cx));
    let plain = full_row(&tab, cx, 0);
    assert!(plain.tint.is_none() && plain.spans.is_empty());
}

#[gpui_kit::test]
fn editing_a_profile_restyles_open_tabs(cx: &mut TestAppContext) {
    let (dir, _w, ws, tab) = open(cx, &[ERROR_LINE]);
    cx.update(|cx| cx.update_global::<ProfileStore, _>(|s, _| s.edit("Default", |p| p.rules.clear())));
    ws.update(cx, |ws, cx| ws.restyle_tabs(cx));
    assert!(full_row(&tab, cx, 0).tint.is_none());
    let saved = std::fs::read_to_string(dir.path().join("profiles/Default.toml")).unwrap();
    assert!(!saved.contains("[[rules]]"), "{saved}");
}

#[gpui_kit::test]
fn filter_hits_use_the_profile_palette(cx: &mut TestAppContext) {
    let (_dir, _w, _ws, tab) = open(cx, &["disk full", "ok"]);
    let entry = HitEntry { fg: ColorSpec::hex(0x010203ff), bg: ColorSpec::hex(0x040506ff), light_fg: None, light_bg: None };
    add_profile(cx, Profile { name: "Pal".into(), rules: vec![], hit_palette: vec![entry] });
    tab.update(cx, |t, cx| t.set_profile("Pal".into(), cx));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("disk", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count() == 1));
    let row = tab.read_with(cx, |t, cx| match t.filtered.read(cx).rows().row(0) {
        LogRow::Line(l) => l,
        LogRow::Gap { .. } => panic!("gap"),
    });
    assert!(row.spans.iter().any(|s| s.range == (0..4) && s.style == SpanStyle::Mark { fg: Color(0x010203ff), bg: Color(0x040506ff) }), "{:?}", row.spans);
    assert_eq!(tab.read_with(cx, |t, _| t.styler().unwrap().chips().len()), 1);
}

#[gpui_kit::test]
fn the_light_theme_uses_overrides_or_derived_colours(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open(cx, &["disk full"]);
    let derived = LineRule { color: ColorSpec::hex(0xefaaa2ff), ..LineRule::new("disk") };
    let overridden = LineRule { color: ColorSpec::hex(0xefaaa2ff), light: Some(ColorSpec::hex(0x123456ff)), ..LineRule::new("full") };
    add_profile(cx, Profile { name: "Hex".into(), rules: vec![derived, overridden], hit_palette: vec![HitEntry::slot(0)] });
    tab.update(cx, |t, cx| t.set_profile("Hex".into(), cx));
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.set_theme(ThemeSetting::Light, window, cx))).unwrap();
    let row = full_row(&tab, cx, 0);
    assert!(row.spans.contains(&rogg_ui::logview::Span { range: 0..4, style: SpanStyle::Fg(Color(derive_light(0xefaaa2ff, false))) }));
    assert!(row.spans.contains(&rogg_ui::logview::Span { range: 5..9, style: SpanStyle::Fg(Color(0x123456ff)) }));
}

#[gpui_kit::test]
fn an_unreadable_profile_is_reported(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::write(dir.path().join("profiles/Bad.toml"), "rules = 1").unwrap();
    cx.update(|cx| rogg_app::init_with(cx, rogg_app::paths::AppPaths::under(dir.path())));
    let (_w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("Bad.toml.bak")), "{:?}", ws.toasts));
    cx.update(|cx| assert_eq!(cx.global::<ProfileStore>().names(), ["Default"]));
}
```

Add a unit-test module to `crates/rogg-app/src/profiles.rs` (created in Step 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::profile::{builtin_services, HitEntry};

    #[test]
    fn built_in_colours_follow_the_palette_exactly() {
        let p = Palette::dark();
        let paint = ProfilePaint::resolve(&builtin_services(), &p, Appearance::Dark);
        assert_eq!(paint.rules[1].color, p.error);
        assert_eq!(paint.rules[1].tint, p.error_row_tint, "the error tint is the palette's");
        assert_eq!(paint.rules[3].color, p.hits[2].fg);
        assert_eq!(paint.hits, p.hits.to_vec());
        assert_eq!(paint.chips, p.chips.to_vec());
        let light = Palette::light();
        assert_eq!(ProfilePaint::resolve(&builtin_services(), &light, Appearance::Light).rules[2].color, light.amber);
    }

    #[test]
    fn hex_colours_tint_and_derive() {
        let p = Palette::dark();
        let spec = ColorSpec::hex(0x336699ff);
        assert_eq!(resolve_color(&spec, None, false, &p, Appearance::Dark), Color(0x336699ff));
        assert_eq!(resolve_color(&spec, None, true, &p, Appearance::Light), Color(derive_light(0x336699ff, true)));
        assert_eq!(resolve_color(&ColorSpec("bad".into()), None, false, &p, Appearance::Dark), p.log_text);
        let custom = HitEntry { fg: ColorSpec::hex(0xffffffff), bg: ColorSpec::hex(0x000000ff), light_fg: None, light_bg: None };
        let profile = Profile { name: "x".into(), rules: vec![], hit_palette: vec![custom] };
        let paint = ProfilePaint::resolve(&profile, &p, Appearance::Dark);
        assert_eq!(paint.chips[0], ChipColor { border: Color(0x000000ff), bg: Color(0x00000066), term: Color(0xffffffff) });
    }

    #[test]
    fn decorate_orders_line_colour_before_match_colours() {
        let p = Palette::dark();
        let styler = RowStyler::from_profile(&rogg_core::profile::builtin_default(), &p, Appearance::Dark);
        let (spans, tint) = styler.decorate("2026-10-05 14:32:08.142  ERROR  x", false);
        assert_eq!(spans[0], Span { range: 0..32, style: SpanStyle::Fg(p.error) });
        assert_eq!(spans[1], Span { range: 0..23, style: SpanStyle::Fg(p.faint) });
        assert_eq!(tint, Some(RowTint::Color(p.error_row_tint)));
    }
}
```

`segments` lets later foreground spans override earlier ones, so the whole-line colour must come first: then the faint timestamp still wins over its 23 bytes.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test profiles`
Expected: compile errors.

- [ ] **Step 2: The store and paint**

Create `crates/rogg-app/src/profiles.rs`:

```rust
//! Highlighting profiles (spec §6.1) as an app global, and their colours resolved for the active theme.

use gpui_kit::{App, Global};
use rogg_core::profile::{self, ColorSpec, ColorValue, DEFAULT_PROFILE, Profile, TINT_ALPHA, derive_light, unique_name};
use rogg_core::style::{CompiledProfile, RuleError};
use rogg_ui::logview::{RowTint, Span, SpanStyle};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, ChipColor, Color, HitColor, Palette};
use std::path::PathBuf;
use std::sync::Arc;

struct Entry {
    profile: Profile,
    compiled: Arc<CompiledProfile>,
    errors: Vec<RuleError>,
}

fn entry(profile: Profile) -> Entry {
    let (compiled, errors) = CompiledProfile::compile(&profile);
    Entry { profile, compiled: Arc::new(compiled), errors }
}

pub struct ProfileStore {
    dir: PathBuf,
    entries: Vec<Entry>,
    /// Bumped on every change; tabs restyle when it moves.
    revision: u64,
    /// Profile files that were unreadable and were moved to `.bak` (toasts at start).
    pub recovered: Vec<PathBuf>,
    pub last_error: Option<String>,
}

impl Global for ProfileStore {}

impl ProfileStore {
    pub fn load(dir: PathBuf) -> Self {
        match profile::load_dir(&dir) {
            Ok(loaded) => Self { dir, entries: loaded.profiles.into_iter().map(entry).collect(), revision: 0, recovered: loaded.recovered, last_error: None },
            Err(e) => Self { dir, entries: profile::builtins().into_iter().map(entry).collect(), revision: 0, recovered: Vec::new(), last_error: Some(e.to_string()) },
        }
    }

    pub fn revision(cx: &App) -> u64 {
        cx.try_global::<ProfileStore>().map_or(0, |s| s.revision)
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.profile.name.clone()).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.profile.name == name)
    }

    /// The entry named `name`, or Default (always first) when there is none.
    fn find(&self, name: &str) -> &Entry {
        self.entries.iter().find(|e| e.profile.name == name).or_else(|| self.entries.iter().find(|e| e.profile.name == DEFAULT_PROFILE)).unwrap_or(&self.entries[0])
    }

    pub fn get(&self, name: &str) -> &Profile {
        &self.find(name).profile
    }

    pub fn compiled(&self, name: &str) -> Arc<CompiledProfile> {
        self.find(name).compiled.clone()
    }

    pub fn errors(&self, name: &str) -> &[RuleError] {
        &self.find(name).errors
    }

    /// Replaces the profile `old_name` with `profile` (a rename moves its file), recompiles and saves it.
    pub fn put(&mut self, old_name: &str, profile: Profile) {
        let saved = profile::save(&self.dir, &profile);
        if old_name != profile.name && saved.is_ok() {
            let _ = profile::delete(&self.dir, old_name);
        }
        self.last_error = saved.err().map(|e| e.to_string());
        match self.entries.iter().position(|e| e.profile.name == old_name) {
            Some(i) => self.entries[i] = entry(profile),
            None => self.entries.push(entry(profile)),
        }
        self.revision += 1;
    }

    /// Adds `profile` under a unique name (`New profile 2`…); returns that name.
    pub fn add(&mut self, mut profile: Profile) -> String {
        profile.name = unique_name(&profile.name, &self.names());
        let name = profile.name.clone();
        self.put(&name, profile);
        name
    }

    /// Deletes a profile and its file; Default cannot be deleted.
    pub fn remove(&mut self, name: &str) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.profile.name == name) else { return false };
        if name == DEFAULT_PROFILE {
            return false;
        }
        self.entries.remove(i);
        self.last_error = profile::delete(&self.dir, name).err().map(|e| e.to_string());
        self.revision += 1;
        true
    }

    /// Edits the profile named `name` (Default when there is none) and saves it.
    pub fn edit(&mut self, name: &str, f: impl FnOnce(&mut Profile)) {
        let mut profile = self.get(name).clone();
        let old = profile.name.clone();
        f(&mut profile);
        self.put(&old, profile);
    }
}

fn token_color(token: &str, p: &Palette) -> Color {
    match token {
        "text" => p.log_text,
        "muted" => p.muted,
        "faint" => p.faint,
        "accent" => p.accent,
        "error" => p.error,
        "amber" => p.amber,
        other => {
            let slot = hit_slot(other).unwrap_or(0);
            if other.ends_with("-bg") { p.hits[slot].bg } else { p.hits[slot].fg }
        }
    }
}

/// `hit-2` / `hit-2-bg` → 2.
fn hit_slot(token: &str) -> Option<usize> {
    token.strip_prefix("hit-")?.trim_end_matches("-bg").parse().ok().filter(|n| *n < 4)
}

/// A profile colour for the active theme (spec §6.1): tokens follow the theme; a hex value is used as is in dark,
/// and in light through its override or else its derived variant. An unreadable colour falls back to the log text (or
/// the first hit background).
pub fn resolve_color(spec: &ColorSpec, light: Option<&ColorSpec>, background: bool, p: &Palette, appearance: Appearance) -> Color {
    let value = |c: ColorValue| match c {
        ColorValue::Token(t) => token_color(t, p),
        ColorValue::Rgba(v) => Color(v),
    };
    match (appearance, spec.parse()) {
        (_, None) => {
            if background {
                p.hits[0].bg
            } else {
                p.log_text
            }
        }
        (Appearance::Dark, Some(c)) => value(c),
        (Appearance::Light, Some(c)) => match light.and_then(ColorSpec::parse) {
            Some(l) => value(l),
            None => match c {
                ColorValue::Token(t) => token_color(t, p),
                ColorValue::Rgba(v) => Color(derive_light(v, background)),
            },
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulePaint {
    pub color: Color,
    pub tint: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfilePaint {
    pub rules: Vec<RulePaint>,
    pub hits: Vec<HitColor>,
    pub chips: Vec<ChipColor>,
}

impl ProfilePaint {
    pub fn resolve(profile: &Profile, p: &Palette, appearance: Appearance) -> Self {
        let rules = profile
            .rules
            .iter()
            .map(|r| {
                let color = resolve_color(&r.color, r.light.as_ref(), false, p, appearance);
                let error_token = r.color.parse() == Some(ColorValue::Token("error")) && r.light.is_none();
                RulePaint { color, tint: if error_token { p.error_row_tint } else { color.with_alpha(TINT_ALPHA) } }
            })
            .collect();
        let hits: Vec<HitColor> = profile
            .hit_palette
            .iter()
            .map(|h| HitColor { fg: resolve_color(&h.fg, h.light_fg.as_ref(), false, p, appearance), bg: resolve_color(&h.bg, h.light_bg.as_ref(), true, p, appearance) })
            .collect();
        let chips = profile
            .hit_palette
            .iter()
            .zip(&hits)
            .map(|(h, c)| match (h.fg.parse(), h.bg.parse()) {
                (Some(ColorValue::Token(f)), Some(ColorValue::Token(b))) if hit_slot(f).is_some() && hit_slot(f) == hit_slot(b) && b.ends_with("-bg") => {
                    p.chips[hit_slot(f).unwrap_or(0)]
                }
                _ => ChipColor { border: c.bg, bg: c.bg.with_alpha(0x66), term: c.fg },
            })
            .collect();
        Self { rules, hits, chips }
    }
}

/// A profile ready to style rows: the compiled rules and their colours for the active theme.
pub struct RowStyler {
    compiled: Arc<CompiledProfile>,
    paint: ProfilePaint,
}

impl RowStyler {
    pub fn new(profile: &Profile, compiled: Arc<CompiledProfile>, p: &Palette, appearance: Appearance) -> Self {
        Self { compiled, paint: ProfilePaint::resolve(profile, p, appearance) }
    }

    pub fn from_profile(profile: &Profile, p: &Palette, appearance: Appearance) -> Self {
        Self::new(profile, Arc::new(CompiledProfile::compile(profile).0), p, appearance)
    }

    /// The styler for the profile named `name` under the active theme.
    pub fn for_profile(cx: &App, name: &str) -> Self {
        let t = cx.rogg();
        match cx.try_global::<ProfileStore>() {
            Some(store) => Self::new(store.get(name), store.compiled(name), &t.palette, t.appearance),
            None => Self::from_profile(&profile::builtin_default(), &t.palette, t.appearance),
        }
    }

    /// Spans and row tint for a row's display text: the line colour first (later spans override it), then match
    /// colours, then bold ranges.
    pub fn decorate(&self, display: &str, filtered: bool) -> (Vec<Span>, Option<RowTint>) {
        let style = self.compiled.style(display, filtered);
        let color = |rule: usize| self.paint.rules.get(rule).map(|r| r.color).unwrap_or_default();
        let mut spans = Vec::with_capacity(1 + style.match_spans.len() + style.bold.len());
        if let Some(rule) = style.line_color {
            spans.push(Span { range: 0..display.len(), style: SpanStyle::Fg(color(rule)) });
        }
        spans.extend(style.match_spans.into_iter().map(|(range, rule)| Span { range, style: SpanStyle::Fg(color(rule)) }));
        spans.extend(style.bold.into_iter().map(|range| Span { range, style: SpanStyle::Bold }));
        let tint = style.tint.and_then(|rule| self.paint.rules.get(rule)).map(|r| RowTint::Color(r.tint));
        (spans, tint)
    }

    /// The mark style of filter term slot `slot`.
    pub fn hit(&self, slot: usize) -> SpanStyle {
        let h = self.paint.hits[slot % self.paint.hits.len().max(1)];
        SpanStyle::Mark { fg: h.fg, bg: h.bg }
    }

    pub fn palette_len(&self) -> usize {
        self.paint.hits.len().max(1)
    }

    pub fn chips(&self) -> Vec<ChipColor> {
        self.paint.chips.clone()
    }

    /// The swatch colour of rule `i` (the editor).
    pub fn rule_color(&self, i: usize) -> Color {
        self.paint.rules.get(i).map(|r| r.color).unwrap_or_default()
    }

    pub fn hit_colors(&self) -> &[HitColor] {
        &self.paint.hits
    }
}
```

`ProfileStore::load` always yields at least Default. `load_dir` guarantees it, and the error branch uses the built-ins. A profile always has at least one palette entry: `load_dir` and `import` refill an empty palette, and the editor never removes the last slot. `hit` still guards the length with `.max(1)`; on an empty list that index would panic, so the editor rule matters.

`crates/rogg-app/src/lib.rs`:
- add `pub mod profiles;` and remove `pub mod decorate;`;
- in `install_globals`, add `cx.set_global(profiles::ProfileStore::load(paths.profiles.clone()));`.

Delete `crates/rogg-app/src/decorate.rs`.

- [ ] **Step 3: Rows use the styler**

`crates/rogg-app/src/rows.rs`. Replace `decorated` with:

```rust
fn decorated(
    display: SharedString,
    number: u64,
    pinned: bool,
    context: bool,
    hits: Vec<Span>,
    search: Option<&CompiledSearch>,
    styler: Option<&RowStyler>,
    filtered: bool,
) -> LogLine {
    let (mut spans, tint) = styler.map(|s| s.decorate(&display, filtered)).unwrap_or_default();
    spans.extend(hits);
    if let Some(search) = search {
        spans.extend(search.spans(&display).into_iter().map(|range| Span { range, style: SpanStyle::SearchHit }));
    }
    LogLine { number, text: display, spans, pinned, context, tint }
}
```

If clippy reports `too_many_arguments`, group `(search, styler, filtered)` into a small `struct Decor<'a>`.

- Add `pub styler: Option<Arc<RowStyler>>` to `FullRows` and `FilteredRows`.
- `FullRows::row` passes `self.styler.as_deref(), false`.
- `FilteredRows::row` computes the hits with the profile palette:

```rust
                let hits = self
                    .highlighter
                    .as_ref()
                    .map(|h| {
                        let slots = self.styler.as_ref().map_or(HIT_SLOTS, |s| s.palette_len());
                        h.spans(&display, slots)
                            .into_iter()
                            .map(|(range, slot)| Span { range, style: self.styler.as_ref().map_or(SpanStyle::Hit(slot), |s| s.hit(slot)) })
                            .collect()
                    })
                    .unwrap_or_default();
                LogRow::Line(decorated(display, self.base + number, pinned, !matched && !pinned, hits, self.search.as_deref(), self.styler.as_deref(), true))
```

- `Document::full_rows(&self, styler: Option<Arc<RowStyler>>)` and `filtered_rows(&self, styler: Option<Arc<RowStyler>>)` take the styler and store it in the rows.
- Remove `use crate::decorate;` and import `crate::profiles::RowStyler`.

Update the existing unit tests in `rows.rs`:
- `full_rows_number_pin_and_decorate` builds its rows with `styler: Some(Arc::new(RowStyler::from_profile(&rogg_core::profile::builtin_default(), &Palette::dark(), Appearance::Dark)))`;
- the tint assertion becomes `assert_eq!(first.tint, Some(RowTint::Color(Palette::dark().error_row_tint)))`;
- the faint span assertion looks for `Span { range: 0..23, style: SpanStyle::Fg(Palette::dark().faint) }` with `.iter().any(...)`, since the whole-line ERROR colour span comes first;
- the second row (`"plain"`) has no spans and no tint;
- every other `FullRows { … }` / `FilteredRows { … }` literal in the tests gets `styler: None`.

In `crates/rogg-app/tests/search.rs`, change `d.full_rows()` to `d.full_rows(None)`.

- [ ] **Step 4: Tabs carry a profile**

`crates/rogg-app/src/tab.rs`:
- `TabInit` gains `pub profile: Option<SharedString>` (`None` in `Default`). `from_settings` sets `profile: Some(b.default_profile.clone().into())`.
- `Tab` gains:
  - `pub profile: SharedString`;
  - `styler: Option<Arc<RowStyler>>`;
  - `style_key: Option<(u64, Appearance, SharedString)>`;
  - `style_revision: u64`.
- `Applied` gains `style: u64`.
- `Tab::new` sets `profile: init.profile.unwrap_or_else(|| SettingsStore::get(cx).behavior.default_profile.into())`, then calls `tab.refresh_style(cx)` right before `tab.sync_rows(cx)`.

Add:

```rust
    /// Rebuilds the row styler when the profile, its contents (the store revision) or the theme changed; the rows of
    /// both panes are rebuilt with it.
    pub fn refresh_style(&mut self, cx: &mut Context<Self>) {
        let key = (ProfileStore::revision(cx), cx.rogg().appearance, self.profile.clone());
        if self.style_key.as_ref() == Some(&key) {
            return;
        }
        self.styler = Some(Arc::new(RowStyler::for_profile(cx, &self.profile)));
        self.style_key = Some(key);
        self.style_revision += 1;
        self.sync_rows(cx);
    }

    /// The tab's highlighting profile (spec §6.1: each tab has one).
    pub fn set_profile(&mut self, name: SharedString, cx: &mut Context<Self>) {
        self.profile = name;
        self.refresh_style(cx);
        cx.notify();
    }

    pub fn styler(&self) -> Option<Arc<RowStyler>> {
        self.styler.clone()
    }
```

In `sync_rows`:
- set `style: self.style_revision` in `now`;
- add `|| now.style != a.style` to both `full_changed` and `filtered_changed`;
- pass `self.styler.clone()` to `doc.full_rows(..)` and `doc.filtered_rows(..)`;
- keep `only_grew` false when the style changed: add `&& now.style == before.style`.

`crates/rogg-app/src/tab_view.rs`:
- the InfoBar label is `t.labels.profile.clone().unwrap_or_else(|| t.profile.clone())`;
- the band gets `.chip_colors(t.styler().map(|s| s.chips()))`.

`crates/rogg-app/src/demo.rs`, in `init`: set `profile: Some(PROFILE.into())` on the `TabInit` and `profile: None` on the labels. The demo tab now really uses Services.

`crates/rogg-app/src/workspace/mod.rs`:
- add `pub fn restyle_tabs(&mut self, cx: &mut Context<Self>) { for tab in self.tabs.clone() { tab.update(cx, |t, cx| t.refresh_style(cx)); } cx.notify(); }`;
- in `Workspace::new`, toast recovered profiles:

```rust
        let recovered: Vec<String> = cx
            .try_global::<crate::profiles::ProfileStore>()
            .map(|s| s.recovered.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect())
            .unwrap_or_default();
        for file in recovered {
            ws.toast(format!("Profile {file} was unreadable; the old file was kept as {file}.bak"), cx);
        }
```

In `workspace/keys.rs`, `apply_appearance` calls `self.restyle_tabs(cx)` after `apply_theme` returns true.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. Run `just visual-check` too; it must stay at 0.000%, because the replica does not use app rows.

- [ ] **Step 6: Commit**

```bash
git add -A crates/rogg-app crates/rogg-gallery
git commit -m "feat(app): highlighting profiles style both panes per tab; built-ins replace the fixed decoration

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Settings dialog, log font picker, ⌘,

**Files:**
- Create: `crates/rogg-app/src/dialogs/mod.rs`, `crates/rogg-app/src/dialogs/settings.rs`, `crates/rogg-app/src/dialogs/fonts.rs`
- Modify:
  - `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/menus.rs`
  - `crates/rogg-app/src/workspace/{mod,keys}.rs`
  - `crates/rogg-ui/src/dialogs.rs` (a generic `picker`, which `all_tabs` uses)
- Test: `crates/rogg-app/tests/settings.rs` (appended), `crates/rogg-ui/tests/dialogs.rs` (appended)

**Interfaces:**
- Consumes:
  - Task 9 `FormRow`, `SectionLabel`, `Stepper`, `Toggle`.
  - Task 10 `SettingsStore`, `Workspace::change_settings`, `available_log_fonts`.
  - Task 11 `ProfileStore::names`.
- Produces:
  - Action `OpenSettings` (⌘,, listed as `Settings` / `⌘ ,`), and the menu item `rogg → Settings…`.
  - `Dialog::{Settings, FontPicker}`.
  - `Workspace::open_settings(&mut self, cx)` and `Workspace::pick_log_font(&mut self, name: &str, window, cx)`.
  - `rogg_app::dialogs::settings::{render, context_label, log_font_options}`.
  - `rogg_app::dialogs::fonts::{MonoFonts, is_monospace, monospace_fonts, render}`.
  - `rogg_ui::dialogs::picker(title, items: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame` (item ids `picker-{i}`). `all_tabs` = `picker("Open files", …)`.
  - Element ids:
    - `setting-theme` (Segmented; options `setting-theme-0..2`)
    - `setting-ui-size`, `setting-log-font`, `setting-log-size`
    - `setting-density` (`-0`, `-1`)
    - `setting-wrap`, `setting-follow`, `setting-context`, `setting-regex`, `setting-case`, `setting-restore`
    - `setting-cache`, `setting-depth`, `setting-profile`

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-app/tests/settings.rs`:

```rust
#[gpui_kit::test]
fn the_settings_dialog_applies_each_change_at_once(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args::default());
    common::press(cx, w, "cmd-,");
    assert!(common::exists(cx, w, "setting-theme"));
    common::click(cx, w, "setting-theme-2");
    cx.update(|cx| assert_eq!(cx.rogg().appearance, Appearance::Dark));
    common::click(cx, w, "setting-log-size-up");
    common::click(cx, w, "setting-wrap");
    common::click(cx, w, "setting-context-up");
    common::click(cx, w, "setting-context-up");
    common::click(cx, w, "setting-cache-down");
    common::click(cx, w, "setting-depth-down");
    cx.update(|cx| {
        let s = SettingsStore::get(cx);
        assert_eq!((s.appearance.log_font_size, s.behavior.wrap, s.behavior.context, s.behavior.cache_limit_gb, s.behavior.scan_depth), (13, true, 2, 10, 3));
        assert_eq!(CacheStore::get(cx).limit(), 10 << 30);
        assert_eq!(cx.rogg().metrics.log_font_size, px(13.));
    });
    let (saved, _) = rogg_core::settings::Settings::load(&dir.path().join("settings.toml")).unwrap();
    assert_eq!(saved.behavior.context, 2);
    common::press(cx, w, "escape");
    assert!(!common::exists(cx, w, "setting-theme"));
    common::click(cx, w, "settings");
    assert!(common::exists(cx, w, "setting-theme"), "the title bar button opens it too");
}

#[gpui_kit::test]
fn picking_another_log_font_returns_to_settings(cx: &mut TestAppContext) {
    common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            ws.open_dialog(rogg_app::workspace::Dialog::FontPicker, cx);
            ws.pick_log_font("Menlo", window, cx);
        })
    })
    .unwrap();
    cx.update(|cx| assert_eq!(SettingsStore::get(cx).appearance.log_font, "Menlo"));
    assert!(common::exists(cx, w, "setting-theme"));
}

#[test]
fn monospace_detection_and_font_options() {
    use rogg_app::dialogs::fonts::is_monospace;
    use rogg_app::dialogs::settings::{context_label, log_font_options};
    assert!(is_monospace([Some(7.2), Some(7.2), Some(7.2)]));
    assert!(!is_monospace([Some(3.), Some(9.), Some(3.)]));
    assert!(!is_monospace([Some(0.), Some(0.), Some(0.)]));
    assert!(!is_monospace([None, Some(7.), Some(7.)]));
    let (options, selected) = log_font_options(&["SF Mono".into(), "Menlo".into()], "Menlo");
    assert_eq!(options.iter().map(|o| o.as_ref()).collect::<Vec<_>>(), ["SF Mono", "Menlo", "Other…"]);
    assert_eq!(selected, 1);
    let (options, selected) = log_font_options(&["SF Mono".into()], "Iosevka");
    assert_eq!(options.iter().map(|o| o.as_ref()).collect::<Vec<_>>(), ["SF Mono", "Iosevka", "Other…"]);
    assert_eq!(selected, 1);
    assert_eq!([context_label(0), context_label(1), context_label(7)], ["None", "±1 line", "±7 lines"]);
}
```

Append to `crates/rogg-ui/tests/dialogs.rs`:

```rust
#[gpui_kit::test]
fn the_generic_picker_reports_picks(cx: &mut TestAppContext) {
    common::setup(cx);
    let picked = Rc::new(Cell::new(usize::MAX));
    let p = picked.clone();
    let w = common::open(cx, 900., 700., move |_, _| {
        let p = p.clone();
        rogg_ui::dialogs::picker("Choose a log font", &[("Menlo".into(), false), ("Monaco".into(), true)], Rc::new(move |i, _, _| p.set(i))).into_any_element()
    });
    common::click(cx, w, "picker-0");
    assert_eq!(picked.get(), 0);
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test settings && cargo test -p rogg-ui --test dialogs`
Expected: compile errors.

- [ ] **Step 2: The generic picker**

`crates/rogg-ui/src/dialogs.rs`:

```rust
/// A list of choices with the current one checked (`.tab-picker-item` rows), e.g. All tabs or the log font picker.
pub fn picker(title: impl Into<SharedString>, items: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame {
    items.iter().enumerate().fold(DialogFrame::new(title), |d, (i, (label, current))| {
        let on_pick = on_pick.clone();
        d.child(PickerItem { id: format!("picker-{i}").into(), label: label.clone(), current: *current, on_click: Some(Rc::new(move |w, cx| on_pick(i, w, cx))) })
    })
}

pub fn all_tabs(tabs: &[(SharedString, bool)], on_pick: IndexCallback) -> DialogFrame {
    picker("Open files", tabs, on_pick)
}
```

This renders identically to before, and the replica golden `all-tabs` stays at 0.000%.

- [ ] **Step 3: Action, keys and menu**

`crates/rogg-app/src/actions.rs`:
- add `OpenSettings` to the `actions!` list;
- add `shortcut!("Settings", "⌘ ,", "cmd-,", OpenSettings, None),` to the keymap after "Toggle sidebar".

`crates/rogg-app/src/menus.rs`: in the `rogg` menu, put `MenuItem::action("Settings…", OpenSettings), MenuItem::separator(),` first, and update the app-menu item count in the menus test from 5 to 7.

`crates/rogg-app/src/workspace/mod.rs`:
- `Dialog` gains `Settings` and `FontPicker`;
- the title bar `.on_settings(act(this, |ws, _, cx| ws.open_settings(cx)))`;
- the root binds `.on_action(cx.listener(|ws, _: &OpenSettings, _, cx| ws.open_settings(cx)))`. The Settings dialog may replace any open dialog, so it is not guarded by `unless_dialog`;
- `dialog_element` arms:

```rust
            Dialog::Settings => crate::dialogs::settings::render(this, cx).on_close(close).into_any_element(),
            Dialog::FontPicker => crate::dialogs::fonts::render(this, cx).on_close(act(this, |ws, _, cx| ws.open_settings(cx))).into_any_element(),
```

Add to `impl Workspace`:

```rust
    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.open_dialog(Dialog::Settings, cx);
    }

    /// The font picker's choice: saves it and returns to Settings.
    pub fn pick_log_font(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_string();
        self.change_settings(move |s| s.appearance.log_font = name, window, cx);
        self.open_settings(cx);
    }
```

`fonts::render` takes `&App`, but measuring fonts needs a one-time `&mut App` pass. When the picker is opened by choosing "Other…", call `crate::dialogs::fonts::monospace_fonts(cx)` (which caches its result in a global) before `open_dialog(Dialog::FontPicker)`. The render path then reads the cache only.

`crates/rogg-app/src/lib.rs`: `pub mod dialogs;`.

- [ ] **Step 4: The dialog bodies**

`crates/rogg-app/src/dialogs/mod.rs`:

```rust
//! Bodies of the app's dialogs (spec §5.8), built from rogg-ui components over app state.

pub mod fonts;
pub mod settings;
```

`crates/rogg-app/src/dialogs/settings.rs`:

```rust
//! The Settings dialog (spec §6.2): appearance and behaviour. Every change is applied and saved at once.

use crate::profiles::ProfileStore;
use crate::settings::{SettingsStore, available_log_fonts};
use crate::workspace::{Dialog, Workspace};
use gpui_kit::{App, AppContext, Entity, SharedString, Window};
use rogg_core::filter::MAX_CONTEXT;
use rogg_core::settings::{CACHE_LIMITS_GB, DensitySetting, LOG_FONT_SIZES, SCAN_DEPTHS, Settings, ThemeSetting, UI_FONT_SIZES, step_cache_limit};
use rogg_core::stats::noun;
use rogg_ui::controls::{CompactSelect, Segmented};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{FormRow, SectionLabel, Stepper, Toggle};

pub const OTHER_FONT: &str = "Other…";

/// `None`, `±1 line`, `±7 lines`.
pub fn context_label(n: u32) -> String {
    if n == 0 { "None".into() } else { format!("±{n} {}", noun(n as u64, "line", "lines")) }
}

/// The log font select: the curated fonts installed here, the current font when it is another one, then `Other…`.
pub fn log_font_options(available: &[String], current: &str) -> (Vec<SharedString>, usize) {
    let mut options: Vec<SharedString> = available.iter().map(|f| SharedString::from(f.clone())).collect();
    let selected = match options.iter().position(|o| o.as_ref() == current) {
        Some(i) => i,
        None => {
            options.push(current.to_string().into());
            options.len() - 1
        }
    };
    options.push(OTHER_FONT.into());
    (options, selected)
}

fn change(this: &Entity<Workspace>, f: impl Fn(&mut Settings) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s), window, cx))
}

fn change_ix(this: &Entity<Workspace>, f: impl Fn(&mut Settings, usize) + 'static) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s, i), window, cx))
}

fn step(this: &Entity<Workspace>, f: impl Fn(&mut Settings, bool) + 'static) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |up, window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s, up), window, cx))
}

fn bump(v: u32, up: bool) -> u32 {
    if up { v + 1 } else { v.saturating_sub(1) }
}

const THEMES: [ThemeSetting; 3] = [ThemeSetting::System, ThemeSetting::Light, ThemeSetting::Dark];
const DENSITIES: [DensitySetting; 2] = [DensitySetting::Compact, DensitySetting::Comfortable];

pub fn render(this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let s = SettingsStore::get(cx);
    let (a, b) = (&s.appearance, &s.behavior);
    let (font_options, font_selected) = log_font_options(&available_log_fonts(cx), &a.log_font);
    let other = font_options.len() - 1;
    let profiles: Vec<String> = cx.try_global::<ProfileStore>().map(|p| p.names()).unwrap_or_else(|| vec![b.default_profile.clone()]);
    let profile_selected = profiles.iter().position(|n| *n == b.default_profile).unwrap_or(0);
    let pick_font = {
        let this = this.clone();
        let options = font_options.clone();
        move |i: usize, window: &mut Window, cx: &mut App| {
            if i == other {
                crate::dialogs::fonts::monospace_fonts(cx);
                this.update(cx, |ws, cx| ws.open_dialog(Dialog::FontPicker, cx));
            } else if let Some(name) = options.get(i).map(|o| o.to_string()) {
                this.update(cx, |ws, cx| ws.change_settings(move |s| s.appearance.log_font = name, window, cx));
            }
        }
    };
    let names = profiles.clone();
    DialogFrame::new("Settings")
        .description("Changes apply immediately and are saved.")
        .child(SectionLabel::new("Appearance"))
        .child(FormRow::new("row-theme", "Theme").hint("System follows the macOS appearance").control(
            Segmented::new("setting-theme", vec!["System".into(), "Light".into(), "Dark".into()], THEMES.iter().position(|t| *t == a.theme).unwrap_or(0))
                .on_select(change_ix(this, |s, i| s.appearance.theme = THEMES[i.min(2)])),
        ))
        .child(FormRow::new("row-ui-size", "UI font size").control(
            Stepper::new("setting-ui-size", format!("{} px", a.ui_font_size))
                .bounds(a.ui_font_size > *UI_FONT_SIZES.start(), a.ui_font_size < *UI_FONT_SIZES.end())
                .on_step(step(this, |s, up| s.appearance.ui_font_size = bump(s.appearance.ui_font_size, up))),
        ))
        .child(FormRow::new("row-log-font", "Log font").control(CompactSelect::new("setting-log-font", font_options, font_selected).on_select(pick_font)))
        .child(FormRow::new("row-log-size", "Log font size").hint("Comfortable line spacing adds 1 px").control(
            Stepper::new("setting-log-size", format!("{} px", a.log_font_size))
                .bounds(a.log_font_size > *LOG_FONT_SIZES.start(), a.log_font_size < *LOG_FONT_SIZES.end())
                .on_step(step(this, |s, up| s.appearance.log_font_size = bump(s.appearance.log_font_size, up))),
        ))
        .child(FormRow::new("row-density", "Line spacing").control(
            Segmented::new("setting-density", vec!["Compact".into(), "Comfortable".into()], DENSITIES.iter().position(|d| *d == a.density).unwrap_or(0))
                .on_select(change_ix(this, |s, i| s.appearance.density = DENSITIES[i.min(1)])),
        ))
        .child(SectionLabel::new("Behavior"))
        .child(FormRow::new("row-wrap", "Wrap lines in new tabs").control(Toggle::new("setting-wrap", b.wrap).on_click(change(this, |s| s.behavior.wrap = !s.behavior.wrap))))
        .child(FormRow::new("row-follow", "Follow selection in new tabs").control(
            Toggle::new("setting-follow", b.follow_selection).on_click(change(this, |s| s.behavior.follow_selection = !s.behavior.follow_selection)),
        ))
        .child(FormRow::new("row-context", "Context lines in new tabs").control(
            Stepper::new("setting-context", context_label(b.context))
                .bounds(b.context > 0, b.context < MAX_CONTEXT)
                .on_step(step(this, |s, up| s.behavior.context = bump(s.behavior.context, up))),
        ))
        .child(FormRow::new("row-regex", "Regular expressions in new tabs").control(Toggle::new("setting-regex", b.regex).on_click(change(this, |s| s.behavior.regex = !s.behavior.regex))))
        .child(FormRow::new("row-case", "Case-sensitive filters in new tabs").control(
            Toggle::new("setting-case", b.case_sensitive).on_click(change(this, |s| s.behavior.case_sensitive = !s.behavior.case_sensitive)),
        ))
        .child(FormRow::new("row-restore", "Restore session on launch").control(
            Toggle::new("setting-restore", b.restore_session).on_click(change(this, |s| s.behavior.restore_session = !s.behavior.restore_session)),
        ))
        .child(FormRow::new("row-cache", "Cache size limit").hint("Extracted archive members, decompressed files and conversions").control(
            Stepper::new("setting-cache", format!("{} GB", b.cache_limit_gb))
                .bounds(b.cache_limit_gb > CACHE_LIMITS_GB[0], b.cache_limit_gb < CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1])
                .on_step(step(this, |s, up| s.behavior.cache_limit_gb = step_cache_limit(s.behavior.cache_limit_gb, up))),
        ))
        .child(FormRow::new("row-depth", "Folder scan depth").hint("Levels of subfolders listed when a folder opens").control(
            Stepper::new("setting-depth", format!("{} {}", b.scan_depth, noun(b.scan_depth as u64, "level", "levels")))
                .bounds(b.scan_depth > *SCAN_DEPTHS.start(), b.scan_depth < *SCAN_DEPTHS.end())
                .on_step(step(this, |s, up| s.behavior.scan_depth = bump(s.behavior.scan_depth, up))),
        ))
        .child(FormRow::new("row-profile", "Default highlighting profile").control(
            CompactSelect::new("setting-profile", profiles.iter().map(|n| SharedString::from(n.clone())).collect(), profile_selected).on_select(change_ix(this, move |s, i| {
                if let Some(name) = names.get(i) {
                    s.behavior.default_profile = name.clone();
                }
            })),
        ))
}
```

`crates/rogg-app/src/dialogs/fonts.rs`:

```rust
//! "Other…" log fonts (spec §6.2): every installed monospace family. A family counts as monospace when `i`, `W` and
//! `.` have the same advance; measured once per run.

use crate::settings::SettingsStore;
use crate::workspace::Workspace;
use gpui_kit::{App, Entity, Global, SharedString, font, px};
use rogg_ui::dialogs::{DialogFrame, picker};
use std::rc::Rc;

pub struct MonoFonts(pub Vec<String>);

impl Global for MonoFonts {}

pub fn is_monospace(advances: [Option<f32>; 3]) -> bool {
    matches!(advances, [Some(a), Some(b), Some(c)] if a > 0. && (a - b).abs() < 0.01 && (b - c).abs() < 0.01)
}

/// The installed monospace families, measured on first use and cached.
pub fn monospace_fonts(cx: &mut App) -> Vec<String> {
    if let Some(fonts) = cx.try_global::<MonoFonts>() {
        return fonts.0.clone();
    }
    let ts = cx.text_system().clone();
    let fonts: Vec<String> = ts
        .all_font_names()
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .filter(|name| {
            let id = ts.resolve_font(&font(name.clone()));
            is_monospace(['i', 'W', '.'].map(|c| ts.advance(id, px(12.), c).ok().map(|s| f32::from(s.width))))
        })
        .collect();
    cx.set_global(MonoFonts(fonts.clone()));
    fonts
}

pub fn render(this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let fonts = cx.try_global::<MonoFonts>().map(|f| f.0.clone()).unwrap_or_default();
    let current = SettingsStore::get(cx).appearance.log_font;
    let items: Vec<(SharedString, bool)> = fonts.iter().map(|f| (SharedString::from(f.clone()), *f == current)).collect();
    let this = this.clone();
    picker("Choose a log font", &items, Rc::new(move |i, window, cx| {
        if let Some(name) = fonts.get(i).cloned() {
            this.update(cx, |ws, cx| ws.pick_log_font(&name, window, cx));
        }
    }))
    .description("Monospace fonts installed on this Mac.")
}
```

`Workspace::dialog` stays `pub(crate)`. `Dialog` is already `pub` and is reachable as `rogg_app::workspace::Dialog`, which the test uses.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-ui && cargo clippy --workspace --all-targets -- -D warnings && just visual-check`
Expected: PASS, and 0.000%.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app crates/rogg-ui
git commit -m "feat(app): settings dialog with live changes, log font picker and ⌘,

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Profile editor — profiles and the rule list

**Files:**
- Create: `crates/rogg-app/src/dialogs/profiles.rs`, `crates/rogg-app/tests/profile_editor.rs`
- Modify:
  - `crates/rogg-app/src/dialogs/mod.rs`
  - `crates/rogg-app/src/workspace/mod.rs`
  - `crates/rogg-app/src/tab.rs` (`TabEvent::ShowProfiles`), `crates/rogg-app/src/tab_view.rs`

**Interfaces:**
- Consumes:
  - Task 9 `EditableRuleRow`, `PaletteSlot`, `SectionLabel`, `TextField`.
  - Task 11 `ProfileStore`, `RowStyler`, `Tab::set_profile`, `Workspace::restyle_tabs`.
  - Task 10 `SettingsStore`.
- Produces:
  - `rogg_app::dialogs::profiles::ProfileEditor` (Entity, Render) with:
    - `new(tab: Option<WeakEntity<Tab>>, profile: String)` and `profile_name() -> &str`;
    - `selected_rule() -> Option<usize>`, `selected_slot() -> Option<usize>`;
    - `select_profile(&mut self, &str, cx)`, `new_profile(cx)`, `duplicate(cx)`, `delete(cx) -> bool`;
    - `start_rename(window, cx)`, `apply_rename(window, cx)`, `rename_input() -> Option<Entity<InputState>>`;
    - `toggle_rule(i, cx)`, `move_rule(i, up, cx)`, `add_rule(window, cx)`, `delete_rule(i, window, cx)`;
    - `select_rule(Option<usize>, window, cx)`, `select_slot(Option<usize>, window, cx)`.
  - `ProfileEditorEvent::{Close, Changed, Renamed { from: String, to: String }, Toast(SharedString)}`.
  - `Dialog::Profiles(Entity<ProfileEditor>)`, `Workspace::open_profile_editor(&mut self, window, cx)`, `Workspace::profile_editor() -> Option<Entity<ProfileEditor>>`.
  - `TabEvent::ShowProfiles`.
  - Element ids:
    - `profile-select`, `profile-new`, `profile-duplicate`, `profile-rename`, `profile-delete`, `profile-import`, `profile-export`
    - `profile-name`, `profile-rename-apply`, `profile-rename-error`
    - `rule-add`
    - `profile-rule-{i}…` and `palette-slot-{i}` from Task 9

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/profile_editor.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::dialogs::profiles::ProfileEditor;
use rogg_app::profiles::ProfileStore;
use rogg_app::settings::SettingsStore;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub w: AnyWindowHandle,
    pub ws: Entity<Workspace>,
    pub tab: Entity<Tab>,
    pub editor: Entity<ProfileEditor>,
}

pub fn open_editor(cx: &mut TestAppContext) -> Fixture {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["2026-10-05 14:32:08.142  ERROR  disk full", "ok"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    common::click(cx, w, "profile");
    let editor = ws.read_with(cx, |ws, _| ws.profile_editor()).expect("the info bar's Profile button opens the editor");
    Fixture { dir, w, ws, tab, editor }
}

fn names(cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| cx.global::<ProfileStore>().names())
}

fn tab_profile(f: &Fixture, cx: &mut TestAppContext) -> String {
    f.tab.read_with(cx, |t, _| t.profile.to_string())
}

#[gpui_kit::test]
fn the_editor_opens_on_the_tabs_profile(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    assert_eq!(f.editor.read_with(cx, |e, _| e.profile_name().to_string()), "Default");
    assert!(common::exists(cx, f.w, "profile-rule-3"));
    assert!(common::exists(cx, f.w, "palette-slot-3"));
}

#[gpui_kit::test]
fn new_duplicate_and_delete_profiles(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    assert!(names(cx).contains(&"New profile".to_string()));
    assert_eq!(tab_profile(&f, cx), "New profile", "choosing a profile in the editor assigns it to the tab");
    assert!(f.dir.path().join("profiles/New profile.toml").exists());
    common::click(cx, f.w, "profile-duplicate");
    assert!(names(cx).contains(&"New profile copy".to_string()));
    common::click(cx, f.w, "profile-delete");
    assert!(!names(cx).contains(&"New profile copy".to_string()));
    assert_eq!(tab_profile(&f, cx), "Default");
    f.editor.update(cx, |e, cx| e.select_profile("Default", cx));
    common::click(cx, f.w, "profile-delete");
    assert!(names(cx).contains(&"Default".to_string()), "Default cannot be deleted");
}

#[gpui_kit::test]
fn rules_toggle_reorder_add_and_delete(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-rule-1-toggle");
    let enabled = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules[1].enabled);
    assert!(!enabled);
    let tint = f.tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(0) {
        rogg_ui::logview::LogRow::Line(l) => l.tint,
        _ => None,
    });
    assert!(tint.is_none(), "open tabs restyle at once");
    common::click(cx, f.w, "profile-rule-1-down");
    let patterns: Vec<String> = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.iter().map(|r| r.pattern.clone()).collect());
    assert_eq!(patterns[2], r"\bERROR\b");
    common::click(cx, f.w, "rule-add");
    let count = cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.len());
    assert_eq!(count, 5);
    assert_eq!(f.editor.read_with(cx, |e, _| e.selected_rule()), Some(4));
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.delete_rule(4, window, cx))).unwrap();
    assert_eq!(cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules.len()), 4);
}

#[gpui_kit::test]
fn renaming_follows_tabs_and_the_default_setting(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "profile-new");
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.default_profile = "New profile".into());
    });
    common::click(cx, f.w, "profile-rename");
    let input = f.editor.read_with(cx, |e, _| e.rename_input()).unwrap();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.set_value("Ops", window, cx))).unwrap();
    common::click(cx, f.w, "profile-rename-apply");
    assert!(names(cx).contains(&"Ops".to_string()) && !names(cx).contains(&"New profile".to_string()));
    assert_eq!(tab_profile(&f, cx), "Ops");
    cx.update(|cx| assert_eq!(SettingsStore::get(cx).behavior.default_profile, "Ops"));
    assert!(f.dir.path().join("profiles/Ops.toml").exists() && !f.dir.path().join("profiles/New profile.toml").exists());
    common::click(cx, f.w, "profile-rename");
    let input = f.editor.read_with(cx, |e, _| e.rename_input()).unwrap();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.set_value("Default", window, cx))).unwrap();
    common::click(cx, f.w, "profile-rename-apply");
    assert!(common::exists(cx, f.w, "profile-rename-error"));
    assert!(names(cx).contains(&"Ops".to_string()));
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test profile_editor`
Expected: compile errors.

- [ ] **Step 2: The editor entity**

`crates/rogg-app/src/dialogs/profiles.rs`:

```rust
//! The highlighting profile editor (spec §6.1): choose, create, duplicate, rename, delete, import and export profiles;
//! reorder, enable, add, edit and delete rules; edit the filter-match palette. Every change is saved at once and open
//! tabs restyle (the workspace listens for `Changed`).

use crate::profiles::{ProfileStore, RowStyler};
use crate::tab::Tab;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, BorrowAppContext, Context, Entity, EventEmitter, SharedString, Subscription, WeakEntity, Window, div, px};
use rogg_core::profile::{DEFAULT_PROFILE, HitEntry, LineRule, Profile};
use rogg_ui::controls::{ActionButton, CompactSelect};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{EditableRuleRow, PaletteSlot, SectionLabel, TextField};
use rogg_ui::theme::ActiveRoggTheme;

pub enum ProfileEditorEvent {
    Close,
    /// Profiles changed: restyle tabs (and move tabs whose profile is gone to Default).
    Changed,
    /// A profile was renamed: tabs and the default-profile setting follow.
    Renamed { from: String, to: String },
    Toast(SharedString),
}

pub struct ProfileEditor {
    tab: Option<WeakEntity<Tab>>,
    profile: String,
    selected_rule: Option<usize>,
    selected_slot: Option<usize>,
    rename: Option<(Entity<InputState>, Subscription)>,
    rename_error: Option<SharedString>,
}

impl EventEmitter<ProfileEditorEvent> for ProfileEditor {}

/// A callback that runs `f` on the editor.
fn ed(this: &Entity<ProfileEditor>, f: impl Fn(&mut ProfileEditor, &mut Window, &mut Context<ProfileEditor>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |e, cx| f(e, window, cx))
}

impl ProfileEditor {
    pub fn new(tab: Option<WeakEntity<Tab>>, profile: String) -> Self {
        Self { tab, profile, selected_rule: None, selected_slot: None, rename: None, rename_error: None }
    }

    pub fn profile_name(&self) -> &str {
        &self.profile
    }

    pub fn selected_rule(&self) -> Option<usize> {
        self.selected_rule
    }

    pub fn selected_slot(&self) -> Option<usize> {
        self.selected_slot
    }

    pub fn rename_input(&self) -> Option<Entity<InputState>> {
        self.rename.as_ref().map(|(input, _)| input.clone())
    }

    /// Edits the shown profile, saves it and tells the workspace.
    pub(crate) fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Profile)) {
        let name = self.profile.clone();
        cx.update_global::<ProfileStore, _>(|s, _| s.edit(&name, f));
        cx.emit(ProfileEditorEvent::Changed);
        cx.notify();
    }

    fn rule_count(&self, cx: &App) -> usize {
        cx.global::<ProfileStore>().get(&self.profile).rules.len()
    }

    /// Shows `name` and assigns it to the tab the editor was opened from (spec §6.1: each tab has one profile).
    pub fn select_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        self.profile = name.to_string();
        self.selected_rule = None;
        self.selected_slot = None;
        self.rename = None;
        self.rename_error = None;
        if let Some(tab) = self.tab.as_ref().and_then(WeakEntity::upgrade) {
            let name = SharedString::from(name.to_string());
            tab.update(cx, |t, cx| t.set_profile(name, cx));
        }
        cx.notify();
    }

    pub fn new_profile(&mut self, cx: &mut Context<Self>) {
        let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(Profile { name: "New profile".into(), rules: Vec::new(), hit_palette: (0..4).map(HitEntry::slot).collect() }));
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(&name, cx);
    }

    pub fn duplicate(&mut self, cx: &mut Context<Self>) {
        let mut copy = cx.global::<ProfileStore>().get(&self.profile).clone();
        copy.name = format!("{} copy", copy.name);
        let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(copy));
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(&name, cx);
    }

    /// Deletes the shown profile (never Default); the tab moves to Default.
    pub fn delete(&mut self, cx: &mut Context<Self>) -> bool {
        let name = self.profile.clone();
        if !cx.update_global::<ProfileStore, _>(|s, _| s.remove(&name)) {
            return false;
        }
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(DEFAULT_PROFILE, cx);
        true
    }

    pub fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(self.profile.clone()));
        input.update(cx, |i, cx| i.focus(window, cx));
        let sub = cx.subscribe_in(&input, window, |e, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                e.apply_rename(window, cx);
            }
        });
        self.rename = Some((input, sub));
        self.rename_error = None;
        cx.notify();
    }

    /// Renames the shown profile to the input's text; an empty or taken name shows an error instead.
    pub fn apply_rename(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((input, _)) = &self.rename else { return };
        let to = input.read(cx).value().trim().to_string();
        let from = self.profile.clone();
        if to == from {
            self.rename = None;
            cx.notify();
            return;
        }
        if to.is_empty() || cx.global::<ProfileStore>().contains(&to) {
            self.rename_error = Some(if to.is_empty() { "Type a name".into() } else { format!("A profile named {to} already exists").into() });
            cx.notify();
            return;
        }
        let new_name = to.clone();
        cx.update_global::<ProfileStore, _>(|s, _| s.edit(&from, |p| p.name = new_name));
        self.profile = to.clone();
        self.rename = None;
        self.rename_error = None;
        cx.emit(ProfileEditorEvent::Renamed { from, to });
        cx.emit(ProfileEditorEvent::Changed);
        cx.notify();
    }

    pub fn toggle_rule(&mut self, i: usize, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if let Some(r) = p.rules.get_mut(i) {
                r.enabled = !r.enabled;
            }
        });
    }

    pub fn move_rule(&mut self, i: usize, up: bool, cx: &mut Context<Self>) {
        let n = self.rule_count(cx);
        let j = if up { i.checked_sub(1) } else { (i + 1 < n).then_some(i + 1) };
        let Some(j) = j else { return };
        self.edit(cx, |p| p.rules.swap(i, j));
        if self.selected_rule == Some(i) {
            self.selected_rule = Some(j);
        }
    }

    pub fn add_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| p.rules.push(LineRule::new("")));
        let last = self.rule_count(cx) - 1;
        self.select_rule(Some(last), window, cx);
    }

    pub fn delete_rule(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if i < p.rules.len() {
                p.rules.remove(i);
            }
        });
        self.select_rule(None, window, cx);
    }

    /// Selects rule `i` (its editing form opens below it in Task 14).
    pub fn select_rule(&mut self, i: Option<usize>, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_rule = i;
        cx.notify();
    }

    pub fn select_slot(&mut self, i: Option<usize>, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_slot = i;
        cx.notify();
    }
}

impl Render for ProfileEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let store = cx.global::<ProfileStore>();
        let profile = store.get(&self.profile).clone();
        let errors = store.errors(&self.profile).to_vec();
        let names = store.names();
        let theme = cx.rogg().clone();
        let styler = RowStyler::new(&profile, store.compiled(&self.profile), &theme.palette, theme.appearance);
        let selected = names.iter().position(|n| *n == profile.name).unwrap_or(0);
        let pick_names = names.clone();
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .mt(px(10.))
            .child(CompactSelect::new("profile-select", names.iter().map(|n| SharedString::from(n.clone())).collect(), selected).on_select({
                let this = this.clone();
                move |i, _, cx| {
                    if let Some(name) = pick_names.get(i) {
                        let name = name.clone();
                        this.update(cx, |e, cx| e.select_profile(&name, cx));
                    }
                }
            }))
            .child(ActionButton::new("profile-new", "New").on_click(ed(&this, |e, _, cx| e.new_profile(cx))))
            .child(ActionButton::new("profile-duplicate", "Duplicate").on_click(ed(&this, |e, _, cx| e.duplicate(cx))))
            .child(ActionButton::new("profile-rename", "Rename").on_click(ed(&this, |e, window, cx| e.start_rename(window, cx))))
            .child(ActionButton::new("profile-delete", "Delete").on_click(ed(&this, |e, _, cx| {
                e.delete(cx);
            })))
            .child(ActionButton::new("profile-import", "Import…").on_click(ed(&this, |e, window, cx| e.import(window, cx))))
            .child(ActionButton::new("profile-export", "Export…").on_click(ed(&this, |e, window, cx| e.export(window, cx))));
        if let Some((input, _)) = &self.rename {
            toolbar = toolbar
                .child(TextField::new("profile-name", input.clone()).width(160.).error(self.rename_error.is_some()))
                .child(ActionButton::new("profile-rename-apply", "Rename").primary().on_click(ed(&this, |e, window, cx| e.apply_rename(window, cx))));
        }
        let mut frame = DialogFrame::new(format!("Highlighting · {}", profile.name))
            .description("Rules apply top to bottom; for each effect the first matching rule wins. Changes apply immediately.")
            .on_close(ed(&this, |_, _, cx| cx.emit(ProfileEditorEvent::Close)))
            .child(toolbar);
        if let Some(error) = &self.rename_error {
            frame = frame.child(div().id("profile-rename-error").mt(px(4.)).text_size(theme.text_scale().px(10.)).text_color(theme.palette.error).child(error.clone()));
        }
        frame = frame.child(SectionLabel::new("Rules"));
        let count = profile.rules.len();
        for (i, rule) in profile.rules.iter().enumerate() {
            let error = errors.iter().find(|e| e.rule == i).map(|e| SharedString::from(e.message.clone()));
            let title = if rule.pattern.is_empty() { SharedString::from("(empty pattern)") } else { SharedString::from(rule.pattern.clone()) };
            frame = frame.child(
                EditableRuleRow::new(i, styler.rule_color(i), title, rule.summary())
                    .enabled(rule.enabled)
                    .selected(self.selected_rule == Some(i))
                    .error(error)
                    .can_move(i > 0, i + 1 < count)
                    .on_select(ed(&this, move |e, window, cx| {
                        let next = if e.selected_rule == Some(i) { None } else { Some(i) };
                        e.select_rule(next, window, cx);
                    }))
                    .on_toggle(ed(&this, move |e, _, cx| e.toggle_rule(i, cx)))
                    .on_move({
                        let this = this.clone();
                        move |up, _, cx| this.update(cx, |e, cx| e.move_rule(i, up, cx))
                    }),
            );
            if self.selected_rule == Some(i) {
                frame = frame.child(self.rule_form(i, rule, &this, cx));
            }
        }
        frame = frame.child(div().flex().pt(px(10.)).child(ActionButton::new("rule-add", "Add rule").on_click(ed(&this, |e, window, cx| e.add_rule(window, cx)))));
        frame = frame.child(SectionLabel::new("Filter-match palette"));
        let slots = div().flex().flex_wrap().gap(px(6.)).children(styler.hit_colors().iter().enumerate().map(|(i, h)| {
            PaletteSlot::new(i, h.fg, h.bg).selected(self.selected_slot == Some(i)).on_click(ed(&this, move |e, window, cx| {
                let next = if e.selected_slot == Some(i) { None } else { Some(i) };
                e.select_slot(next, window, cx);
            }))
        }));
        frame = frame.child(slots);
        if let Some(slot) = self.selected_slot.filter(|s| *s < profile.hit_palette.len()) {
            frame = frame.child(self.slot_form(slot, &profile, &this, cx));
        }
        frame.note("Profiles are saved in ~/Library/Application Support/rogg/profiles.")
    }
}
```

Task 13 adds placeholder-free stubs of the forms, which Task 14 replaces, plus no-op import and export. They render nothing yet:

```rust
impl ProfileEditor {
    fn rule_form(&self, _i: usize, _rule: &LineRule, _this: &Entity<Self>, _cx: &App) -> gpui_kit::AnyElement {
        div().into_any_element()
    }

    fn slot_form(&self, _slot: usize, _profile: &Profile, _this: &Entity<Self>, _cx: &App) -> gpui_kit::AnyElement {
        div().into_any_element()
    }

    /// Task 14 opens the file panel here.
    pub fn import(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// Task 14 opens the save panel here.
    pub fn export(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}
```

These four methods are the extension points Task 14 fills in. Their bodies are intentionally empty in this task, and no test exercises them yet.

`crates/rogg-app/src/dialogs/mod.rs`: add `pub mod profiles;`.

- [ ] **Step 3: Wiring**

`crates/rogg-app/src/tab.rs`: add `ShowProfiles` to `TabEvent`.

`crates/rogg-app/src/tab_view.rs`: the InfoBar's `.on_profile(tab_act(tab, |_, _, cx| cx.emit(TabEvent::ShowProfiles)))`. This replaces the `later(…)` toast; `later` stays for Export until Task 18.

`crates/rogg-app/src/workspace/mod.rs`:
- `Dialog` gains `Profiles(Entity<crate::dialogs::profiles::ProfileEditor>)`;
- add a field `dialog_subscription: Option<Subscription>`, initialised to `None` and cleared in `close_dialog`;
- `on_tab_event` handles `TabEvent::ShowProfiles => self.open_profile_editor(window, cx)`. Change the `_window` parameter to `window`;
- `dialog_element` adds `Dialog::Profiles(editor) => editor.clone().into_any_element()`.

Add:

```rust
    /// The profile editor for the active tab's profile (spec §6.1).
    pub fn open_profile_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::dialogs::profiles::{ProfileEditor, ProfileEditorEvent};
        let tab = self.active_tab();
        let profile = tab.as_ref().map(|t| t.read(cx).profile.to_string()).unwrap_or_else(|| crate::settings::SettingsStore::get(cx).behavior.default_profile);
        let editor = cx.new(|_| ProfileEditor::new(tab.map(|t| t.downgrade()), profile));
        self.dialog_subscription = Some(cx.subscribe_in(&editor, window, |ws, _, ev: &ProfileEditorEvent, _window, cx| match ev {
            ProfileEditorEvent::Close => ws.close_dialog(cx),
            ProfileEditorEvent::Changed => ws.profiles_changed(cx),
            ProfileEditorEvent::Renamed { from, to } => ws.profile_renamed(from, to, cx),
            ProfileEditorEvent::Toast(message) => ws.toast(message.clone(), cx),
        }));
        self.open_dialog(Dialog::Profiles(editor), cx);
    }

    pub fn profile_editor(&self) -> Option<Entity<crate::dialogs::profiles::ProfileEditor>> {
        match &self.dialog {
            Some(Dialog::Profiles(editor)) => Some(editor.clone()),
            _ => None,
        }
    }

    /// After any profile change: tabs whose profile was deleted move to Default, every tab restyles.
    fn profiles_changed(&mut self, cx: &mut Context<Self>) {
        for tab in self.tabs.clone() {
            let gone = cx.try_global::<crate::profiles::ProfileStore>().is_some_and(|s| !s.contains(&tab.read(cx).profile));
            if gone {
                tab.update(cx, |t, cx| t.set_profile(rogg_core::profile::DEFAULT_PROFILE.into(), cx));
            }
        }
        self.restyle_tabs(cx);
    }

    fn profile_renamed(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        for tab in self.tabs.clone() {
            if tab.read(cx).profile.as_ref() == from {
                let to = SharedString::from(to.to_string());
                tab.update(cx, |t, cx| t.set_profile(to, cx));
            }
        }
        if crate::settings::SettingsStore::get(cx).behavior.default_profile == from {
            let to = to.to_string();
            crate::settings::SettingsStore::update(cx, |s| s.behavior.default_profile = to);
        }
    }
```

The `Renamed` event is emitted before `Changed`. `profile_renamed` moves the tabs to the new name first, so `profiles_changed` does not see them as orphaned.

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): profile editor with profile management and the rule list

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Profile editor — rule form, palette, import and export

**Files:**
- Modify: `crates/rogg-app/src/dialogs/profiles.rs`
- Test: `crates/rogg-app/tests/profile_editor.rs` (appended)

**Interfaces:**
- Consumes:
  - Task 13 `ProfileEditor` and its extension points: `rule_form`, `slot_form`, `import`, `export`, `select_rule`, `select_slot`.
  - Task 3 `profile::{import, export, parse_hex, ColorSpec, Panes, MAX_PALETTE, COLOR_TOKENS, EXPORT_EXT}`.
  - Task 9 `TextField`, `Swatch`.
  - M0 `FlagButton`, `Segmented`.
- Produces:
  - `ProfileEditor::rule_inputs() -> Option<(Entity<InputState>, Entity<InputState>, Entity<InputState>)>` (pattern, colour, light) and `slot_inputs() -> Option<[Entity<InputState>; 4]>` (fg, bg, light fg, light bg).
  - `color_error() -> bool`.
  - `import_profile_from(&mut self, &Path, cx)` and `export_profile_to(&mut self, &Path, cx)`.
  - Element ids:
    - `rule-form`, `rule-pattern`, `rule-regex`, `rule-case`
    - `rule-effect-match`, `rule-effect-line`, `rule-effect-tint`, `rule-effect-bold`
    - `rule-panes` (`-0` full, `-1` filtered, `-2` both)
    - `rule-swatch-<token>` for the ten swatch tokens `text`, `muted`, `faint`, `accent`, `error`, `amber`, `hit-0`…`hit-3`
    - `rule-color`, `rule-light`, `rule-delete`
    - `slot-form`, `slot-fg`, `slot-bg`, `slot-light-fg`, `slot-light-bg`, `slot-add`, `slot-remove`

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-app/tests/profile_editor.rs`:

```rust
fn select_rule(f: &Fixture, cx: &mut TestAppContext, i: usize) {
    cx.update_window(f.w, |_, window, cx| f.editor.update(cx, |e, cx| e.select_rule(Some(i), window, cx))).unwrap();
}

/// Replaces an input's text the way typing does (`replace_all` emits `InputEvent::Change`).
fn type_into(f: &Fixture, cx: &mut TestAppContext, input: &gpui_kit::Entity<gpui_kit::component::input::InputState>, text: &str) {
    let text = text.to_string();
    cx.update_window(f.w, |_, window, cx| input.update(cx, |i, cx| i.replace_all(text, window, cx))).unwrap();
    cx.run_until_parked();
}

fn rule(cx: &mut TestAppContext, i: usize) -> rogg_core::profile::LineRule {
    cx.update(|cx| cx.global::<ProfileStore>().get("Default").rules[i].clone())
}

#[gpui_kit::test]
fn rule_form_edits_apply_immediately(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 0);
    assert!(common::exists(cx, f.w, "rule-form"));
    let (pattern, _, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &pattern, "disk");
    common::click(cx, f.w, "rule-effect-bold");
    common::click(cx, f.w, "rule-panes-1");
    common::click(cx, f.w, "rule-swatch-amber");
    common::click(cx, f.w, "rule-regex");
    let r = rule(cx, 0);
    assert_eq!(r.pattern, "disk");
    assert!(r.bold && !r.regex, "the timestamp rule was a regex; the flag toggled it off");
    assert_eq!(r.panes, rogg_core::profile::Panes::Filtered);
    assert_eq!(r.color, rogg_core::profile::ColorSpec::token("amber"));
    let saved = std::fs::read_to_string(f.dir.path().join("profiles/Default.toml")).unwrap();
    assert!(saved.contains("pattern = \"disk\""), "{saved}");
}

#[gpui_kit::test]
fn an_invalid_pattern_shows_its_error_and_keeps_other_rules(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 3);
    let (pattern, _, _) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &pattern, "(unclosed");
    assert!(cx.update(|cx| cx.global::<ProfileStore>().errors("Default").iter().any(|e| e.rule == 3)));
    assert!(common::exists(cx, f.w, "profile-rule-3-error"));
    let tint = f.tab.read_with(cx, |t, cx| match t.full.read(cx).rows().row(0) {
        rogg_ui::logview::LogRow::Line(l) => l.tint,
        _ => None,
    });
    assert!(tint.is_some(), "the ERROR rule still styles the row");
}

#[gpui_kit::test]
fn colour_fields_accept_tokens_and_hex_and_flag_the_rest(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    select_rule(&f, cx, 2);
    let (_, color, light) = f.editor.read_with(cx, |e, _| e.rule_inputs()).unwrap();
    type_into(&f, cx, &color, "#123456");
    assert_eq!(rule(cx, 2).color.0, "#123456");
    type_into(&f, cx, &color, "nope");
    assert_eq!(rule(cx, 2).color.0, "#123456", "an unreadable colour is not applied");
    assert!(f.editor.read_with(cx, |e, _| e.color_error()));
    type_into(&f, cx, &light, "#abcdef");
    assert_eq!(rule(cx, 2).light.map(|c| c.0).as_deref(), Some("#abcdef"));
    type_into(&f, cx, &light, "");
    assert_eq!(rule(cx, 2).light, None, "an empty light colour derives it again");
}

#[gpui_kit::test]
fn palette_slots_add_edit_and_remove(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    common::click(cx, f.w, "palette-slot-0");
    common::click(cx, f.w, "slot-add");
    let len = || cx.update(|cx| cx.global::<ProfileStore>().get("Default").hit_palette.len());
    assert_eq!(len(), 5);
    common::click(cx, f.w, "palette-slot-4");
    let [fg, _, _, _] = f.editor.read_with(cx, |e, _| e.slot_inputs()).unwrap();
    type_into(&f, cx, &fg, "#010203");
    assert_eq!(cx.update(|cx| cx.global::<ProfileStore>().get("Default").hit_palette[4].fg.0.clone()), "#010203");
    common::click(cx, f.w, "slot-remove");
    assert_eq!(len(), 4);
}

#[gpui_kit::test]
fn profiles_import_and_export(cx: &mut TestAppContext) {
    let f = open_editor(cx);
    let out = f.dir.path().join("shared.roggprofile");
    f.editor.update(cx, |e, cx| e.export_profile_to(&out, cx));
    assert!(out.exists());
    f.editor.update(cx, |e, cx| e.import_profile_from(&out, cx));
    assert!(names(cx).contains(&"Default 2".to_string()), "an imported name that is taken gets a number");
    assert_eq!(f.editor.read_with(cx, |e, _| e.profile_name().to_string()), "Default 2");
    std::fs::write(&out, "rules = 1").unwrap();
    f.editor.update(cx, |e, cx| e.import_profile_from(&out, cx));
    f.ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("shared.roggprofile")), "{:?}", ws.toasts));
}
```

`palette_slots_add_edit_and_remove` borrows `cx` in the `len` closure and also mutably in `common::click`. Write the length reads inline instead of through the closure if the borrow checker rejects it.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test profile_editor`
Expected: compile errors (`rule_inputs`, `slot_inputs`, `color_error`, `import_profile_from`, `export_profile_to`).

- [ ] **Step 2: Inputs that follow the selection**

Add fields to `ProfileEditor`:

```rust
    /// Pattern, colour and light-colour inputs of the selected rule, with their subscriptions.
    rule_inputs: Option<([Entity<InputState>; 3], Vec<Subscription>)>,
    /// Foreground, background and their light overrides of the selected palette slot.
    slot_inputs: Option<([Entity<InputState>; 4], Vec<Subscription>)>,
    color_error: bool,
```

Initialise them to `None` / `false`, and clear them in `select_profile`. Then replace `select_rule` and `select_slot`:

```rust
    pub fn select_rule(&mut self, i: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_rule = i;
        self.color_error = false;
        self.rule_inputs = i.and_then(|i| cx.global::<ProfileStore>().get(&self.profile).rules.get(i).cloned()).map(|rule| {
            let pattern = cx.new(|cx| InputState::new(window, cx).default_value(rule.pattern.clone()).placeholder("Text or pattern"));
            let color = cx.new(|cx| InputState::new(window, cx).default_value(rule.color.0.clone()).placeholder("#rrggbb or token"));
            let light = cx.new(|cx| InputState::new(window, cx).default_value(rule.light.map(|l| l.0).unwrap_or_default()).placeholder("auto"));
            let subs = vec![
                cx.subscribe_in(&pattern, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let text = input.read(cx).value().to_string();
                        e.edit(cx, |p| {
                            if let Some(r) = p.rules.get_mut(i) {
                                r.pattern = text;
                            }
                        });
                    }
                }),
                cx.subscribe_in(&color, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let spec = ColorSpec(input.read(cx).value().trim().to_string());
                        e.color_error = spec.parse().is_none();
                        if !e.color_error {
                            e.edit(cx, |p| {
                                if let Some(r) = p.rules.get_mut(i) {
                                    r.color = spec;
                                }
                            });
                        }
                        cx.notify();
                    }
                }),
                cx.subscribe_in(&light, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let text = input.read(cx).value().trim().to_string();
                        let spec = (!text.is_empty()).then(|| ColorSpec(text));
                        if spec.as_ref().is_none_or(|s| s.parse().is_some()) {
                            e.edit(cx, |p| {
                                if let Some(r) = p.rules.get_mut(i) {
                                    r.light = spec;
                                }
                            });
                        }
                    }
                }),
            ];
            ([pattern, color, light], subs)
        });
        cx.notify();
    }

    pub fn select_slot(&mut self, i: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_slot = i;
        self.slot_inputs = i.and_then(|i| cx.global::<ProfileStore>().get(&self.profile).hit_palette.get(i).cloned()).map(|h| {
            let make = |text: String, placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| InputState::new(window, cx).default_value(text).placeholder(placeholder))
            };
            let inputs = [
                make(h.fg.0.clone(), "text colour", window, cx),
                make(h.bg.0.clone(), "background", window, cx),
                make(h.light_fg.map(|c| c.0).unwrap_or_default(), "auto", window, cx),
                make(h.light_bg.map(|c| c.0).unwrap_or_default(), "auto", window, cx),
            ];
            let subs = inputs
                .iter()
                .enumerate()
                .map(|(field, input)| {
                    cx.subscribe_in(input, window, move |e, input, ev: &InputEvent, _, cx| {
                        let (InputEvent::Change, Some(slot)) = (ev, e.selected_slot) else { return };
                        let text = input.read(cx).value().trim().to_string();
                        let spec = ColorSpec(text.clone());
                        let optional = field >= 2;
                        if !(spec.parse().is_some() || (optional && text.is_empty())) {
                            return;
                        }
                        e.edit(cx, |p| {
                            let Some(h) = p.hit_palette.get_mut(slot) else { return };
                            match field {
                                0 => h.fg = spec,
                                1 => h.bg = spec,
                                2 => h.light_fg = (!text.is_empty()).then_some(spec),
                                _ => h.light_bg = (!text.is_empty()).then_some(spec),
                            }
                        });
                    })
                })
                .collect();
            (inputs, subs)
        });
        cx.notify();
    }

    pub fn rule_inputs(&self) -> Option<(Entity<InputState>, Entity<InputState>, Entity<InputState>)> {
        self.rule_inputs.as_ref().map(|([p, c, l], _)| (p.clone(), c.clone(), l.clone()))
    }

    pub fn slot_inputs(&self) -> Option<[Entity<InputState>; 4]> {
        self.slot_inputs.as_ref().map(|(inputs, _)| inputs.clone())
    }

    pub fn color_error(&self) -> bool {
        self.color_error
    }

    /// A swatch click: the token becomes the rule colour (its light override is dropped) and the colour field shows it.
    fn set_token(&mut self, i: usize, token: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if let Some(r) = p.rules.get_mut(i) {
                r.color = ColorSpec::token(token);
                r.light = None;
            }
        });
        self.color_error = false;
        if let Some(([_, color, light], _)) = &self.rule_inputs {
            color.update(cx, |c, cx| c.set_value(token, window, cx));
            light.update(cx, |l, cx| l.set_value("", window, cx));
        }
    }
```

`set_value` does not emit `Change` (gpui-base 0.7.1), so a swatch click never loops back through the subscription.

- [ ] **Step 3: The forms**

Replace the Task 13 stubs of `rule_form` and `slot_form`:

```rust
const SWATCH_TOKENS: [&str; 10] = ["text", "muted", "faint", "accent", "error", "amber", "hit-0", "hit-1", "hit-2", "hit-3"];

fn form_line(label: &'static str, cx: &App) -> gpui_kit::Div {
    let s = cx.rogg().text_scale();
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().w(px(74.)).flex_none().text_size(s.px(11.)).text_color(cx.rogg().palette.muted).child(label))
}

impl ProfileEditor {
    fn rule_form(&self, i: usize, rule: &LineRule, this: &Entity<Self>, cx: &App) -> gpui_kit::AnyElement {
        let Some(([pattern, color, light], _)) = &self.rule_inputs else { return div().into_any_element() };
        let p = cx.rogg().palette;
        let has_error = cx.global::<ProfileStore>().errors(&self.profile).iter().any(|e| e.rule == i);
        let flag = |id: &'static str, label: &'static str, on: bool, f: fn(&mut LineRule)| {
            FlagButton::new(id, label).pressed(on).on_click(ed(this, move |e, _, cx| {
                e.edit(cx, |p| {
                    if let Some(r) = p.rules.get_mut(i) {
                        f(r)
                    }
                })
            }))
        };
        let derived = match rule.color.parse() {
            Some(ColorValue::Rgba(v)) => format!("auto · {}", ColorSpec::hex(derive_light(v, false)).0),
            _ => "follows the theme".to_string(),
        };
        let panes = [Panes::Full, Panes::Filtered, Panes::Both];
        div()
            .id("rule-form")
            .flex()
            .flex_col()
            .gap(px(8.))
            .pl(px(37.))
            .pr(px(4.))
            .pb(px(12.))
            .child(
                form_line("Pattern", cx)
                    .child(TextField::new("rule-pattern", pattern.clone()).mono(true).width(240.).error(has_error))
                    .child(flag("rule-regex", ".*", rule.regex, |r| r.regex = !r.regex))
                    .child(flag("rule-case", "Aa", rule.case_sensitive, |r| r.case_sensitive = !r.case_sensitive)),
            )
            .child(
                form_line("Effects", cx)
                    .child(flag("rule-effect-match", "Match text", rule.match_color, |r| r.match_color = !r.match_color))
                    .child(flag("rule-effect-line", "Line text", rule.line_color, |r| r.line_color = !r.line_color))
                    .child(flag("rule-effect-tint", "Row tint", rule.row_tint, |r| r.row_tint = !r.row_tint))
                    .child(flag("rule-effect-bold", "Bold", rule.bold, |r| r.bold = !r.bold)),
            )
            .child(form_line("Panels", cx).child(
                Segmented::new("rule-panes", vec!["Full log".into(), "Filtered log".into(), "Both".into()], panes.iter().position(|x| *x == rule.panes).unwrap_or(2)).on_select({
                    let this = this.clone();
                    move |k, _, cx| this.update(cx, |e, cx| e.edit(cx, |p| if let Some(r) = p.rules.get_mut(i) { r.panes = panes[k.min(2)] }))
                }),
            ))
            .child(
                form_line("Colour", cx)
                    .children(SWATCH_TOKENS.iter().map(|token| {
                        let color = crate::profiles::resolve_color(&ColorSpec::token(token), None, false, &p, cx.rogg().appearance);
                        Swatch::new(SharedString::from(format!("rule-swatch-{token}")), color)
                            .selected(rule.color == ColorSpec::token(token))
                            .on_click(ed(this, move |e, window, cx| e.set_token(i, token, window, cx)))
                    }))
                    .child(TextField::new("rule-color", color.clone()).mono(true).width(96.).error(self.color_error)),
            )
            .child(form_line("Light theme", cx).child(TextField::new("rule-light", light.clone()).mono(true).width(96.)).child(
                div().text_size(cx.rogg().text_scale().px(10.)).text_color(p.faint).child(derived),
            ))
            .child(div().flex().child(ActionButton::new("rule-delete", "Delete rule").on_click(ed(this, move |e, window, cx| e.delete_rule(i, window, cx)))))
            .into_any_element()
    }

    fn slot_form(&self, slot: usize, profile: &Profile, this: &Entity<Self>, cx: &App) -> gpui_kit::AnyElement {
        let Some(([fg, bg, light_fg, light_bg], _)) = &self.slot_inputs else { return div().into_any_element() };
        let len = profile.hit_palette.len();
        div()
            .id("slot-form")
            .flex()
            .flex_col()
            .gap(px(8.))
            .pt(px(10.))
            .child(form_line("Colours", cx).child(TextField::new("slot-fg", fg.clone()).mono(true).width(96.)).child(TextField::new("slot-bg", bg.clone()).mono(true).width(96.)))
            .child(form_line("Light theme", cx).child(TextField::new("slot-light-fg", light_fg.clone()).mono(true).width(96.)).child(TextField::new("slot-light-bg", light_bg.clone()).mono(true).width(96.)))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .when(len < MAX_PALETTE, |d| {
                        d.child(ActionButton::new("slot-add", "Add colour").on_click(ed(this, move |e, window, cx| {
                            e.edit(cx, |p| p.hit_palette.push(HitEntry::slot(p.hit_palette.len())));
                            let last = cx.global::<ProfileStore>().get(&e.profile).hit_palette.len() - 1;
                            e.select_slot(Some(last), window, cx);
                        })))
                    })
                    .when(len > 1, |d| {
                        d.child(ActionButton::new("slot-remove", "Remove colour").on_click(ed(this, move |e, window, cx| {
                            e.edit(cx, |p| {
                                if slot < p.hit_palette.len() && p.hit_palette.len() > 1 {
                                    p.hit_palette.remove(slot);
                                }
                            });
                            e.select_slot(None, window, cx);
                        })))
                    }),
            )
            .into_any_element()
    }
```

In the test, "Add colour" is reached by first selecting slot 0 (`palette-slot-0`), which shows the slot form with its `slot-add` button.

Add these imports to `dialogs/profiles.rs`:
- `rogg_core::profile::{ColorSpec, ColorValue, MAX_PALETTE, Panes, derive_light, EXPORT_EXT}`;
- `rogg_ui::controls::{FlagButton, Segmented}` and `rogg_ui::form::Swatch`;
- `std::path::Path`.

- [ ] **Step 4: Import and export**

Replace the Task 13 no-op `import` and `export`:

```rust
    /// Import… (spec §6.1 `.roggprofile`): a taken name gets a number; a broken file is reported, never renamed.
    pub fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Import".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |e, cx| e.import_profile_from(&path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn import_profile_from(&mut self, path: &Path, cx: &mut Context<Self>) {
        match rogg_core::profile::import(path) {
            Ok(profile) => {
                let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(profile));
                cx.emit(ProfileEditorEvent::Changed);
                cx.emit(ProfileEditorEvent::Toast(format!("Imported profile {name}").into()));
                self.select_profile(&name, cx);
            }
            Err(e) => cx.emit(ProfileEditorEvent::Toast(format!("Could not import {}: {e}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).into())),
        }
    }

    /// Export…: the shown profile as `<name>.roggprofile`.
    pub fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let suggested = format!("{}.{EXPORT_EXT}", self.profile);
        let rx = cx.prompt_for_new_path(&rogg_core::paths::home_dir(), Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |e, cx| e.export_profile_to(&path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn export_profile_to(&mut self, path: &Path, cx: &mut Context<Self>) {
        let profile = cx.global::<ProfileStore>().get(&self.profile).clone();
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let message = match rogg_core::profile::export(path, &profile) {
            Ok(()) => format!("Exported profile to {file}"),
            Err(e) => format!("Could not export {file}: {e}"),
        };
        cx.emit(ProfileEditorEvent::Toast(message.into()));
    }
```

`rogg_core::paths::home_dir` is already public (M1).

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

If `replace_all` turns out not to emit `InputEvent::Change` in gpui-base 0.7.1, change `type_into` to call `i.replace_all(...)` followed by `cx.emit(InputEvent::Change)` inside the input's update. Record that as a ruling.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): profile rule form, palette slots, import and export

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 15: Statistics popover and richer history entries

**Files:**
- Create: `crates/rogg-app/src/dialogs/stats.rs`, `crates/rogg-app/tests/stats.rs`
- Modify:
  - `crates/rogg-core/src/history.rs` (`HistoryEntry::summary`)
  - `crates/rogg-app/src/dialogs/mod.rs`, `crates/rogg-app/src/tab.rs` (`TabEvent::ShowStatistics`), `crates/rogg-app/src/tab_view.rs`
  - `crates/rogg-app/src/workspace/mod.rs`

**Interfaces:**
- Consumes:
  - M1 `FilterStats`, `TermHits`, `group_digits`, `noun`; `FilterState.{stats, progress}`.
  - M0 `rogg_ui::dialogs::statistics(rows)` (title `Filtered log statistics`, rows `stat-{i}`).
  - Task 7 `StatusBar::on_stats`.
- Produces:
  - `rogg_core::history::HistoryEntry::summary(&self) -> String`, e.g. `2 include · 1 exclude · 1 off · regex · case · ±2`.
  - `rogg_app::dialogs::stats`:
    - `rows(stats: Option<&FilterStats>, progress: Option<f32>) -> Vec<(SharedString, SharedString)>`;
    - `percent(part: u64, total: u64) -> String`;
    - `duration_label(Duration) -> String`.
  - `Dialog::Statistics`, rendered live from the active tab.
  - `TabEvent::ShowStatistics`.

- [ ] **Step 1: Write the failing tests**

Append to the tests of `crates/rogg-core/src/history.rs`:

```rust
    #[test]
    fn summaries_show_states_flags_and_context() {
        let spec = FilterSpec {
            terms: vec![Term::include("a"), Term::include("b"), Term::new("c", TermState::Exclude), Term::new("d", TermState::Off)],
            regex: true,
            case_sensitive: true,
            context: 2,
        };
        assert_eq!(HistoryEntry::from_spec(&spec).summary(), "2 include · 1 exclude · 1 off · regex · case · ±2");
        assert_eq!(entry("a").summary(), "1 include · 0 exclude · 0 off");
    }
```

This assumes `FilterSpec::summary()` returns `"N include · N exclude · N off"`. Check it in `filter/spec.rs`, and if its format differs, make the test's second assertion match it.

Create `crates/rogg-app/tests/stats.rs`:

```rust
mod common;
use gpui_kit::{AppContext, TestAppContext};
use rogg_app::args::Args;
use rogg_app::dialogs::stats::{duration_label, percent, rows};
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, TermHits};
use std::time::Duration;

fn sample() -> FilterStats {
    FilterStats {
        total_lines: 40,
        matched: 6,
        pinned: 1,
        pinned_unmatched: 1,
        context_lines: 3,
        term_hits: vec![
            TermHits { text: "error".into(), state: TermState::Include, lines: 7 },
            TermHits { text: "healthcheck".into(), state: TermState::Exclude, lines: 4 },
            TermHits { text: "retry".into(), state: TermState::Off, lines: 0 },
        ],
        duration: Duration::from_micros(3_400),
    }
}

#[test]
fn statistics_rows_read_like_the_spec() {
    let r: Vec<(String, String)> = rows(Some(&sample()), None).into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    assert_eq!(
        r,
        [
            ("Matched lines", "6 of 40 (15%)"),
            ("Pinned lines", "1 (1 not matching)"),
            ("Context lines", "3"),
            ("error", "include · 7 lines"),
            ("healthcheck", "exclude · 4 lines"),
            ("retry", "off"),
            ("Scan time", "3.4 ms"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    let running = rows(Some(&sample()), Some(0.45));
    assert_eq!(running[0].1.as_ref(), "6 of 40 (15%) · scanning 45%");
    assert_eq!(rows(None, None)[0].0.as_ref(), "No filter has run yet");
    assert_eq!([percent(1, 3), percent(0, 0), percent(1, 10_000_000), percent(40, 40)], ["33.3%", "0%", "<0.1%", "100%"]);
    assert_eq!([duration_label(Duration::from_micros(400)), duration_label(Duration::from_millis(1_240))], ["0.4 ms", "1.24 s"]);
}

#[gpui_kit::test]
fn the_footer_opens_live_statistics(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["error one", "ok", "error two"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some()));
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some()));
    common::click(cx, w, "footer-stats");
    assert!(common::exists(cx, w, "stat-0") && common::exists(cx, w, "stat-3"));
    doc.update(cx, |d, cx| d.set_expression("error || ok", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.as_ref().is_some_and(|s| s.matched == 3)));
    assert!(common::exists(cx, w, "stat-4"), "the open popover follows the new filter");
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core history && cargo test -p rogg-app --test stats`
Expected: compile errors.

- [ ] **Step 2: Implement**

`crates/rogg-core/src/history.rs`, in `impl HistoryEntry`:

```rust
    /// `2 include · 1 exclude · 1 off · regex · case · ±2` — states, then the flags and context that are set.
    pub fn summary(&self) -> String {
        let mut parts = vec![self.to_spec().summary()];
        if self.regex {
            parts.push("regex".into());
        }
        if self.case_sensitive {
            parts.push("case".into());
        }
        if self.context > 0 {
            parts.push(format!("±{}", self.context.min(MAX_CONTEXT)));
        }
        parts.join(" · ")
    }
```

`crates/rogg-app/src/workspace/mod.rs`, in the History dialog arm: use `e.summary()` instead of `e.to_spec().summary()`.

`crates/rogg-app/src/dialogs/stats.rs`:

```rust
//! The statistics popover (spec §4.11): matched / total, pins, context, per-term hits and the scan time of the active
//! tab's filter, rendered live from the document.

use gpui_kit::SharedString;
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, group_digits, noun};
use std::time::Duration;

/// `15%`, `33.3%`, `<0.1%`; `0%` of nothing.
pub fn percent(part: u64, total: u64) -> String {
    if total == 0 || part == 0 {
        return "0%".into();
    }
    let p = part as f64 * 100. / total as f64;
    if p < 0.1 {
        "<0.1%".into()
    } else if (p - p.round()).abs() < 0.05 {
        format!("{}%", p.round() as u64)
    } else {
        format!("{p:.1}%")
    }
}

/// `0.4 ms`, `250 ms`, `1.24 s`.
pub fn duration_label(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.;
    if ms < 10. {
        format!("{ms:.1} ms")
    } else if ms < 1000. {
        format!("{} ms", ms.round() as u64)
    } else {
        format!("{:.2} s", ms / 1000.)
    }
}

fn state_label(state: TermState) -> &'static str {
    match state {
        TermState::Include => "include",
        TermState::Exclude => "exclude",
        TermState::Off => "off",
    }
}

pub fn rows(stats: Option<&FilterStats>, progress: Option<f32>) -> Vec<(SharedString, SharedString)> {
    let Some(s) = stats else { return vec![("No filter has run yet".into(), "Add a filter term to see statistics".into())] };
    let mut matched = format!("{} of {} ({})", group_digits(s.matched), group_digits(s.total_lines), percent(s.matched, s.total_lines));
    if let Some(f) = progress {
        matched.push_str(&format!(" · scanning {:.0}%", f * 100.));
    }
    let pinned = if s.pinned_unmatched > 0 { format!("{} ({} not matching)", group_digits(s.pinned), group_digits(s.pinned_unmatched)) } else { group_digits(s.pinned) };
    let mut out: Vec<(SharedString, SharedString)> =
        vec![("Matched lines".into(), matched.into()), ("Pinned lines".into(), pinned.into()), ("Context lines".into(), group_digits(s.context_lines).into())];
    for t in &s.term_hits {
        let value = match t.state {
            TermState::Off => "off".to_string(),
            state => format!("{} · {} {}", state_label(state), group_digits(t.lines), noun(t.lines, "line", "lines")),
        };
        out.push((t.text.clone().into(), value.into()));
    }
    out.push(("Scan time".into(), duration_label(s.duration).into()));
    out
}
```

`crates/rogg-app/src/dialogs/mod.rs`: add `pub mod stats;`.

`crates/rogg-app/src/tab.rs`: add `ShowStatistics` to `TabEvent`.

`crates/rogg-app/src/tab_view.rs`: the `StatusBar` gets `.on_stats(tab_act(tab, |_, _, cx| cx.emit(TabEvent::ShowStatistics)))`.

`crates/rogg-app/src/workspace/mod.rs`:
- `Dialog::Statistics`;
- `on_tab_event` maps `TabEvent::ShowStatistics => self.open_dialog(Dialog::Statistics, cx)`;
- add this `dialog_element` arm:

```rust
            Dialog::Statistics => {
                let rows = self
                    .active_tab()
                    .map(|t| {
                        let d = t.read(cx).doc.read(cx);
                        crate::dialogs::stats::rows(d.filter.stats.as_ref(), d.filter.progress)
                    })
                    .unwrap_or_else(|| crate::dialogs::stats::rows(None, None));
                dialogs::statistics(&rows).description("Counts for the active tab's filter; they update as it runs.").on_close(close).into_any_element()
            }
```

The workspace re-renders when a tab notifies (it observes its tabs), so an open popover updates while a scan publishes.

- [ ] **Step 3: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core && cargo test -p rogg-app && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. The app history test (if it asserts summaries) uses the new format: update the expected strings in `crates/rogg-app/tests/*.rs` that contain `include · ` summaries.

- [ ] **Step 4: Commit**

```bash
git add crates/rogg-core crates/rogg-app
git commit -m "feat(app): statistics popover from the footer; history entries show flags and context

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Original line numbers and "Open filtered as tab"

**Files:**
- Create:
  - `crates/rogg-app/src/numbers.rs`, `crates/rogg-app/src/derived.rs`
  - `crates/rogg-app/tests/derived.rs`
- Modify:
  - `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/rows.rs`
  - `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`
  - `crates/rogg-app/src/workspace/{mod,chrome}.rs`
  - `crates/rogg-app/src/sidebar_model.rs`, `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/menus.rs`, `crates/rogg-app/src/lib.rs`
  - Tests that construct `FullRows` / `FilteredRows` or call `number_base()`

**Interfaces:**
- Consumes:
  - Task 6 `derived::{materialize, rebuild, Derived}`, `Cache::derived_path`.
  - Task 5 `DerivedSpec`, `SessionSource`.
  - Task 1 `clock::local_hms`.
  - Task 11 `Tab::profile`, `RowStyler`.
- Produces:
  - `rogg_app::numbers::Numbers::{Offset(u64), Mapped(Arc<Vec<u64>>)}` with:
    - `display(line) -> u64`;
    - `line_of(number) -> Option<u64>`;
    - `nearest_line(number, count) -> Option<u64>`.
  - `Document::numbers(&self) -> &Numbers`, which replaces `number_base()`. `DocumentInit.number_base` stays (the demo).
  - `FullRows { text, numbers: Numbers, pins, search, styler }` and `FilteredRows { text, numbers, view, highlighter, search, styler }`.
  - `rogg_app::derived`:
    - `DerivedSource { title, crumbs, tooltip, snapshot: Option<String>, spec: Option<DerivedSpec>, origin: DerivedOrigin }`;
    - `DerivedOrigin::{Lines { parent: Arc<LogText>, numbers: Numbers, lines: Arc<Vec<u64>> }, Rebuild}`;
    - `DerivedSource::restored(spec: DerivedSpec) -> DerivedSource`;
    - `DerivedJob`.
  - `Source::Derived(Arc<DerivedSource>)`.
  - `Document::derived_file(&self) -> Option<&Path>`. The file is deleted when the document drops.
  - Action `OpenAsTab` (⇧⌘T); `Workspace::open_as_tab(&mut self, window, cx)`.
  - `Tab`:
    - `pub crumb_tabs: Vec<Option<EntityId>>`, the tabs behind each breadcrumb segment except the last;
    - `crumb_tab(&self, i, cx) -> Option<EntityId>`.
  - `TabEvent::{FocusCrumb(usize), OpenAsTab}`.
  - `TabInit.crumb_tabs: Vec<Option<EntityId>>`.
  - `OpenFile.derived: bool`; derived tabs group under `Filtered tabs` in the sidebar.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/derived.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::Document;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Entity<Tab> {
    ws.read_with(cx, |ws, _| ws.active_tab().unwrap())
}

fn doc(tab: &Entity<Tab>, cx: &mut TestAppContext) -> Entity<Document> {
    tab.read_with(cx, |t, _| t.doc.clone())
}

fn ready(tab: &Entity<Tab>, cx: &mut TestAppContext) {
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx) && t.doc.read(cx).filter.progress.is_none()));
}

fn filter(tab: &Entity<Tab>, expr: &str, cx: &mut TestAppContext) {
    let d = doc(tab, cx);
    d.update(cx, |d, cx| d.set_expression(expr, cx)).unwrap();
    ready(tab, cx);
}

fn numbers(tab: &Entity<Tab>, cx: &mut TestAppContext) -> Vec<u64> {
    tab.read_with(cx, |t, cx| {
        let rows = t.full.read(cx).rows().clone();
        (0..rows.row_count()).filter_map(|i| rows.number_at(i)).collect()
    })
}

fn open(cx: &mut TestAppContext, lines: &[&str]) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = active(&ws, cx);
    ready(&tab, cx);
    (dir, w, ws, tab)
}

#[gpui_kit::test]
fn open_as_tab_freezes_the_filtered_rows(cx: &mut TestAppContext) {
    let (_dir, w, ws, parent) = open(cx, &["a error", "b", "c error", "d", "e error"]);
    filter(&parent, "error", cx);
    doc(&parent, cx).update(cx, |d, cx| d.toggle_pin(1, cx));
    ready(&parent, cx);
    common::click(cx, w, "tool-open-tab");
    let child = active(&ws, cx);
    assert_ne!(child.entity_id(), parent.entity_id());
    ready(&child, cx);
    assert_eq!(child.read_with(cx, |t, cx| t.title(cx)).as_ref(), "a.log ⟩ filtered");
    assert_eq!(numbers(&child, cx), [1, 2, 3, 5]);
    filter(&parent, "zzz", cx);
    assert_eq!(numbers(&child, cx), [1, 2, 3, 5], "frozen at creation");
    let crumbs = child.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs());
    assert_eq!(crumbs.last().map(|c| c.as_ref()), Some("filtered"));
    let parent_crumb = crumbs.len() - 2;
    let crumb_id: &'static str = Box::leak(format!("crumb-{parent_crumb}").into_boxed_str());
    common::click(cx, w, crumb_id);
    assert_eq!(active(&ws, cx).entity_id(), parent.entity_id(), "a breadcrumb focuses the tab behind it");
}

#[gpui_kit::test]
fn nested_derived_tabs_keep_original_numbers(cx: &mut TestAppContext) {
    let lines = ["0", "1", "x2", "3", "4", "x y5", "6", "7", "x y8", "9"];
    let (_dir, w, ws, root) = open(cx, &lines);
    filter(&root, "x", cx);
    common::press(cx, w, "shift-cmd-t");
    let a = active(&ws, cx);
    ready(&a, cx);
    assert_eq!(numbers(&a, cx), [3, 6, 9]);
    filter(&a, "y", cx);
    common::press(cx, w, "shift-cmd-t");
    let b = active(&ws, cx);
    ready(&b, cx);
    assert_eq!(numbers(&b, cx), [6, 9]);
    assert!(b.update(cx, |t, cx| t.go_to_line(9, cx)));
    assert_eq!(b.read_with(cx, |t, _| t.selection()), Some(9));
    assert!(!b.update(cx, |t, cx| t.go_to_line(3, cx)), "line 3 is not in this tab");
    b.update(cx, |t, cx| t.toggle_pin(9, cx));
    assert!(doc(&b, cx).read_with(cx, |d, _| d.filter.pins.contains(&1)), "pins are stored as the tab's own lines");
    let row = b.read_with(cx, |t, cx| t.full.read(cx).rows().row_for_number(6));
    assert_eq!(row, Some(0));
    let crumbs = b.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs());
    assert_eq!(&crumbs[crumbs.len() - 2..], ["filtered", "filtered"]);
    let a_crumb: &'static str = Box::leak(format!("crumb-{}", crumbs.len() - 2).into_boxed_str());
    common::click(cx, w, a_crumb);
    assert_eq!(active(&ws, cx).entity_id(), a.entity_id());
}

#[gpui_kit::test]
fn a_live_parent_gives_a_snapshot_and_streams_are_not_restorable(cx: &mut TestAppContext) {
    common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("feed", || Ok(std::io::Cursor::new(b"error one\nok\nerror two\n".to_vec())), window, cx))).unwrap();
    let parent = active(&ws, cx);
    common::wait_until(cx, |cx| parent.read_with(cx, |t, cx| t.doc.read(cx).text().is_some_and(|x| x.line_count() >= 3)));
    filter(&parent, "error", cx);
    common::press(cx, w, "shift-cmd-t");
    let child = active(&ws, cx);
    ready(&child, cx);
    assert!(common::exists(cx, w, "snapshot"));
    let restorable = child.read_with(cx, |t, cx| match &t.doc.read(cx).source {
        rogg_app::document::Source::Derived(d) => d.spec.is_some(),
        _ => panic!("not derived"),
    });
    assert!(!restorable, "a stream parent cannot be rebuilt (spec §6.4: stdin tabs are not restored)");
}

#[gpui_kit::test]
fn open_as_tab_refuses_an_empty_view_and_closing_deletes_the_copy(cx: &mut TestAppContext) {
    let (_dir, w, ws, parent) = open(cx, &["a", "b"]);
    filter(&parent, "zzz", cx);
    common::press(cx, w, "shift-cmd-t");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1);
    assert!(ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains("No lines to open"))));
    filter(&parent, "a", cx);
    common::press(cx, w, "shift-cmd-t");
    let child = active(&ws, cx);
    ready(&child, cx);
    let file = doc(&child, cx).read_with(cx, |d, _| d.derived_file().map(|p| p.to_path_buf())).unwrap();
    assert!(file.exists());
    // The test's handle would keep the tab (and its document) alive past the close.
    drop(child);
    common::press(cx, w, "cmd-w");
    cx.run_until_parked();
    assert!(!file.exists(), "closing a derived tab deletes its copy");
}
```

Add a unit test module to `crates/rogg-app/src/numbers.rs` (Step 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_and_maps_convert_both_ways() {
        let o = Numbers::Offset(100);
        assert_eq!((o.display(0), o.line_of(105), o.line_of(99)), (100, Some(5), None));
        assert_eq!(o.nearest_line(500, 10), Some(9));
        let m = Numbers::Mapped(Arc::new(vec![3, 6, 9]));
        assert_eq!((m.display(1), m.line_of(9), m.line_of(4)), (6, Some(2), None));
        assert_eq!((m.nearest_line(4, 3), m.nearest_line(1, 3), m.nearest_line(99, 3)), (Some(1), Some(0), Some(2)));
        assert_eq!(Numbers::Mapped(Arc::new(vec![])).nearest_line(1, 0), None);
    }
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test derived`
Expected: compile errors.

- [ ] **Step 2: `Numbers` replaces the number base**

Create `crates/rogg-app/src/numbers.rs`:

```rust
//! Display numbers (spec §4.4, §4.9): a file shows line `i` as `base + i`; a derived tab shows each line's number in
//! the original source. Engine indices stay 0-based; convert only here.

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum Numbers {
    /// Line `i` is `base + i` (files: 1; the demo: 120480).
    Offset(u64),
    /// Line `i` is `numbers[i]` (ascending).
    Mapped(Arc<Vec<u64>>),
}

impl Default for Numbers {
    fn default() -> Self {
        Numbers::Offset(1)
    }
}

impl Numbers {
    pub fn display(&self, line: u64) -> u64 {
        match self {
            Numbers::Offset(base) => base + line,
            Numbers::Mapped(n) => n.get(line as usize).copied().unwrap_or(0),
        }
    }

    /// The line showing `number`, if any (the caller checks it against the line count).
    pub fn line_of(&self, number: u64) -> Option<u64> {
        match self {
            Numbers::Offset(base) => number.checked_sub(*base),
            Numbers::Mapped(n) => n.binary_search(&number).ok().map(|i| i as u64),
        }
    }

    /// The first line numbered at or after `number`, else the last line; None when there are no lines.
    pub fn nearest_line(&self, number: u64, count: u64) -> Option<u64> {
        if count == 0 {
            return None;
        }
        let line = match self {
            Numbers::Offset(base) => number.saturating_sub(*base),
            Numbers::Mapped(n) => n.partition_point(|&x| x < number) as u64,
        };
        Some(line.min(count - 1))
    }
}
```

`crates/rogg-app/src/lib.rs`: add `pub mod numbers;` and `pub mod derived;`.

`crates/rogg-app/src/document/mod.rs`:
- the field `number_base: u64` becomes `numbers: Numbers`, initialised to `Numbers::Offset(init.number_base)`;
- `number_base()` is replaced by `pub fn numbers(&self) -> &Numbers`.

`crates/rogg-app/src/rows.rs`:
- `FullRows.base: u64` becomes `numbers: Numbers`, and the same in `FilteredRows`;
- `FullRows`:
  - `row`: `self.numbers.display(i)`;
  - `row_for_number`: `self.numbers.line_of(number).filter(|&l| l < self.text.line_count()).map(|l| l as usize)`;
  - `number_at`: `((ix as u64) < count).then(|| self.numbers.display(ix as u64))`;
  - `nearest_row`: `self.numbers.nearest_line(number, self.text.line_count()).map(|l| l as usize)`.
- `FilteredRows`:
  - `row`: `self.numbers.display(number)`;
  - `row_for_number`: `self.numbers.line_of(number).and_then(|l| self.view.row_for_line(l)).map(|r| r as usize)`;
  - `number_at`: `self.numbers.display(number)`;
  - `nearest_row`: `let line = match self.numbers.line_of(number) { Some(l) => l, None => self.numbers.nearest_line(number, u64::MAX).unwrap_or(0) };`, then the existing partition over `shown`.
- `Document::full_rows` / `filtered_rows` pass `numbers: self.numbers.clone()`.
- The unit tests construct `numbers: Numbers::Offset(100)` (or `Offset(1)`) instead of `base: 100` (or `base: 1`).

`crates/rogg-app/src/tab.rs`:
- `toggle_pin`: `if let Some(line) = self.doc.read(cx).numbers().line_of(number) { self.doc.update(cx, |d, cx| d.toggle_pin(line, cx)); }`;
- `jump_to_hit`:
  - `let numbers = self.doc.read(cx).numbers().clone();`
  - `let from = self.selection.and_then(|n| numbers.line_of(n));`
  - `self.select(pane, numbers.display(line), true, cx);`

Fix any other `number_base()` caller the compiler reports. The demo passes `DocumentInit { number_base: FIRST_LINE_NUMBER, .. }` unchanged.

- [ ] **Step 3: Derived sources and their job**

Create `crates/rogg-app/src/derived.rs`:

```rust
//! "Open filtered as tab" (spec §4.9, §5.6): a tab whose text is a frozen copy of another tab's filtered rows, with the
//! original line numbers. Built on a background thread (`materialize`, or `rebuild` from a session).

use crate::numbers::Numbers;
use rogg_core::cache::{Cache, CacheLease};
use rogg_core::derived::{Derived, materialize, rebuild};
use rogg_core::session::{DerivedSpec, SessionSource};
use rogg_core::text::LogText;
use rogg_core::{CancelToken, CoreError};
use gpui_kit::SharedString;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub enum DerivedOrigin {
    /// From an open tab: its text, its numbers and the lines its filtered log showed.
    Lines { parent: Arc<LogText>, numbers: Numbers, lines: Arc<Vec<u64>> },
    /// From its definition (session restore, `.rogg`).
    Rebuild,
}

pub struct DerivedSource {
    /// `api.log ⟩ filtered`.
    pub title: SharedString,
    /// The parent's breadcrumb plus `filtered`.
    pub crumbs: Vec<SharedString>,
    /// The filter it was made with.
    pub tooltip: SharedString,
    /// `HH:MM:SS` when the parent was live.
    pub snapshot: Option<String>,
    /// How to rebuild it; None when the parent cannot be reopened (a stream, in-memory text).
    pub spec: Option<DerivedSpec>,
    pub origin: DerivedOrigin,
}

impl std::fmt::Debug for DerivedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedSource").field("title", &self.title).finish()
    }
}

/// Title and breadcrumb of a session source, for derived tabs restored without their parent tab.
fn describe(source: &SessionSource) -> (SharedString, Vec<SharedString>) {
    match source {
        SessionSource::File { path } => {
            let r = rogg_core::source::SourceRef::File(path.clone());
            (r.name().into(), r.crumbs().into_iter().map(SharedString::from).collect())
        }
        SessionSource::Member { source } => (source.name().into(), source.crumbs().into_iter().map(SharedString::from).collect()),
        SessionSource::Derived(inner) => {
            let d = DerivedSource::restored((**inner).clone());
            (d.title, d.crumbs)
        }
    }
}

pub fn expression_label(spec: &rogg_core::filter::FilterSpec) -> SharedString {
    let expr = spec.expression();
    if expr.is_empty() { "Pinned lines".into() } else { expr.into() }
}

impl DerivedSource {
    pub fn restored(spec: DerivedSpec) -> Self {
        let (parent_title, mut crumbs) = describe(&spec.parent);
        crumbs.push("filtered".into());
        Self {
            title: format!("{parent_title} ⟩ filtered").into(),
            crumbs,
            tooltip: expression_label(&spec.filter),
            snapshot: spec.snapshot.clone(),
            spec: Some(spec),
            origin: DerivedOrigin::Rebuild,
        }
    }
}

pub(crate) type DerivedResult = Box<Result<(Derived, Vec<CacheLease>), CoreError>>;

enum Message {
    Progress(f32),
    Done(DerivedResult),
}

pub(crate) enum DerivedPoll {
    Pending(Option<f32>),
    Done(DerivedResult),
    Died,
}

pub(crate) struct DerivedJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
}

impl DerivedJob {
    pub(crate) fn spawn(source: Arc<DerivedSource>, cache: Arc<Cache>) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let _detached = std::thread::Builder::new().name("rogg-derived".into()).spawn(move || {
            let progress = tx.clone();
            let report = move |done: u64, total: u64| {
                let _ = progress.send(Message::Progress(if total == 0 { 1. } else { done as f32 / total as f32 }));
            };
            let result = match &source.origin {
                DerivedOrigin::Lines { parent, numbers, lines } => materialize(parent, lines, |l| numbers.display(l), &cache, &token, report).map(|d| (d, Vec::new())),
                DerivedOrigin::Rebuild => match &source.spec {
                    Some(spec) => rebuild(spec, &cache, &token, report).map(|r| (r.derived, r.leases)),
                    None => Err(CoreError::Parse { path: Default::default(), message: "this tab cannot be rebuilt".into() }),
                },
            };
            let _ = tx.send(Message::Done(Box::new(result)));
        });
        Self { cancel, rx }
    }

    pub(crate) fn poll(&self) -> DerivedPoll {
        let mut last = None;
        loop {
            match self.rx.try_recv() {
                Ok(Message::Progress(f)) => last = Some(f),
                Ok(Message::Done(result)) => return DerivedPoll::Done(result),
                Err(TryRecvError::Empty) => return DerivedPoll::Pending(last),
                Err(TryRecvError::Disconnected) => return DerivedPoll::Died,
            }
        }
    }
}

impl Drop for DerivedJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
```

`crates/rogg-app/src/document/mod.rs`:
- `Source` gains `Derived(Arc<crate::derived::DerivedSource>)`;
- `title()` → `d.title.clone()`, `crumbs()` → `d.crumbs.clone()`, `tooltip()` → `d.tooltip.clone()`, `location()` → `d.title.clone()`;
- `path()` and `source_ref()` → `None`.

`Document` gains:
- `derived_job: Option<crate::derived::DerivedJob>`;
- `derived_file: Option<PathBuf>`;
- `pub fn derived_file(&self) -> Option<&Path> { self.derived_file.as_deref() }`.

In `start_load`, add the arm:

```rust
            Source::Derived(source) => {
                self.derived_job = Some(crate::derived::DerivedJob::spawn(source, CacheStore::get(cx)));
                self.load = LoadState::Loading { fraction: 0., label: "Building tab".into() };
                cx.notify();
                self.ensure_polling(cx);
            }
```

`stop_jobs` also sets `self.derived_job = None`, and `has_jobs` includes `self.derived_job.is_some()`. In `poll_jobs`, call a new `self.poll_derived(cx)` after `poll_open`:

```rust
    fn poll_derived(&mut self, cx: &mut Context<Self>) {
        let Some(job) = &self.derived_job else { return };
        match job.poll() {
            crate::derived::DerivedPoll::Pending(Some(f)) => {
                self.load = LoadState::Loading { fraction: f, label: "Building tab".into() };
                cx.notify();
            }
            crate::derived::DerivedPoll::Pending(None) => {}
            crate::derived::DerivedPoll::Done(result) => {
                self.derived_job = None;
                match *result {
                    Ok((derived, leases)) => {
                        if let Some(old) = self.derived_file.replace(derived.path.clone()) {
                            let _ = std::fs::remove_file(old);
                        }
                        self.numbers = Numbers::Mapped(Arc::new(derived.numbers));
                        // A derived text never grows: no watch, no follower.
                        self.finish_load(Ok(OpenedSource { text: derived.text, leases, inflating: None, watch: None }), cx);
                    }
                    Err(e) => self.finish_load(Err(e), cx),
                }
            }
            crate::derived::DerivedPoll::Died => {
                self.derived_job = None;
                self.load = LoadState::Failed { message: "Building the tab stopped unexpectedly".into() };
                cx.notify();
            }
        }
    }
```

In `impl Drop for Document`, add `if let Some(path) = self.derived_file.take() { let _ = std::fs::remove_file(path); }`.

- [ ] **Step 4: Opening, breadcrumbs, snapshot, keys**

`crates/rogg-app/src/actions.rs`: add `OpenAsTab` and the keymap row `shortcut!("Open filtered as tab", "⇧ ⌘ T", "shift-cmd-t", OpenAsTab, WS),`.

`crates/rogg-app/src/menus.rs`: in View, add `MenuItem::action("Open Filtered as Tab", OpenAsTab)`.

`crates/rogg-app/src/tab.rs`:
- `TabEvent` gains `FocusCrumb(usize)` and `OpenAsTab`;
- `TabInit` gains `pub crumb_tabs: Vec<Option<EntityId>>` (empty in `Default`);
- `Tab` gains `pub crumb_tabs: Vec<Option<EntityId>>`, set from `init.crumb_tabs`;
- add:

```rust
    /// The tab behind breadcrumb segment `i`: this tab for the last segment, the parent tabs of a derived tab before it.
    pub fn crumb_tab(&self, i: usize, cx: &App) -> Option<EntityId> {
        let last = self.doc.read(cx).source.crumbs().len().saturating_sub(1);
        if i == last { Some(self.self_id) } else { self.crumb_tabs.get(i).copied().flatten() }
    }

    /// Breadcrumb tabs for a tab derived from this one: one entry per segment of this tab's breadcrumb.
    pub fn crumb_tabs_for_child(&self, cx: &App) -> Vec<Option<EntityId>> {
        let n = self.doc.read(cx).source.crumbs().len();
        (0..n).map(|i| self.crumb_tab(i, cx)).collect()
    }
```

Set `self_id: cx.entity_id()` in `Tab::new` (a private `self_id: EntityId` field).

`crates/rogg-app/src/tab_view.rs`:
- the InfoBar gets `.on_crumb(tab_act_ix(tab, |_, i, _, cx| cx.emit(TabEvent::FocusCrumb(i))))`;
- it gets `.snapshot(match &d.source { Source::Derived(s) => s.snapshot.as_ref().map(|hms| SharedString::from(format!("Snapshot · {hms}"))), _ => None })`;
- the `tool-open-tab` button emits `TabEvent::OpenAsTab` instead of the `later(…)` toast.

`crates/rogg-app/src/workspace/mod.rs`:
- `on_tab_event` adds `TabEvent::FocusCrumb(i) => self.focus_crumb(*i, window, cx)` and `TabEvent::OpenAsTab => self.open_as_tab(window, cx)`;
- the root binds `.on_action(keys::unless_dialog(cx, |ws, _: &OpenAsTab, window, cx| ws.open_as_tab(window, cx)))`;
- add:

```rust
    fn focus_crumb(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let Some(id) = tab.read(cx).crumb_tab(i, cx) else { return };
        if let Some(index) = self.tabs.iter().position(|t| t.entity_id() == id) {
            self.activate(index, window, cx);
        }
    }

    /// ⇧⌘T (spec §4.9, §5.6): the active tab's filtered rows, frozen into a new tab with their original numbers.
    pub fn open_as_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let (source, init) = {
            let t = tab.read(cx);
            let d = t.doc.read(cx);
            let Some(text) = d.text().cloned() else { return };
            if d.filter.progress.is_some() {
                self.toast("Filtering is still running — open as tab when it finishes", cx);
                return;
            }
            let lines = d.filter.view.shown_lines().clone();
            if lines.is_empty() {
                self.toast("No lines to open: the filtered log is empty", cx);
                return;
            }
            let live = d.is_live() || matches!(d.source, Source::Stream { .. });
            let snapshot = live.then(|| rogg_core::clock::local_hms(std::time::SystemTime::now()));
            let spec = crate::session::session_source(&d.source).map(|parent| rogg_core::session::DerivedSpec {
                parent,
                filter: d.filter.spec.clone(),
                pins: d.filter.pins.iter().copied().collect(),
                lines: live.then(|| lines.to_vec()),
                snapshot: snapshot.clone(),
            });
            let mut crumbs = t.labels.crumbs.clone().unwrap_or_else(|| d.source.crumbs());
            crumbs.push("filtered".into());
            let source = crate::derived::DerivedSource {
                title: format!("{} ⟩ filtered", d.title()).into(),
                crumbs,
                tooltip: crate::derived::expression_label(&d.filter.spec),
                snapshot,
                spec,
                origin: crate::derived::DerivedOrigin::Lines { parent: text, numbers: d.numbers().clone(), lines },
            };
            let mut init = TabInit::from_settings(&crate::settings::SettingsStore::get(cx));
            init.profile = Some(t.profile.clone());
            init.crumb_tabs = t.crumb_tabs_for_child(cx);
            (Source::Derived(std::sync::Arc::new(source)), init)
        };
        self.open_source(source, init, window, cx);
    }
```

Task 17 creates `crate::session::session_source`. Until then, add it to a new `crates/rogg-app/src/session.rs` with just this function, plus `pub mod session;` in `lib.rs`:

```rust
//! Session restore (spec §6.4) — this task adds only the source mapping; Task 17 adds capture and restore.

use crate::document::Source;
use rogg_core::session::SessionSource;

/// How a tab's source is stored in a session; None for streams and in-memory texts (spec §6.4: not restored).
pub fn session_source(source: &Source) -> Option<SessionSource> {
    match source {
        Source::File(path) => Some(SessionSource::File { path: path.clone() }),
        Source::Member(source) => Some(SessionSource::Member { source: source.clone() }),
        Source::Derived(d) => d.spec.clone().map(|spec| SessionSource::Derived(Box::new(spec))),
        Source::Stream { .. } | Source::Memory { .. } => None,
    }
}
```

The demo's labels override crumbs with the bundle breadcrumb, and `open_as_tab` uses the label crumbs when they are set. A demo tab's derived child therefore shows `incident-2026-10-05.zip › api.log › filtered`.

`crates/rogg-app/src/sidebar_model.rs`:
- `OpenFile` gains `pub derived: bool`, set in `Workspace::open_files` from `matches!(d.source, Source::Derived(_))`;
- `group_open_files` adds a first arm `_ if f.derived => ("Filtered".into(), "Filtered tabs".into()),`.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. The shortcuts list gains one row, so the app's shortcuts dialog grows (app captures only; the replica is unaffected).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): open filtered as tab with original numbers, nested tabs, breadcrumbs and snapshots

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 17: Session save and restore, window frame, `.rogg` files

**Files:**
- Modify:
  - `crates/rogg-app/src/session.rs` (from Task 16), `crates/rogg-app/src/workspace/{mod,chrome}.rs`
  - `crates/rogg-app/src/window.rs`, `crates/rogg-app/src/lib.rs`
  - `crates/rogg-ui/src/logview/state.rs` (`top_number`)
- Create: `crates/rogg-app/tests/session.rs`

**Interfaces:**
- Consumes:
  - Task 5 `Session`, `TabState`, `WindowState`, `SessionSource`, `ROGG_EXT`.
  - Task 16 `DerivedSource::restored`, `session_source`.
  - Task 10 `TabInit` fields and `SettingsStore`.
- Produces:
  - `LogViewState::top_number(&self) -> Option<u64>`.
  - `rogg_app::session`:
    - `AUTOSAVE: Duration = 30 s`;
    - `tab_state(&Tab, &App) -> Option<TabState>`;
    - `window_state(WindowBounds, sidebar_visible, sidebar_width) -> WindowState`;
    - `bounds_for(&WindowState) -> WindowBounds`.
  - `Workspace`:
    - `capture_session(&self, all: bool, cx) -> Session` (`all = false`: only the active tab);
    - `save_session_now(&mut self, cx)`;
    - `restore_session(&mut self, Session, window, cx) -> usize`;
    - `open_rogg(&mut self, &Path, window, cx)`.

    `Workspace::new` restores `session.json` when `restore_session` is on and the app was not started with `--demo`.
  - `window::window_options` takes `Option<&WindowState>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/session.rs`:

```rust
mod common;
use gpui_kit::{AppContext, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::LoadState;
use rogg_app::settings::SettingsStore;
use rogg_core::filter::FilterSpec;
use rogg_core::session::{DerivedSpec, Session, SessionSource, TabState};
use rogg_core::source::SourceRef;

#[gpui_kit::test]
fn a_session_round_trips_through_save_and_start(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let a = common::write_log(dir.path(), "a.log", &["error one", "ok", "error two", "x"]);
    let b = common::write_log(dir.path(), "b.log", &["b"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![a.clone(), b] });
    let tab_a = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    common::wait_until(cx, |cx| tab_a.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab_a.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.toggle_pin(3, cx));
    tab_a.update(cx, |t, cx| {
        t.toggle_wrap(rogg_app::document::Pane::Filtered, cx);
        t.set_profile("Services".into(), cx);
        t.split = 0.3;
    });
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(0, window, cx))).unwrap();
    tab_a.update(cx, |t, cx| t.select(rogg_app::document::Pane::Full, 3, false, cx));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (saved, _) = Session::load(&dir.path().join("session.json")).unwrap();
    assert_eq!(saved.tabs.len(), 2);
    assert_eq!(saved.active, 0);
    let t = &saved.tabs[0];
    assert_eq!(t.source, SessionSource::File { path: a.clone() });
    assert_eq!((t.filter.expression(), t.pins.clone(), t.selection, t.wrap, t.profile.as_deref()), ("error".into(), vec![3], Some(3), [false, true], Some("Services")));
    assert!((t.split - 0.3).abs() < 1e-6);
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    let restored = ws2.read_with(cx, |ws, _| ws.tabs()[0].clone());
    common::wait_until(cx, |cx| restored.read_with(cx, |t, cx| t.is_ready(cx) && t.doc.read(cx).filter.stats.is_some()));
    restored.read_with(cx, |t, cx| {
        let d = t.doc.read(cx);
        assert_eq!(d.filter.spec.expression(), "error");
        assert!(d.filter.pins.contains(&3));
        assert_eq!(t.profile.as_ref(), "Services");
        assert!(t.filtered.read(cx).wrap() && !t.full.read(cx).wrap());
        assert_eq!(t.selection(), Some(3));
        assert!((t.split - 0.3).abs() < 1e-6);
    });
    assert_eq!(ws2.read_with(cx, |ws, _| (ws.tabs().len(), ws.active_index())), (2, 0));
}

#[gpui_kit::test]
fn restore_keeps_good_tabs_and_fails_missing_ones(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let good = common::write_log(dir.path(), "good.log", &["fine"]);
    let gone = dir.path().join("gone.log");
    let member = SourceRef::member(SourceRef::File(dir.path().join("missing.zip")), "api.log");
    let derived = DerivedSpec { parent: SessionSource::File { path: gone.clone() }, filter: FilterSpec::from_expression("x"), pins: vec![], lines: None, snapshot: None };
    let session = Session {
        tabs: vec![
            TabState::new(SessionSource::File { path: good }),
            TabState::new(SessionSource::File { path: gone }),
            TabState::new(SessionSource::Member { source: member }),
            TabState::new(SessionSource::Derived(Box::new(derived))),
        ],
        ..Session::default()
    };
    session.save(&dir.path().join("session.json")).unwrap();
    let (_w, ws) = common::open_workspace(cx, Args::default());
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert_eq!(tabs.len(), 4);
    common::wait_until(cx, |cx| tabs.iter().all(|t| t.read_with(cx, |t, cx| !matches!(t.doc.read(cx).load, LoadState::Loading { .. }))));
    let states: Vec<bool> = tabs.iter().map(|t| t.read_with(cx, |t, cx| matches!(t.doc.read(cx).load, LoadState::Ready(_)))).collect();
    assert_eq!(states, [true, false, false, false], "missing sources open in the error state; the good tab restores");
}

#[gpui_kit::test]
fn derived_tabs_restore_from_their_definition(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["x1", "y", "x3"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("x", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).filter.stats.is_some()));
    common::press(cx, w, "shift-cmd-t");
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, cx| ws.tabs().len() == 2 && ws.tabs()[1].read(cx).is_ready(cx)));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    let derived = ws2.read_with(cx, |ws, _| ws.tabs()[1].clone());
    common::wait_until(cx, |cx| derived.read_with(cx, |t, cx| t.is_ready(cx)));
    assert_eq!(derived.read_with(cx, |t, cx| t.title(cx)).as_ref(), "a.log ⟩ filtered");
    let numbers: Vec<u64> = derived.read_with(cx, |t, cx| {
        let rows = t.full.read(cx).rows().clone();
        (0..rows.row_count()).filter_map(|i| rows.number_at(i)).collect()
    });
    assert_eq!(numbers, [1, 3]);
}

#[gpui_kit::test]
fn streams_are_not_saved_and_restore_can_be_turned_off(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("feed", || Ok(std::io::Cursor::new(b"a\n".to_vec())), window, cx))).unwrap();
    assert!(ws.read_with(cx, |ws, cx| ws.capture_session(true, cx)).tabs.is_empty());
    Session { tabs: vec![TabState::new(SessionSource::File { path: dir.path().join("x.log") })], ..Session::default() }.save(&dir.path().join("session.json")).unwrap();
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.restore_session = false);
    });
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    assert!(ws2.read_with(cx, |ws, _| ws.tabs().is_empty()));
}

#[gpui_kit::test]
fn a_corrupt_session_toasts_and_rogg_files_open(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    std::fs::write(dir.path().join("session.json"), "{ nope").unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("session.json.bak")), "{:?}", ws.toasts));
    let log = common::write_log(dir.path(), "r.log", &["r"]);
    let rogg = dir.path().join("shared.rogg");
    Session { tabs: vec![TabState::new(SessionSource::File { path: log })], ..Session::default() }.save(&rogg).unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![rogg.clone()], window, cx))).unwrap();
    assert_eq!(ws.read_with(cx, |ws, cx| ws.active_tab().map(|t| t.read(cx).title(cx).to_string())), Some("r.log".into()));
    let broken = dir.path().join("broken.rogg");
    std::fs::write(&broken, "{").unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![broken.clone()], window, cx))).unwrap();
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("Could not open broken.rogg")), "{:?}", ws.toasts));
    assert!(broken.exists());
}

#[test]
fn window_frames_convert_both_ways() {
    use gpui_kit::{Bounds, WindowBounds, point, px, size};
    use rogg_app::session::{bounds_for, window_state};
    let b = Bounds { origin: point(px(10.), px(20.)), size: size(px(1200.), px(800.)) };
    let s = window_state(WindowBounds::Maximized(b), false, px(240.));
    assert!(s.maximized && !s.sidebar_visible && s.sidebar_width == Some(240.));
    assert_eq!(bounds_for(&s), WindowBounds::Maximized(b));
    let tiny = rogg_core::session::WindowState { x: 0., y: 0., width: 100., height: 50., maximized: false, sidebar_visible: true, sidebar_width: None };
    let WindowBounds::Windowed(t) = bounds_for(&tiny) else { panic!() };
    assert_eq!((t.size.width, t.size.height), (px(760.), px(520.)), "never smaller than the window's minimum");
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test session`
Expected: compile errors.

- [ ] **Step 2: Top line of a pane**

`crates/rogg-ui/src/logview/state.rs`:

```rust
    /// Display number of the first line in view (skipping a gap row at the top); None when there are no rows.
    pub fn top_number(&self) -> Option<u64> {
        (0..3).find_map(|k| self.rows.number_at(self.anchor.row + k))
    }
```

- [ ] **Step 3: Capture and restore**

Extend `crates/rogg-app/src/session.rs`:

```rust
use crate::derived::DerivedSource;
use crate::document::{DocumentInit, Source};
use crate::tab::{Tab, TabInit};
use gpui_kit::{App, Bounds, Pixels, WindowBounds, point, px, size};
use rogg_core::session::{TabState, WindowState};
use std::sync::Arc;
use std::time::Duration;

/// `session.json` is written this often while the app runs (spec §6.4), and on quit.
pub const AUTOSAVE: Duration = Duration::from_secs(30);

/// The window's minimum size (`window::window_options`).
const MIN_W: f32 = 760.;
const MIN_H: f32 = 520.;

pub fn window_state(bounds: WindowBounds, sidebar_visible: bool, sidebar_width: Pixels) -> WindowState {
    let (b, maximized) = match bounds {
        WindowBounds::Windowed(b) => (b, false),
        WindowBounds::Maximized(b) | WindowBounds::Fullscreen(b) => (b, true),
    };
    WindowState {
        x: f32::from(b.origin.x),
        y: f32::from(b.origin.y),
        width: f32::from(b.size.width),
        height: f32::from(b.size.height),
        maximized,
        sidebar_visible,
        sidebar_width: Some(f32::from(sidebar_width)),
    }
}

pub fn bounds_for(state: &WindowState) -> WindowBounds {
    let b = Bounds { origin: point(px(state.x), px(state.y)), size: size(px(state.width.max(MIN_W)), px(state.height.max(MIN_H))) };
    if state.maximized { WindowBounds::Maximized(b) } else { WindowBounds::Windowed(b) }
}

/// A tab's restorable state; None for streams and in-memory texts.
pub fn tab_state(tab: &Tab, cx: &App) -> Option<TabState> {
    let d = tab.doc.read(cx);
    let source = session_source(&d.source)?;
    Some(TabState {
        source,
        profile: Some(tab.profile.to_string()),
        filter: d.filter.spec.clone(),
        pins: d.filter.pins.iter().copied().collect(),
        selection: tab.selection(),
        top: [tab.full.read(cx).top_number(), tab.filtered.read(cx).top_number()],
        split: tab.split,
        wrap: [tab.full.read(cx).wrap(), tab.filtered.read(cx).wrap()],
        follow: tab.follow,
    })
}

/// The tab to open for a stored tab, and its init.
pub fn restore_tab(state: TabState) -> (Source, TabInit) {
    let source = match state.source {
        SessionSource::File { path } => Source::File(path),
        SessionSource::Member { source } => Source::Member(source),
        SessionSource::Derived(spec) => Source::Derived(Arc::new(DerivedSource::restored(*spec))),
    };
    let init = TabInit {
        doc: DocumentInit { spec: state.filter, pins: state.pins.into_iter().collect(), ..DocumentInit::default() },
        selection: state.selection,
        wrap: state.wrap,
        follow: state.follow,
        split: Some(state.split),
        top: state.top,
        profile: state.profile.map(Into::into),
        ..TabInit::default()
    };
    (source, init)
}
```

In `crates/rogg-app/src/workspace/mod.rs`:
- add the fields `last_window: Option<WindowState>` (initialised from `window.window_bounds()` in `new`) and `_session_subscriptions: Vec<Subscription>`;
- in `new`, after the toasts and before `ws.open_paths(args.paths, …)`:

```rust
        ws._session_subscriptions = vec![
            cx.observe_window_bounds(window, |ws, window, cx| {
                ws.last_window = Some(crate::session::window_state(window.window_bounds(), ws.sidebar_visible, ws.sidebar_width));
                let _ = cx;
            }),
            cx.on_app_quit(|ws, cx| {
                ws.save_session_now(cx);
                async {}
            }),
        ];
        ws.last_window = Some(crate::session::window_state(window.window_bounds(), ws.sidebar_visible, ws.sidebar_width));
        if !args.demo && crate::settings::SettingsStore::get(cx).behavior.restore_session
            && let Some(path) = cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone())
        {
            match rogg_core::session::Session::load(&path) {
                Ok((session, recovered)) => {
                    if recovered {
                        ws.toast("Session was unreadable and has been reset; the old file was kept as session.json.bak", cx);
                    }
                    ws.restore_session(session, window, cx);
                }
                Err(e) => ws.toast(format!("Could not restore the session: {e}"), cx),
            }
        }
        ws.start_autosave(window, cx);
```

Add to `impl Workspace`:

```rust
    /// The window and its tabs (spec §6.4); with `all == false`, only the active tab (`.rogg` "this tab").
    pub fn capture_session(&self, all: bool, cx: &App) -> rogg_core::session::Session {
        let picked: Vec<&Entity<Tab>> = if all { self.tabs.iter().collect() } else { self.active_tab_ref().into_iter().collect() };
        let mut active = 0;
        let mut tabs = Vec::new();
        for tab in picked {
            if let Some(state) = crate::session::tab_state(tab.read(cx), cx) {
                if all && self.tabs.get(self.active).is_some_and(|t| t.entity_id() == tab.entity_id()) {
                    active = tabs.len();
                }
                tabs.push(state);
            }
        }
        let window = self.last_window.clone().map(|mut w| {
            w.sidebar_visible = self.sidebar_visible;
            w.sidebar_width = Some(f32::from(self.sidebar_width));
            w
        });
        rogg_core::session::Session { version: rogg_core::session::SESSION_VERSION, window, tabs, active }
    }

    fn active_tab_ref(&self) -> Option<&Entity<Tab>> {
        self.tabs.get(self.active)
    }

    /// Writes `session.json` (quit, every 30 s). The demo never overwrites the user's session.
    pub fn save_session_now(&mut self, cx: &mut Context<Self>) {
        if self.fixed_sidebar.is_some() {
            return;
        }
        let Some(path) = cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone()) else { return };
        let _ = self.capture_session(true, cx).save(&path);
    }

    fn start_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(crate::session::AUTOSAVE).await;
                if this.update(cx, |ws, cx| ws.save_session_now(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Opens a stored session's tabs after the ones already open; returns how many were opened. Archive members put
    /// their archive in the sidebar, as reopening from Recent does.
    pub fn restore_session(&mut self, session: rogg_core::session::Session, window: &mut Window, cx: &mut Context<Self>) -> usize {
        if let Some(w) = &session.window {
            self.sidebar_visible = w.sidebar_visible;
            if let Some(width) = w.sidebar_width {
                self.drag_sidebar(px(width), cx);
            }
        }
        let first = self.tabs.len();
        let count = session.tabs.len();
        for state in session.tabs {
            if let rogg_core::session::SessionSource::Member { source } = &state.source {
                self.show_archive_of(source, window, cx);
            }
            let (source, init) = crate::session::restore_tab(state);
            self.open_source(source, init, window, cx);
        }
        if count > 0 {
            self.activate(first + session.active.min(count - 1), window, cx);
        }
        count
    }

    /// A `.rogg` file (spec §6.5): restores its tabs; a broken file is reported and left as it is.
    pub fn open_rogg(&mut self, path: &std::path::Path, window: &mut Window, cx: &mut Context<Self>) {
        match rogg_core::session::Session::read_rogg(path) {
            Ok(session) => {
                let session = rogg_core::session::Session { window: None, ..session };
                self.restore_session(session, window, cx);
            }
            Err(e) => {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.toast(format!("Could not open {name}: {e}"), cx);
            }
        }
    }
```

`open_rogg` passes `window: None` so a `.rogg` file never moves the window or changes the sidebar.

`crates/rogg-app/src/workspace/chrome.rs`:
- in `open_paths`, before `classify`, route `.rogg` files with `if path.extension().is_some_and(|e| e == rogg_core::session::ROGG_EXT) { self.open_rogg(&path, window, cx); continue; }`;
- add:

```rust
    /// Puts the outermost archive of a restored member in the sidebar (listed, not opening its first member).
    pub(crate) fn show_archive_of(&mut self, source: &SourceRef, window: &mut Window, cx: &mut Context<Self>) {
        let root = source.root_path().to_path_buf();
        if let PathKind::Archive(kind, size) = classify(&root) {
            self.open_archive(root, kind, size, false, window, cx);
        }
    }
```

- [ ] **Step 4: The window frame at start**

`crates/rogg-app/src/window.rs`:

```rust
pub fn window_options(state: Option<&rogg_core::session::WindowState>, cx: &App) -> WindowOptions {
    let bounds = state.map(crate::session::bounds_for).unwrap_or_else(|| WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)));
    WindowOptions { window_bounds: Some(bounds), /* the rest as before */ ..}
}

pub fn open_main_window(args: Args, cx: &mut App) -> anyhow::Result<()> {
    // Only the frame is read here (strictly: a damaged file is left for the workspace, which sets it aside with a toast).
    let restore = !args.demo && crate::settings::SettingsStore::get(cx).behavior.restore_session;
    let state = restore
        .then(|| cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone()))
        .flatten()
        .and_then(|path| rogg_core::session::Session::read_rogg(&path).ok())
        .and_then(|s| s.window);
    gpui_kit::open_window(window_options(state.as_ref(), cx), cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx)))?;
    Ok(())
}
```

Update every other `window_options(cx)` caller (the gallery, if it uses it) to `window_options(None, cx)`.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-ui && cargo test -p rogg-gallery && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS.

Check that no existing test is affected by the restore. Tests use a fresh temp dir per `setup`, and a second `open_workspace` in the same test restores what the first one saved; only the new session tests do that. The autosave timer advances only with the test clock.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app crates/rogg-ui
git commit -m "feat(app): session restore with window frame, derived tabs rebuilt, .rogg files

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 18: Export — filtered lines, sessions, filter expression (⌘E)

**Files:**
- Create: `crates/rogg-app/src/export.rs`, `crates/rogg-app/src/dialogs/export.rs`, `crates/rogg-app/tests/export.rs`
- Modify:
  - `crates/rogg-app/src/document/mod.rs` (export job), `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`
  - `crates/rogg-app/src/workspace/mod.rs`
  - `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/menus.rs`, `crates/rogg-app/src/dialogs/mod.rs`, `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes:
  - Task 6 `export::{export_lines, ExportOptions}`.
  - Task 16 `Numbers`.
  - Task 17 `Workspace::capture_session`.
  - Task 9 `Toggle`, `FormRow`, `SectionLabel`.
- Produces:
  - Action `Export` (⌘E, `Export` / `⌘ E`), and the menu item File → `Export…`.
  - `rogg_app::export::ExportJob`.
  - `DocumentEvent::Exported(Result<(u64, String), String>)`, carrying the line count and file name, or an error message.
  - `Document`:
    - `export_lines(&mut self, options, dst: PathBuf, cx) -> Result<(), SharedString>`, which refuses while a scan runs or when nothing is shown;
    - `job_progress` reports `Exporting N%`.
  - `Workspace`:
    - `pub export_options: ExportOptions` (default context and gaps on, numbers off), `pub export_all_tabs: bool`;
    - `open_export(cx)`;
    - `export_lines_to(&mut self, PathBuf, cx)`;
    - `export_session_to(&mut self, PathBuf, cx)`;
    - `copy_expression(cx)`.
  - `Dialog::Export`.
  - Element ids:
    - `export-context`, `export-gaps`, `export-numbers`, `export-lines`
    - `export-scope` (`-0` this tab, `-1` all tabs), `export-session`
    - `export-copy`

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/export.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::workspace::Workspace;
use rogg_core::session::Session;

fn open_filtered(cx: &mut TestAppContext, lines: &[&str], expr: &str) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression(expr, cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some() && d.filter.progress.is_none()));
    (dir, w, ws)
}

fn toasted(ws: &Entity<Workspace>, cx: &mut TestAppContext, needle: &str) -> bool {
    ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains(needle)))
}

#[gpui_kit::test]
fn exports_filtered_lines_in_the_background(cx: &mut TestAppContext) {
    let (dir, _w, ws) = open_filtered(cx, &["error a", "ok", "error b"], "error");
    let dst = dir.path().join("out.log");
    ws.update(cx, |ws, cx| ws.export_lines_to(dst.clone(), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Exported 2 lines to out.log"));
    assert_eq!(std::fs::read_to_string(&dst).unwrap(), "error a\n··· 1 line omitted\nerror b\n");
    ws.update(cx, |ws, cx| {
        ws.export_options.numbers = true;
        ws.export_options.gaps = false;
        ws.export_lines_to(dst.clone(), cx);
    });
    common::wait_until(cx, |cx| std::fs::read_to_string(&dst).is_ok_and(|s| s.starts_with("1  ")));
    assert_eq!(std::fs::read_to_string(&dst).unwrap(), "1  error a\n3  error b\n");
}

#[gpui_kit::test]
fn export_errors_toast_and_closing_cancels(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let (dir, w, ws) = open_filtered(cx, &["error a"], "error");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    ws.update(cx, |ws, cx| ws.export_lines_to(locked.join("out.log"), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Could not export"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(std::fs::read_dir(&locked).unwrap().count(), 0);
    let big: Vec<String> = (0..300_000).map(|i| format!("error line {i}")).collect();
    let big_refs: Vec<&str> = big.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &big_refs);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some() && d.filter.progress.is_none()));
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    ws.update(cx, |ws, cx| ws.export_lines_to(out.join("big-filtered.log"), cx));
    // Handles held by the test would keep the document (and so its export) alive past the close.
    drop(doc);
    drop(tab);
    common::press(cx, w, "cmd-w");
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let leftovers: Vec<_> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".partial")).collect();
    assert!(leftovers.is_empty(), "closing the tab cancels its export and removes the partial file: {leftovers:?}");
}

#[gpui_kit::test]
fn the_export_dialog_sets_options_and_copies_the_expression(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_filtered(cx, &["error a", "timeout"], "error || timeout");
    common::press(cx, w, "cmd-e");
    assert!(common::exists(cx, w, "export-lines"));
    common::click(cx, w, "export-numbers");
    common::click(cx, w, "export-context");
    let options = ws.read_with(cx, |ws, _| ws.export_options);
    assert!(options.numbers && !options.context);
    common::click(cx, w, "export-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("error || timeout"));
    assert!(toasted(&ws, cx, "Filter expression copied"));
    common::click(cx, w, "export-scope-1");
    assert!(ws.read_with(cx, |ws, _| ws.export_all_tabs));
}

#[gpui_kit::test]
fn sessions_export_to_rogg_files(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_filtered(cx, &["error a"], "error");
    let other = common::write_log(dir.path(), "b.log", &["b"]);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![other], window, cx))).unwrap();
    let one = dir.path().join("one.rogg");
    ws.update(cx, |ws, cx| ws.export_session_to(one.clone(), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Saved session to one.rogg"));
    assert_eq!(Session::read_rogg(&one).unwrap().tabs.len(), 1, "this tab only");
    let all = dir.path().join("all.rogg");
    ws.update(cx, |ws, cx| {
        ws.export_all_tabs = true;
        ws.export_session_to(all.clone(), cx);
    });
    common::wait_until(cx, |cx| toasted(&ws, cx, "Saved session to all.rogg"));
    assert_eq!(Session::read_rogg(&all).unwrap().tabs.len(), 2);
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test export`
Expected: compile errors.

- [ ] **Step 2: The job and the document side**

Create `crates/rogg-app/src/export.rs`:

```rust
//! Exporting filtered lines on a background thread (spec §6.5); the document polls it and reports the result.

use crate::numbers::Numbers;
use rogg_core::CancelToken;
use rogg_core::export::{ExportOptions, export_lines};
use rogg_core::text::LogText;
use rogg_core::view::FilteredView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum Message {
    Progress(f32),
    Done(Result<u64, String>),
}

pub struct ExportJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
    pub dst: PathBuf,
    pub progress: f32,
}

impl ExportJob {
    pub fn spawn(text: Arc<LogText>, view: Arc<FilteredView>, numbers: Numbers, options: ExportOptions, dst: PathBuf) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let path = dst.clone();
        let _detached = std::thread::Builder::new().name("rogg-export".into()).spawn(move || {
            let progress = tx.clone();
            let result = export_lines(&text, &view, |l| numbers.display(l), options, &path, &token, |done, total| {
                let _ = progress.send(Message::Progress(if total == 0 { 1. } else { done as f32 / total as f32 }));
            });
            let _ = tx.send(Message::Done(result.map_err(|e| e.to_string())));
        });
        Self { cancel, rx, dst, progress: 0. }
    }

    /// None while running; the outcome once finished (a dead worker is an error).
    pub fn poll(&mut self) -> Option<Result<u64, String>> {
        loop {
            match self.rx.try_recv() {
                Ok(Message::Progress(f)) => self.progress = f,
                Ok(Message::Done(result)) => return Some(result),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => return Some(Err("export stopped unexpectedly".into())),
            }
        }
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
```

Add `pub mod export;` to `lib.rs`.

`crates/rogg-app/src/document/mod.rs`:
- add the field `export: Option<crate::export::ExportJob>`;
- add `DocumentEvent::Exported(Result<(u64, String), String>)`;
- `has_jobs` includes `self.export.is_some()`;
- `poll_jobs` calls `self.poll_export(cx)`;
- `job_progress` checks the export first: `if let Some(job) = &self.export { return Some((format!("Exporting {:.0}%", job.progress * 100.).into(), job.progress)); }`.

```rust
    /// Spec §6.5: writes the filtered log to `dst` in the background; refused while a scan runs or nothing is shown.
    pub fn export_lines(&mut self, options: rogg_core::export::ExportOptions, dst: PathBuf, cx: &mut Context<Self>) -> Result<(), SharedString> {
        let Some(text) = self.text().cloned() else { return Err("The log is not loaded yet".into()) };
        if self.filter.progress.is_some() {
            return Err("Filtering is still running — export when it finishes".into());
        }
        if self.filter.view.shown_lines().is_empty() {
            return Err("Nothing to export: the filtered log is empty".into());
        }
        self.export = Some(crate::export::ExportJob::spawn(text, self.filter.view.clone(), self.numbers.clone(), options, dst));
        cx.notify();
        self.ensure_polling(cx);
        Ok(())
    }

    fn poll_export(&mut self, cx: &mut Context<Self>) {
        let Some(job) = &mut self.export else { return };
        let before = job.progress;
        match job.poll() {
            None => {
                if job.progress != before {
                    cx.notify();
                }
            }
            Some(result) => {
                let name = job.dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.export = None;
                cx.emit(DocumentEvent::Exported(result.map(|n| (n, name))));
                cx.notify();
            }
        }
    }
```

`stop_jobs` does not touch `export`. A reload does not cancel an export of the earlier snapshot, which keeps its own `Arc` of the text. The document's `Drop` drops the job, which cancels it; that is the tab-close path.

`crates/rogg-app/src/tab.rs`, in `on_doc_event`:

```rust
            DocumentEvent::Exported(Ok((n, name))) => {
                cx.emit(TabEvent::Toast(format!("Exported {} {} to {name}", group_digits(*n), noun(*n, "line", "lines")).into()))
            }
            DocumentEvent::Exported(Err(e)) => cx.emit(TabEvent::Toast(format!("Could not export: {e}").into())),
```

`TabEvent` gains `ShowExport`. `tab_view.rs` changes the InfoBar's `.on_export(...)` to emit `TabEvent::ShowExport` and deletes the now-unused `later` helper.

- [ ] **Step 3: The dialog and the workspace**

`crates/rogg-app/src/actions.rs`: add `Export` and the keymap row `shortcut!("Export", "⌘ E", "cmd-e", Export, WS),`.

`crates/rogg-app/src/menus.rs`: File gains `MenuItem::action("Export…", Export)` before the separator.

`crates/rogg-app/src/workspace/mod.rs`:
- fields `pub export_options: rogg_core::export::ExportOptions` (initial `ExportOptions { context: true, gaps: true, numbers: false }`) and `pub export_all_tabs: bool` (`false`);
- `Dialog::Export`;
- `on_tab_event` maps `TabEvent::ShowExport => self.open_export(cx)`;
- the root binds `.on_action(keys::unless_dialog(cx, |ws, _: &Export, _, cx| ws.open_export(cx)))`;
- add the arm `Dialog::Export => crate::dialogs::export::render(this, cx).on_close(close).into_any_element()`.

Add:

```rust
    pub fn open_export(&mut self, cx: &mut Context<Self>) {
        if self.active_tab().is_some() {
            self.open_dialog(Dialog::Export, cx);
        }
    }

    /// Export lines…: the save panel, then the background export.
    pub(crate) fn prompt_export_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let d = tab.read(cx).doc.read(cx);
        let dir = d.source.path().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_else(rogg_core::paths::home_dir);
        let stem = d.title().split(['⟩', ' ']).next().unwrap_or("log").trim_end_matches(".log").to_string();
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{stem}-filtered.log")));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |ws, cx| ws.export_lines_to(path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn export_lines_to(&mut self, dst: PathBuf, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let options = self.export_options;
        let doc = tab.read(cx).doc.clone();
        match doc.update(cx, |d, cx| d.export_lines(options, dst, cx)) {
            Ok(()) => self.close_dialog(cx),
            Err(message) => self.toast(message, cx),
        }
    }

    /// Save session…: this tab or all tabs as a `.rogg` file, written in the background.
    pub(crate) fn prompt_export_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_new_path(&rogg_core::paths::home_dir(), Some("session.rogg"));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |ws, cx| ws.export_session_to(path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn export_session_to(&mut self, dst: PathBuf, cx: &mut Context<Self>) {
        let session = self.capture_session(self.export_all_tabs, cx);
        let name = dst.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let task = cx.background_executor().spawn(async move { session.save(&dst) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |ws, cx| match result {
                Ok(()) => ws.toast(format!("Saved session to {name}"), cx),
                Err(e) => ws.toast(format!("Could not save {name}: {e}"), cx),
            })
            .ok();
        })
        .detach();
        self.close_dialog(cx);
    }

    /// Copy filter expression (spec §6.5).
    pub fn copy_expression(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        let expression = tab.read(cx).doc.read(cx).filter.spec.expression();
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(expression));
        self.toast("Filter expression copied", cx);
    }
```

`crates/rogg-app/src/dialogs/export.rs`:

```rust
//! The Export dialog (spec §6.5): filtered lines to a `.log`, the session to a `.rogg`, the filter expression to the
//! clipboard.

use crate::workspace::{Workspace, act};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, div, px};
use rogg_ui::controls::{ActionButton, Segmented};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{FormRow, SectionLabel, Toggle};
use rogg_ui::theme::ActiveRoggTheme;

pub fn render(this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let ws = this.read(cx);
    let o = ws.export_options;
    let expression = ws.active_tab().map(|t| t.read(cx).doc.read(cx).filter.spec.expression()).unwrap_or_default();
    let t = cx.rogg().clone();
    DialogFrame::new("Export")
        .description("Exports run in the background; the status bar shows their progress.")
        .child(SectionLabel::new("Filtered lines"))
        .child(FormRow::new("row-export-context", "Include context lines").control(Toggle::new("export-context", o.context).on_click(act(this, |ws, _, cx| {
            ws.export_options.context = !ws.export_options.context;
            cx.notify();
        }))))
        .child(FormRow::new("row-export-gaps", "Include gap markers").control(Toggle::new("export-gaps", o.gaps).on_click(act(this, |ws, _, cx| {
            ws.export_options.gaps = !ws.export_options.gaps;
            cx.notify();
        }))))
        .child(FormRow::new("row-export-numbers", "Prefix original line numbers").control(Toggle::new("export-numbers", o.numbers).on_click(act(this, |ws, _, cx| {
            ws.export_options.numbers = !ws.export_options.numbers;
            cx.notify();
        }))))
        .child(div().flex().justify_end().pt(px(8.)).child(ActionButton::new("export-lines", "Export lines…").primary().on_click(act(this, |ws, window, cx| ws.prompt_export_lines(window, cx)))))
        .child(SectionLabel::new("Session"))
        .child(FormRow::new("row-export-scope", "Tabs").control(Segmented::new("export-scope", vec!["This tab".into(), "All tabs".into()], ws.export_all_tabs as usize).on_select({
            let this = this.clone();
            move |i, _, cx| this.update(cx, |ws, cx| {
                ws.export_all_tabs = i == 1;
                cx.notify();
            })
        })))
        .child(div().flex().justify_end().pt(px(8.)).child(ActionButton::new("export-session", "Save session…").on_click(act(this, |ws, window, cx| ws.prompt_export_session(window, cx)))))
        .child(SectionLabel::new("Filter"))
        .child(FormRow::new("row-export-expression", "Filter expression").control(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().max_w(px(220.)).truncate().font_family(t.fonts.mono.clone()).text_size(t.text_scale().px(11.)).text_color(t.palette.muted).child(expression))
                .child(ActionButton::new("export-copy", "Copy").on_click(act(this, |ws, _, cx| ws.copy_expression(cx)))),
        ))
}
```

`act` is `pub(crate)` in `workspace/mod.rs` (M2), so it is reachable as `crate::workspace::act`. Add `pub mod export;` to `dialogs/mod.rs`.

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. Also extend `actions::tests`: assert that `shortcut_rows()` contains `("Export", "⌘ E")`, `("Settings", "⌘ ,")` and `("Open filtered as tab", "⇧ ⌘ T")`.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): export filtered lines and sessions in the background, copy the filter expression, ⌘E

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 19: M3 carry-over fixes

**Files:**
- Modify: `crates/rogg-app/src/document/mod.rs` (idle polling), `crates/rogg-core/src/stream.rs` (done guard), `crates/rogg-core/src/archive.rs` (gzip trailing bytes)
- Test: `crates/rogg-core/tests/archive.rs`, `crates/rogg-core/tests/stream.rs`, `crates/rogg-app/tests/live.rs`

**Interfaces:**
- Consumes: M3 `Follower`, `StreamCapture`, `gunzip`; M2/M3 `EngineConfig`.
- Produces:
  - `rogg_app::document::FOLLOW_POLL: Duration = 100 ms`.
  - `Document::poll_interval(&self) -> Duration`: `POLL_INTERVAL` while any job besides the follower runs, else `FOLLOW_POLL`.
  - `StreamCapture` reports `is_done()` even when its thread panics.
  - `gunzip` stops cleanly at bytes after the last member that do not start another member.

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-core/tests/archive.rs`:

```rust
#[test]
fn trailing_bytes_after_the_last_gzip_member_are_ignored() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("t.log.gz");
    let member = |text: &str| {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(text.as_bytes()).unwrap();
        enc.finish().unwrap()
    };
    let mut bytes = member("one\n");
    bytes.extend(member("two\n"));
    bytes.extend(b"\0\0\0garbage after the stream");
    std::fs::write(&gz, &bytes).unwrap();
    let mut out = Vec::new();
    rogg_core::archive::gunzip(&gz, &mut out, &rogg_core::CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(out, b"one\ntwo\n", "both members, then the trailing bytes are ignored like gzip -d does");
    let mut corrupt = member("x\n");
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0xff;
    std::fs::write(&gz, &corrupt).unwrap();
    assert!(rogg_core::archive::gunzip(&gz, &mut Vec::new(), &rogg_core::CancelToken::new(), |_, _| {}).is_err(), "a damaged member still fails");
}
```

Append to `crates/rogg-core/tests/stream.rs`:

```rust
#[test]
fn a_panicking_capture_still_reports_done() {
    struct Boom;
    impl std::io::Read for Boom {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("reader exploded")
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let cache = rogg_core::cache::Cache::new(dir.path(), 1 << 30);
    let capture = rogg_core::stream::StreamCapture::start(|| Ok(Boom), &cache).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !capture.is_done() {
        assert!(std::time::Instant::now() < deadline, "done was never set");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
```

Append to `crates/rogg-app/tests/live.rs`:

```rust
#[gpui_kit::test]
fn an_idle_followed_file_polls_slowly(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "idle.log", &["a", "b"]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some() && d.filter.progress.is_none()));
    assert_eq!(doc.read_with(cx, |d, _| d.poll_interval()), rogg_app::document::FOLLOW_POLL);
    doc.update(cx, |d, cx| d.set_search(rogg_app::document::Pane::Full, "a", cx));
    let busy = doc.read_with(cx, |d, _| d.poll_interval());
    assert!(busy == rogg_app::document::POLL_INTERVAL || busy == rogg_app::document::FOLLOW_POLL, "a small search may finish synchronously");
}

#[gpui_kit::test]
fn truncating_a_large_file_mid_scan_reloads_and_renders(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(rogg_app::document::EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..400_000).map(|i| format!("line {i} error padding padding padding")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &refs);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log.clone()] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    std::fs::OpenOptions::new().write(true).open(&log).unwrap().set_len(0).unwrap();
    std::fs::write(&log, "fresh\n").unwrap();
    for _ in 0..20 {
        common::render(cx, w);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_count() == 1)));
    common::render(cx, w);
    assert!(ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains("truncated or replaced"))));
}
```

Both live tests need `use rogg_app::args::Args;` and `gpui_kit::{AppContext, TestAppContext}` in the test file; add them if missing.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test archive --test stream && cargo test -p rogg-app --test live`
Expected: the gzip test fails (MultiGzDecoder errors on the trailing bytes); the stream test times out (done never set); `poll_interval` does not compile. The truncation test may already pass; it pins the behaviour at the app level (carry-over item).

- [ ] **Step 2: Implement**

`crates/rogg-core/src/archive.rs`, `gunzip`. Replace the `MultiGzDecoder` loop with an explicit member loop:

```rust
pub fn gunzip(src: &Path, out: &mut impl Write, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<u64, CoreError> {
    use std::io::BufRead;
    let file = File::open(src).map_err(CoreError::io(src))?;
    let total = file.metadata().map_err(CoreError::io(src))?.len();
    let read = Rc::new(Cell::new(0));
    let mut input = io::BufReader::with_capacity(COPY_CHUNK, Counting { inner: file, read: read.clone() });
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut written = 0u64;
    let mut members = 0u32;
    loop {
        // At a member boundary: the end of the input, or bytes that do not start another member (padding, garbage —
        // `gzip -d` warns and ignores them), end the stream once at least one member was read.
        let head = input.fill_buf().map_err(|e| archive_error(src, e))?;
        if head.is_empty() || (members > 0 && !head.starts_with(&[0x1f, 0x8b])) {
            break;
        }
        let mut decoder = flate2::bufread::GzDecoder::new(&mut input);
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let n = match decoder.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(archive_error(src, e)),
            };
            out.write_all(&buf[..n]).map_err(CoreError::io(src))?;
            written += n as u64;
            progress(read.get().min(total), total);
        }
        members += 1;
    }
    progress(total, total);
    Ok(written)
}
```

Keep the `MultiGzDecoder` import: sniffing (`archive.rs` around line 70) and `.tar.gz` listing (around line 132) still use it. A `.tar.gz` stream ends at the tar end marker, so trailing bytes never reach its decoder.

`Inflating`, M3's progressive `.gz` path in `source.rs`, decompresses through `archive::gunzip`, so it gets the fix too. Confirm with `grep -n "gunzip" crates/rogg-core/src/source.rs`.

`crates/rogg-core/src/stream.rs`, inside the spawned thread closure. Create the guard first:

```rust
            /// Sets `done` however the thread ends — a panic in the reader included (M3 carry-over).
            struct DoneGuard(Arc<AtomicBool>);
            impl Drop for DoneGuard {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::Release);
                }
            }
            let _done = DoneGuard(done);
```

and delete the final `done.store(true, Ordering::Release);`. The guard owns the `done` clone the closure captured.

`crates/rogg-app/src/document/mod.rs`:

```rust
/// How often an otherwise idle document checks its follower's channel (live follow): growth shows within 0.1 s, and
/// many open live tabs no longer wake the app every frame (M3 carry-over).
pub const FOLLOW_POLL: Duration = Duration::from_millis(100);

impl Document {
    /// The poll loop's period: every frame while a job besides the follower runs, else `FOLLOW_POLL`.
    pub fn poll_interval(&self) -> Duration {
        let busy = self.open_job.is_some()
            || self.inflating.is_some()
            || self.filter.job.is_some()
            || self.filter.view_job.is_some()
            || self.search_running()
            || self.derived_job.is_some()
            || self.export.is_some();
        if busy { POLL_INTERVAL } else { FOLLOW_POLL }
    }
}
```

In `ensure_polling`, read the interval each turn:

```rust
            loop {
                let interval = match this.update(cx, |doc, _| doc.poll_interval()) {
                    Ok(interval) => interval,
                    Err(_) => break,
                };
                cx.background_executor().timer(interval).await;
                match this.update(cx, |doc, cx| doc.poll_jobs(cx)) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
```

A job started while the loop sleeps 100 ms is first polled at most 100 ms late. Every job starter calls `ensure_polling`, which is a no-op while the loop runs. That latency is acceptable for a user action; the scan itself runs at full speed on its thread.

- [ ] **Step 3: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. If a follower test in `document/follower_tests.rs` assumed a 16 ms cadence for an idle follower, extend its wait to cover `FOLLOW_POLL`; the tests poll with deadlines.

- [ ] **Step 4: Commit**

```bash
git add crates/rogg-core crates/rogg-app
git commit -m "fix: idle live tabs poll at 100 ms, panicking captures report done, trailing gzip bytes are ignored

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 20: Visual check, app-page dialogs, manual checks and notes

**Files:**
- Modify:
  - `crates/rogg-gallery/src/capture.rs`, `crates/rogg-gallery/src/gallery.rs` (app-page profile and appearance dialogs)
  - `tools/visual-diff/config.mjs`
  - `docs/superpowers/reviews/2026-10-07-m2-visual.md` ("After M4" section)
- Create: `docs/superpowers/notes/2026-10-07-m4-manual-checks.md`

**Interfaces:**
- Consumes: Task 12 `Workspace::open_settings`, Task 13 `Workspace::open_profile_editor`.
- Produces:
  - `rogg-gallery --capture app --dialog profile|appearance` opens the real editor and settings dialog in the demo app.
  - The visual report covers them, as guidance for the joint review.

- [ ] **Step 1: App-page dialogs**

`crates/rogg-gallery/src/capture.rs`: remove the rejection of `Profile | Appearance` on the app page and its test assertion, if any.

`crates/rogg-gallery/src/gallery.rs`, `for_capture`: replace the `(Page::App, Some(d), Some(app))` arm with:

```rust
            (Page::App, Some(d), Some(app)) => app.update(cx, |ws, cx| match d {
                ReplicaDialog::Profile => ws.open_profile_editor(window, cx),
                ReplicaDialog::Appearance => ws.open_settings(cx),
                ReplicaDialog::AllTabs => ws.open_dialog(Dialog::AllTabs, cx),
                ReplicaDialog::Shortcuts => ws.open_dialog(Dialog::Shortcuts, cx),
                ReplicaDialog::History => ws.open_dialog(Dialog::History, cx),
            }),
```

`tools/visual-diff/config.mjs`: add `'app'` to the `pages` of the `profile-dialog` and `settings-dialog` entries, and update the comment ("profile and settings are M4 in the app" → "real dialogs on the app page since M4").

- [ ] **Step 2: Golden check and app report**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; just visual-check`
Expected: 0.000% differing pixels on all 9 replica captures. If not, find the change that moved a replica pixel and fix it before going on; the replica is not re-approved in M4.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; just visual-app`
Expected: the report is written. Read `target/visual/report.html` and the JSON tables.

Append to `docs/superpowers/reviews/2026-10-07-m2-visual.md`:

```markdown
## After M4 (YYYY-MM-DD)
Replica goldens: `just visual-check` 0.000% on all 9 captures (unchanged).

App captures (`just visual-app`), differing pixels: <fill in each capture's number from the report>.
Changes since the M2 gate, for the joint review:
- The demo tabs use the built-in Services profile: ERROR lines are painted in the error colour, WARN lines in amber,
  and `request_id=…` in the third hit colour (the reference mockup leaves these rows uncoloured). accept?
- New app-page captures `--profile` and `--appearance` show the real profile editor and Settings dialog against the
  reference's placement previews; both are taller and have more controls. accept?
- The shortcuts dialog lists three more rows (Settings, Export, Open filtered as tab); its height was already capped. accept?
```

Replace `YYYY-MM-DD` with the run date and fill the numbers in from the report. Write each measured value; leave none as a template.

- [ ] **Step 3: Manual checks**

Create `docs/superpowers/notes/2026-10-07-m4-manual-checks.md`:

```markdown
# M4 manual checks

Release build: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo build --release -p rogg-app`, then run
`target/release/rogg`. These need a real machine (native panels, quit, the window manager, the system appearance).

## Settings
1. ⌘, opens Settings; the title bar's settings button too.
2. Theme System, then switch macOS between light and dark (System Settings → Appearance): rogg follows at once.
3. Log font: SF Mono, Menlo, Monaco, Courier New (only the installed ones are listed); "Other…" lists installed
   monospace fonts only (no proportional fonts); picking one applies it and returns to Settings.
4. Log font size 9 and 20 in both densities: rows stay readable, pins stay centred, selection and search outlines fit.
5. UI font size 11 and 16: bars keep their heights; no text is clipped in the title bar, info bar, filter band, footer.
6. Cache size limit 1 GB with a large bundle open: closing tabs frees the cache (Finder: ~/Library/Caches/rogg).
7. Folder scan depth 1, then open a folder with nested logs: only top-level files are listed.
8. Quit and relaunch: every setting is kept.

## Profiles
9. Profile: … in the info bar opens the editor for that tab's profile; choosing another profile restyles the tab.
10. New, Duplicate, Rename (Enter applies), Delete (not on Default); tabs using a deleted profile switch to Default.
11. Edit a rule's pattern, flags, effects, panels and colours; rows change while typing; an invalid regex shows its error.
12. Light theme: a hex colour's derived light variant is readable; an override replaces it.
13. Palette: add and edit a slot; filter chips and filtered-log marks use the new colours.
14. Import… / Export… through the native panels; a damaged `.roggprofile` is reported and left untouched.

## Export and derived tabs
15. ⌘E: export filtered lines with each option; the footer shows `Exporting N%` on a 1 GB file; the UI stays responsive;
    closing the tab mid-export leaves no file.
16. Save session… for this tab and for all tabs; open the `.rogg` with ⌘O: the tabs come back with filters and pins.
17. Copy: the clipboard holds the expression.
18. ⇧⌘T on a 1 GB file's filtered view: `Building tab N%`, then the tab; line numbers match the parent; Go to line,
    pins and find use the original numbers; a nested tab works the same; the breadcrumb jumps back to the parents.
19. A live file: ⇧⌘T shows `Snapshot · HH:MM:SS`.

## Session
20. Open files, an archive member, a derived tab; set filters, pins, selection, wrap, split, sidebar width; quit with ⌘Q
    and relaunch: everything is back, including the window frame (also when it was maximised).
21. Move a restored file away and relaunch: its tab shows the error state with Retry; the others are fine.
22. Settings → Restore session off: relaunch opens empty.
```

- [ ] **Step 4: Full suite and commit**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && just visual-check`
Expected: PASS, and 0.000%.

```bash
git add crates/rogg-gallery tools/visual-diff docs/superpowers
git commit -m "docs: M4 visual notes and manual checks; real profile and settings dialogs on the app page

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Deferred from M4 (post-v1)

These limits are known and accepted. None blocks v1.

**Engine and performance (M3 carry-over):**
- Each live growth copies all matches (`TailBase::new` / `merge`) and the full-log search result on the UI thread.
- Each live growth clones the whole line index on the follower thread.
- Superseded view builds are not cancelled.
- A plain file is indexed before it shows.

**Sources (M3 carry-over):**
- Listing a plain `.tar` reads every byte.
- A truncated `.gz` switches the tab to the error state instead of keeping the decompressed part.
- The hung-network-mount `stat` and `classify()` still run on the UI thread.
- A UTF-16 file is not live-followed.
- An in-place rewrite that keeps the first 64 bytes reads as an append.

**Sidebar:**
- Rows are not virtualised (R12). Virtualising touches the replica's shared `Sidebar` and needs the user's re-approval of the goldens.
- `insert_folder` / `insert_listing` are not idempotent.
- `same_file` canonicalises on the UI thread.

**Derived tabs:**
- The copied lines use disk space in `streams/`. The copy is deleted with its tab and swept after a crash.
- A derived tab's parent can only be focused through the breadcrumb while that parent tab is open.
- A derived file whose first copied line starts with bytes that look like a UTF-16 byte order mark is read as UTF-16.

**Profiles:**
- The filter chips of a custom palette slot use a derived chip style. Only the four token slots reproduce the reference chips exactly.
- "Other…" measures fonts on the UI thread the first time it opens.

**Remaining M3 test gaps:**
- the SIGBUS handler chaining;
- unsupported zip methods;
- duplicate tar paths;
- exact-limit folder truncation;
- non-UTF-8 names;
- 3- and 4-byte UTF-8 straddles;
- the drag-handle grab offsets.

**Spec §9 deferred list** (bundle-wide search, timestamps, merged timeline, other platforms, more encodings): unchanged.
