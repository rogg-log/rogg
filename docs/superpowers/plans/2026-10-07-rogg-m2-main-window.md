# rogg M2 — Main Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `rogg` application: a real macOS window that wires the M1 engine (`rogg-core`) into the M0 components (`rogg-ui`) — tabs, sidebar of open files, both log panes, filter band, find, follow selection, wrap, keyboard, menu bar, empty states, progress, recent files and `rogg --demo` — and extend the visual harness so the real app can be compared with the reference for the M2 gate.

**Architecture:** A new crate `rogg-app` (library `rogg_app` + binary `rogg`) is the only crate that knows both sides (spec §3.2). A `Document` entity per tab owns the `LogText`, the filter pipeline and per-pane search; it runs large jobs on background threads (M1 `FilterJob`, an open thread, a search thread) and polls their channels from one gpui task at ~60 Hz, while inputs under 4 MiB run synchronously so small files never flash a loading state. `FullRows`/`FilteredRows` implement `rogg_ui::logview::LogRowSource` over the engine (only visible rows are materialised). A `Tab` entity holds the two `LogViewState`s, find inputs and selection/follow state; the `Workspace` view owns tabs, sidebar, dialogs, toasts, menus and the keymap. `rogg-ui` gains the log-view interactions deferred from M0 (keyboard selection, range selection, scrollbar dragging, context menu, focus), a placeholder layout, drag handles and toasts.

**Tech Stack:** Rust 1.99.0 (edition 2024), gpui-pre 0.3.8 through gpui-kit =0.7.1, rogg-core (M1), rogg-ui (M0), Node + Playwright visual harness (M0).

**Spec:** `docs/superpowers/specs/2026-10-06-rogg-design.md` (§3.2–3.3, §4.6–4.12, §5, §6.3, §6.6, §7, §8, §9 M2). Carry-over notes folded in: `docs/superpowers/notes/2026-10-07-m0-carryover-to-m2.md`, `docs/superpowers/notes/2026-10-07-m1-carryover-to-m2.md`.

## Global Constraints

- macOS only (Apple Silicon); no cross-platform abstractions (spec §2).
- GUI: `gpui-pre =0.3.8` through `gpui-kit =0.7.1` only — import gpui types from `gpui_kit`, never add a `gpui` dependency. Any crate that uses `gpui_kit::actions!` or `#[derive(gpui_kit::Action)]` declares `extern crate gpui_kit as gpui;` at its crate root (the derive emits `gpui::` paths; gpui-kit documents this alias).
- Toolchain: Rust 1.99.0 pinned by `rust-toolchain.toml`. Prefix every cargo/just command with `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH";`.
- Boundaries (spec §3.2): `rogg-core` has no gpui dependency; `rogg-ui` renders plain data and reports intents through callbacks/events and never touches files or the engine; `rogg-app` is the only crate that depends on both.
- Concurrency (spec §3.3): indexing, filtering and searching of inputs larger than `EngineConfig::sync_bytes` (default 4 MiB) run off the UI thread; the UI polls job channels at most once per frame (`POLL_INTERVAL` = 16 ms); a superseded or closed job is cancelled and its results never reach the UI.
- M1 engine contracts (carry-over): a `FilterJob` channel that closes without `Finished`/`Cancelled` means the worker died — show an error, not a cancel; never drop a `FilterJob` on a rayon thread and never block the UI thread on it (retire old jobs on a short-lived std thread); slice raw lines to 64 KB before `String::from_utf8_lossy`/`Highlighter::spans`; `FilteredView::build`'s `excluded` closure is `|n| filter.is_excluded(text.line(n))`; decide whether to rescan by comparing terms, regex flag and case flag only (a context change only rebuilds the view); `FilterStats` takes `matched` from the scan and the rest from the view of the same scan.
- Visuals come only from `rogg_ui::theme` tokens; no gpui-kit default colors. Every new window root (workspace, welcome, placeholder) sets `line_height(relative(theme::size::LINE_HEIGHT))` — `MainWindowLayout` does this for you.
- Build and run the app with `-p rogg-app` (`cargo run -p rogg-app`): the gallery enables gpui-kit `test-support` for the whole workspace build.
- Dependencies stay on the workspace pins in the root `Cargo.toml`; this plan adds no third-party crate (rogg-app uses `anyhow`, dev-dep `tempfile`).
- Display numbers: a file's first line is number 1 (`number_base` = 1); `rogg --demo` uses base 120480 like the reference. Engine line indices are 0-based; convert only at the `Tab`/rows boundary.
- Copy and labels match the reference: `N matches / N lines · N pinned (N added) · N context` (singular `match`/`line` for 1), `N shown`, `N hits`, `Line N`, `Previous results retained · <message>`, `No matching lines`.
- Commit messages are conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

The five failure modes most likely to bite a user that the feature tests below would not otherwise exercise. Each line names the owning task, which carries a test for it.

1. **Filter changed while a large scan is still running** — only the newest scan's matches ever appear; no mix of old and new chunks, no stale progress. (Task 8: `superseded_async_scan_never_leaks_results`.)
2. **Tab closed or document dropped mid-scan on a large file** — closing returns immediately (no UI stall waiting for the worker) and nothing panics later. (Task 8: `dropping_a_document_mid_scan_does_not_block`.)
3. **Invalid regex typed into the expression editor** — the previous results and chips stay, the error line reads `Previous results retained · …`, the editor stays open, Escape restores the chips and clears the error. (Task 11: `invalid_regex_keeps_results_and_editor`; Task 13: `escape_cancels_editing_and_clears_the_error`.)
4. **Lines longer than 64 KB** — rows render truncated with `…`, filter hits and search spans stay inside the display text, and Copy returns the full original line. (Task 9: `long_lines_are_truncated_for_display_but_copied_in_full`.)
5. **Empty, missing or unreadable files** — an empty file opens to an empty full log and the no-matches state; a missing file shows the error state with its path and a working Retry. (Task 7: `empty_and_missing_files`; Task 11: `missing_file_shows_error_state_with_retry`.)

---

## File Structure

```
crates/rogg-core/src/
  persist.rs            NEW  load-with-recovery / atomic save for small JSON files (shared by history, recent)
  recent.rs             NEW  RecentList model (spec §6.3)
  history.rs            MOD  use persist
  stats.rs              MOD  noun(), FilterStats::from_parts, singular copy
  paths.rs              MOD  recent_path()
crates/rogg-ui/src/
  logview/state.rs      MOD  scroll strategies, keyboard selection, range selection, copy text, drag + context state
  logview/scroll.rs     MOD  ensure_visible, row_top, at_position
  logview/model.rs      MOD  LogRowSource::{number_at, nearest_row, copy_text}
  logview/element.rs    MOD  range paint, drag-select, scrollbar dragging, horizontal bar, right-click target
  logview/text.rs       MOD  token_at
  logview/actions.rs    NEW  keyboard actions + KEY_CONTEXT
  logview/mod.rs        MOD  LogView focus, actions, context menu, focus ring
  layout.rs             MOD  single-body layout, LayoutMeasure, BandDrag/SidebarDrag, sidebar resize edge
  filter.rs             MOD  draggable band handle, focus ring
  titlebar.rs           MOD  tab tooltips, tab scroll handle, double-click zoom, draggable brand
  dialogs.rs            MOD  history_picker
  toast.rs              NEW  ToastStack
crates/rogg-app/        NEW crate (lib rogg_app + bin rogg)
  src/lib.rs            run(), init_with(), install_globals()
  src/main.rs           binary entry
  src/args.rs           CLI: rogg [--demo] [FILE...]
  src/paths.rs          AppPaths global (history, recent, cache)
  src/actions.rs        actions + the single keymap/shortcut table
  src/menus.rs          native menu bar
  src/window.rs         window options (transparent titlebar, traffic lights) + open
  src/format.rs         format_size
  src/document/mod.rs   Document entity: source, loading, polling, EngineConfig
  src/document/load.rs  background open job
  src/document/filter.rs FilterState + filter pipeline
  src/document/search.rs Pane, per-pane search
  src/history.rs        HistoryStore global
  src/recent.rs         RecentStore global
  src/decorate.rs       built-in row decoration (timestamp faint, ERROR tint) until profiles (M4)
  src/rows.rs           FullRows / FilteredRows (LogRowSource over the engine)
  src/tab.rs            Tab entity: panes, selection sync, follow, pins, find, edit
  src/tab_view.rs       Tab → info bar, headers, panes, band, status, placeholders
  src/sidebar_model.rs  open-files tree for the sidebar
  src/workspace.rs      Workspace view: tabs, sidebar, dialogs, toasts, actions, drag handles
  src/demo.rs           reference sample data + `rogg --demo`
  tests/common/mod.rs, tests/app.rs, tests/document.rs, tests/filter.rs, tests/search.rs, tests/tab.rs,
  tests/workspace.rs, tests/keyboard.rs, tests/demo.rs
crates/rogg-gallery/    MOD  uses rogg_app::demo data; `--capture app` page hosting the real workspace
tools/visual-diff/      MOD  ROGG_VISUAL_PAGE=app compares the real app with the reference
docs/superpowers/reviews/2026-10-07-m2-visual.md   NEW  gate report summary
```

---

### Task 1: Core persistence helper, recent files model and footer copy

**Files:**
- Create: `crates/rogg-core/src/persist.rs`, `crates/rogg-core/src/recent.rs`
- Modify: `crates/rogg-core/src/history.rs`, `crates/rogg-core/src/stats.rs`, `crates/rogg-core/src/paths.rs`, `crates/rogg-core/src/lib.rs`
- Test: unit tests in `persist.rs`, `recent.rs`, `stats.rs`, `paths.rs`; existing `history.rs` tests keep passing

**Interfaces:**
- Consumes: `CoreError::{Io, io}` (M1), `History`/`HistoryEntry` (M1), `FilterStats`, `group_digits`, `FilteredView` (M1).
- Produces:
  - `persist::{with_suffix(&Path, &str) -> PathBuf, load_json<T: DeserializeOwned + Default>(&Path) -> Result<(T, bool), CoreError>, save_json<T: Serialize>(&Path, &T) -> Result<(), CoreError>}`
  - `recent::{RECENT_LIMIT = 20, RecentEntry { path: PathBuf, is_folder: bool }, RecentList}`; `RecentEntry::{file, folder, label() -> String, is_missing() -> bool}`; `RecentList::{entries() -> &[RecentEntry], record(RecentEntry), remove(&Path), load(&Path) -> Result<(RecentList, bool), CoreError>, save(&Path) -> Result<(), CoreError>}`
  - `paths::recent_path() -> PathBuf` (`…/Application Support/rogg/recent.json`)
  - `stats::noun<'a>(n: u64, one: &'a str, many: &'a str) -> &'a str`
  - `FilterStats::from_parts(spec: &FilterSpec, matched: u64, term_hits: &[u64], total_lines: u64, duration: Duration, view: &FilteredView) -> FilterStats`
  - `FilterStats::summary()` now says `1 match / 1 line` in the singular.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/persist.rs` with only its tests for now:

```rust
//! Small JSON state files: load with corruption recovery and save atomically (spec §6.6).

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct State {
        #[serde(default)]
        items: Vec<String>,
    }

    #[test]
    fn missing_file_loads_default_without_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let (state, recovered) = load_json::<State>(&dir.path().join("none.json")).unwrap();
        assert_eq!(state, State::default());
        assert!(!recovered);
    }

    #[test]
    fn save_then_load_round_trips_and_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/state.json");
        let state = State { items: vec!["a".into(), "b".into()] };
        save_json(&path, &state).unwrap();
        assert_eq!(load_json::<State>(&path).unwrap(), (state, false));
        assert!(!with_suffix(&path, ".partial").exists());
    }

    #[test]
    fn unparseable_file_is_moved_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, [0x7b, 0xff, 0xfe]).unwrap();
        let (state, recovered) = load_json::<State>(&path).unwrap();
        assert_eq!(state, State::default());
        assert!(recovered);
        assert!(!path.exists());
        assert_eq!(std::fs::read(with_suffix(&path, ".bak")).unwrap(), [0x7b, 0xff, 0xfe]);
    }
}
```

Create `crates/rogg-core/src/recent.rs` with only its tests:

```rust
//! Recently opened files and folders (spec §6.3), persisted as `recent.json`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_dedups_by_path_moves_to_front_and_caps() {
        let mut list = RecentList::default();
        list.record(RecentEntry::file("/a.log"));
        list.record(RecentEntry::folder("/logs"));
        list.record(RecentEntry::file("/a.log"));
        let paths: Vec<_> = list.entries().iter().map(|e| e.path.to_str().unwrap().to_string()).collect();
        assert_eq!(paths, ["/a.log", "/logs"]);
        for i in 0..30 {
            list.record(RecentEntry::file(format!("/f{i}.log")));
        }
        assert_eq!(list.entries().len(), RECENT_LIMIT);
        assert_eq!(list.entries()[0].path.to_str(), Some("/f29.log"));
    }

    #[test]
    fn remove_drops_the_entry() {
        let mut list = RecentList::default();
        list.record(RecentEntry::file("/a.log"));
        list.record(RecentEntry::file("/b.log"));
        list.remove(std::path::Path::new("/a.log"));
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.entries()[0].path.to_str(), Some("/b.log"));
    }

    #[test]
    fn save_load_round_trip_and_corruption_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let mut list = RecentList::default();
        list.record(RecentEntry::folder("/logs"));
        list.save(&path).unwrap();
        let (loaded, recovered) = RecentList::load(&path).unwrap();
        assert!(!recovered);
        assert_eq!(loaded.entries(), list.entries());
        std::fs::write(&path, "not json").unwrap();
        let (loaded, recovered) = RecentList::load(&path).unwrap();
        assert!(recovered);
        assert!(loaded.entries().is_empty());
    }

    #[test]
    fn label_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("here.log");
        std::fs::write(&present, "x").unwrap();
        assert_eq!(RecentEntry::file(&present).label(), "here.log");
        assert!(!RecentEntry::file(&present).is_missing());
        assert!(RecentEntry::file(dir.path().join("gone.log")).is_missing());
    }
}
```

Append to the `tests` module in `crates/rogg-core/src/stats.rs`:

```rust
    #[test]
    fn summary_uses_singular_for_one() {
        let view = FilteredView::build(Arc::new(vec![0]), &BTreeSet::new(), 0, 1, |_| false);
        let spec = FilterSpec { terms: vec![Term::include("a")], ..Default::default() };
        let stats = FilterStats::from_parts(&spec, 1, &[1], 1, std::time::Duration::ZERO, &view);
        assert_eq!(stats.summary(), "1 match / 1 line · 0 pinned · 0 context");
        assert_eq!(noun(1, "match", "matches"), "match");
        assert_eq!(noun(0, "match", "matches"), "matches");
        assert_eq!(noun(2, "line", "lines"), "lines");
    }

    #[test]
    fn from_parts_matches_compute() {
        let spec = FilterSpec { terms: vec![Term::include("a"), Term::new("b", TermState::Exclude)], ..Default::default() };
        let result = FilterResult { matches: vec![1, 3], term_hits: vec![2, 1], total_lines: 10, duration: std::time::Duration::from_millis(5) };
        let view = FilteredView::build(Arc::new(result.matches.clone()), &BTreeSet::from([7]), 1, 10, |_| false);
        let from_parts = FilterStats::from_parts(&spec, 2, &result.term_hits, 10, result.duration, &view);
        assert_eq!(from_parts, FilterStats::compute(&spec, &result, &view));
    }
```

Append to the `tests` module in `crates/rogg-core/src/paths.rs`:

```rust
    #[test]
    fn recent_path_is_in_app_support() {
        assert!(recent_path().ends_with("Library/Application Support/rogg/recent.json"));
    }
```

Add the modules to `crates/rogg-core/src/lib.rs` (keep the existing lines; insert in alphabetical order):

```rust
pub mod persist;
pub mod recent;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --lib persist recent stats paths`
Expected: compile errors — `load_json`, `save_json`, `with_suffix`, `RecentList`, `RecentEntry`, `RECENT_LIMIT`, `noun`, `FilterStats::from_parts`, `recent_path` not found.

- [ ] **Step 3: Implement**

Put this above the tests in `crates/rogg-core/src/persist.rs`:

```rust
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

/// Reads `path` as JSON. A missing file yields `T::default()`. An unparseable file (invalid JSON or invalid
/// UTF-8) is renamed to `<file>.bak` and yields `T::default()` with `recovered = true`; other I/O errors are
/// returned.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((T::default(), false)),
        Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
    };
    match serde_json::from_slice::<T>(&bytes) {
        Ok(value) => Ok((value, false)),
        Err(_) => {
            std::fs::rename(path, with_suffix(path, ".bak")).map_err(CoreError::io(path))?;
            Ok((T::default(), true))
        }
    }
}

/// Writes `value` as pretty JSON to `<file>.partial`, then renames it over `path`; creates the parent directory.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
    }
    let json = serde_json::to_string_pretty(value).expect("plain state structs serialize");
    let partial = with_suffix(path, ".partial");
    std::fs::write(&partial, json).map_err(CoreError::io(&partial))?;
    std::fs::rename(&partial, path).map_err(CoreError::io(path))
}
```

Put this above the tests in `crates/rogg-core/src/recent.rs`:

```rust
use crate::error::CoreError;
use crate::persist::{load_json, save_json};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const RECENT_LIMIT: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub is_folder: bool,
}

impl RecentEntry {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: false }
    }

    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: true }
    }

    /// The last path component, for list labels.
    pub fn label(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string())
    }

    /// True when the path no longer exists; such entries render faint (spec §6.3).
    pub fn is_missing(&self) -> bool {
        !self.path.exists()
    }
}

/// Newest first, at most `RECENT_LIMIT` entries, unique by path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentList {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<RecentEntry>,
}

impl RecentList {
    pub fn entries(&self) -> &[RecentEntry] {
        &self.entries
    }

    /// Moves `entry` to the front (an entry with the same path is replaced) and keeps the newest 20.
    pub fn record(&mut self, entry: RecentEntry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.insert(0, entry);
        self.entries.truncate(RECENT_LIMIT);
    }

    pub fn remove(&mut self, path: &Path) {
        self.entries.retain(|e| e.path != path);
    }

    /// Returns the list and whether a corrupt file was moved aside to `<file>.bak`.
    pub fn load(path: &Path) -> Result<(RecentList, bool), CoreError> {
        let (mut list, recovered) = load_json::<RecentList>(path)?;
        list.entries.truncate(RECENT_LIMIT);
        Ok((list, recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &RecentList { version: 1, entries: self.entries.clone() })
    }
}
```

In `crates/rogg-core/src/history.rs`:
- add `use crate::persist::{load_json, save_json};`
- change `#[derive(Serialize, Deserialize)] struct HistoryFile` to `#[derive(Default, Serialize, Deserialize)] struct HistoryFile`
- delete the private `fn with_suffix` (if a history test uses it, import `crate::persist::with_suffix` in the test module instead)
- replace the bodies of `load` and `save` with:

```rust
    pub fn load(path: &Path) -> Result<LoadOutcome, CoreError> {
        let (file, recovered) = load_json::<HistoryFile>(path)?;
        let mut history = History { entries: file.entries };
        for entry in &mut history.entries {
            entry.context = entry.context.min(MAX_CONTEXT);
        }
        history.entries.truncate(HISTORY_LIMIT);
        Ok(LoadOutcome { history, recovered })
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &HistoryFile { version: 1, entries: self.entries.clone() })
    }
```

Remove the now-unused `PathBuf` import from `history.rs` if the compiler reports it.

In `crates/rogg-core/src/stats.rs`, add after `group_digits`:

```rust
/// `one` for exactly 1, `many` otherwise: `noun(1, "match", "matches")` → `"match"`.
pub fn noun<'a>(n: u64, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}
```

and replace the `impl FilterStats` block with:

```rust
impl FilterStats {
    pub fn compute(spec: &FilterSpec, result: &FilterResult, view: &FilteredView) -> Self {
        Self::from_parts(spec, result.matches.len() as u64, &result.term_hits, result.total_lines, result.duration, view)
    }

    /// Stats for a scan whose matches are held elsewhere (the app keeps them in an `Arc`); `matched` and
    /// `term_hits` come from the scan, the pin and context counts from `view` built on that scan.
    pub fn from_parts(spec: &FilterSpec, matched: u64, term_hits: &[u64], total_lines: u64, duration: Duration, view: &FilteredView) -> Self {
        let term_hits = spec
            .terms
            .iter()
            .enumerate()
            .map(|(i, t)| TermHits { text: t.text.clone(), state: t.state, lines: term_hits.get(i).copied().unwrap_or(0) })
            .collect();
        FilterStats {
            total_lines,
            matched,
            pinned: view.pinned_count(),
            pinned_unmatched: view.pinned_unmatched(),
            context_lines: view.context_lines(),
            term_hits,
            duration,
        }
    }

    /// Footer text: `6 matches / 40 lines · 1 pinned (1 added) · 0 context`.
    pub fn summary(&self) -> String {
        let added = if self.pinned_unmatched > 0 { format!(" ({} added)", group_digits(self.pinned_unmatched)) } else { String::new() };
        format!(
            "{} {} / {} {} · {} pinned{} · {} context",
            group_digits(self.matched),
            noun(self.matched, "match", "matches"),
            group_digits(self.total_lines),
            noun(self.total_lines, "line", "lines"),
            group_digits(self.pinned),
            added,
            group_digits(self.context_lines)
        )
    }
}
```

In `crates/rogg-core/src/paths.rs`, add after `history_path`:

```rust
pub fn recent_path() -> PathBuf {
    app_support_dir().join("recent.json")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core`
Expected: all rogg-core tests pass, including the existing `history` tests and the new `persist`, `recent`, `stats` and `paths` tests.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo clippy -p rogg-core --all-targets`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): shared JSON persistence, recent files model and singular footer copy

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Log view navigation — scroll strategies, stable anchor, keyboard and range selection, copy text

**Files:**
- Modify: `crates/rogg-ui/src/logview/model.rs`, `crates/rogg-ui/src/logview/scroll.rs`, `crates/rogg-ui/src/logview/state.rs`, `crates/rogg-ui/src/logview/element.rs`, `crates/rogg-ui/src/logview/mod.rs`
- Test: `crates/rogg-ui/tests/logview.rs` (add tests), unit tests in `scroll.rs` and `state.rs`

**Interfaces:**
- Consumes: M0 `LogViewState`, `LogRowSource`, `ScrollGeometry`, `LogViewEvent`.
- Produces (used by Tasks 3, 4, 9, 11, 13):
  - `LogRowSource` gains default methods `number_at(&self, ix: usize) -> Option<u64>`, `nearest_row(&self, number: u64) -> Option<usize>`, `copy_text(&self, ix: usize) -> Option<String>`.
  - `ScrollGeometry::{ensure_visible(a, row) -> ScrollAnchor, row_top(row) -> ScrollAnchor}`.
  - `rogg_ui::logview::{ScrollTo::{Center, Visible, Top}, MAX_COPY_ROWS}`.
  - `LogViewState::{scroll_to_row(row, ScrollTo, cx), extend_selection(number, cx), selection_range() -> Option<(u64, u64)>, is_selected(number) -> bool, move_selection(delta: isize, extend: bool, cx), page(down: bool, extend: bool, cx), select_edge(last: bool, extend: bool, cx), selected_text(with_numbers: bool) -> Option<String>}`. `set_selected` clears any range; `center_on_row` stays as a wrapper; `set_rows` keeps the top line in place.
  - Keyboard/edge moves emit `LogViewEvent::Selected { row, number }` exactly like a click.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `crates/rogg-ui/src/logview/scroll.rs`:

```rust
    #[test]
    fn ensure_visible_is_minimal() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let top = g.top();
        assert_eq!(g.ensure_visible(top, 5), top, "already visible rows don't scroll");
        let down = g.ensure_visible(top, 300);
        let (_, y) = *g.visible(down).iter().find(|(r, _)| *r == 300).unwrap();
        assert!((y + 23. - 230.).abs() < 1e-3, "row 300 bottom-aligned, got y={y}");
        let up = g.ensure_visible(down, 100);
        assert_eq!(up, ScrollAnchor { row: 100, offset: 0. }, "rows above align to the top");
        assert_eq!(g.ensure_visible(top, 5000), g.bottom(), "out-of-range rows clamp");
    }

    #[test]
    fn row_top_puts_the_row_first() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        assert_eq!(g.row_top(40), ScrollAnchor { row: 40, offset: 0. });
        assert_eq!(g.row_top(999), g.bottom());
    }
```

Append to the end of `crates/rogg-ui/src/logview/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::logview::{LogLine, LogRow, VecRows};

    #[test]
    fn heights_cache_is_bounded() {
        let rows: Vec<LogRow> = (0..10)
            .map(|i| LogRow::Line(LogLine { number: i, text: "x".into(), spans: vec![], pinned: false, context: false, tint: None }))
            .collect();
        let mut s = LogViewState::new(Rc::new(VecRows::new(rows)));
        s.wrap = true;
        let many: Vec<(usize, f32)> = (0..MAX_CACHED_HEIGHTS + 10).map(|r| (r, 46.)).collect();
        s.record_heights(&many);
        assert!(s.heights.len() <= MAX_CACHED_HEIGHTS + 10);
        s.record_heights(&[(1, 69.)]);
        assert!(s.heights.len() <= 1, "an over-full cache is cleared before inserting");
    }
}
```

Add to the imports of `crates/rogg-ui/tests/logview.rs` and append the tests below. (`rows(n, gap_at)` and `open`/`render` already exist in that file.)

```rust
use rogg_ui::logview::ScrollTo;

fn rows_from(start: u64, n: usize) -> Rc<VecRows> {
    Rc::new(VecRows::new(
        (0..n)
            .map(|i| LogRow::Line(LogLine { number: start + i as u64, text: format!("line {i}").into(), spans: vec![], pinned: false, context: false, tint: None }))
            .collect(),
    ))
}

fn record(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> Rc<RefCell<Vec<String>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    cx.update(|cx| {
        cx.subscribe(state, move |_, ev: &LogViewEvent, _| {
            if let LogViewEvent::Selected { number, .. } = ev {
                e.borrow_mut().push(format!("select {number}"))
            }
        })
        .detach();
    });
    events
}

#[gpui_kit::test]
fn scroll_to_row_visible_moves_minimally(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| s.scroll_to_row(5, ScrollTo::Visible, cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 0);
    state.update(cx, |s, cx| s.scroll_to_row(300, ScrollTo::Visible, cx));
    render(cx, w);
    let view = common::bounds(cx, w, "log-test");
    let row = state.read_with(cx, |s, _| s.last_rows().iter().find(|g| g.row == 300).cloned()).expect("row 300 visible");
    assert!((row.bounds.bottom() - view.bottom()).abs() <= px(0.5), "row 300 bottom-aligned: {:?} vs {view:?}", row.bounds);
}

#[gpui_kit::test]
fn keyboard_moves_skip_gaps_page_and_jump_to_edges(cx: &mut TestAppContext) {
    common::setup(cx);
    // rows: 1000, 1001, 1002, gap, 1003, 1004, …, 1099
    let state = cx.new(|_| LogViewState::new(rows(100, Some(3))));
    let events = record(cx, &state);
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| {
        s.set_selected(Some(1002), cx);
        s.move_selection(1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1003), "the gap row is skipped");
    state.update(cx, |s, cx| s.move_selection(-1, false, cx));
    state.update(cx, |s, cx| s.select_edge(true, false, cx));
    state.update(cx, |s, cx| s.select_edge(false, false, cx));
    render(cx, w);
    // viewport 238 px: 11 rows intersect it from the top, so a page moves 10 rows (row 10 holds line 1009)
    state.update(cx, |s, cx| s.page(true, false, cx));
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1009));
    assert_eq!(*events.borrow(), ["select 1003", "select 1002", "select 1099", "select 1000", "select 1009"]);
}

#[gpui_kit::test]
fn set_rows_keeps_the_top_line_in_place(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| s.set_rows(rows_from(900, 1100), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 0, "a list at its top stays at its top");
    state.update(cx, |s, cx| s.scroll_to_row(500, ScrollTo::Top, cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 500); // line 1400
    state.update(cx, |s, cx| s.set_rows(rows(1000, None), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 400, "line 1400 stays on top");
}

#[gpui_kit::test]
fn range_selection_copies_lines_with_and_without_numbers(cx: &mut TestAppContext) {
    common::setup(cx);
    // rows: 1000..1004, gap, 1005..1009
    let state = cx.new(|_| LogViewState::new(rows(10, Some(5))));
    state.update(cx, |s, cx| {
        s.set_selected(Some(1003), cx);
        s.extend_selection(1006, cx);
    });
    state.read_with(cx, |s, _| {
        assert_eq!(s.selection_range(), Some((1003, 1006)));
        assert!(s.is_selected(1004) && !s.is_selected(1007));
        assert_eq!(s.selected_text(false).as_deref(), Some("line 3\nline 4\nline 5\nline 6\n"));
        assert_eq!(s.selected_text(true).unwrap().lines().next(), Some("1003\tline 3"));
    });
    state.update(cx, |s, cx| s.extend_selection(1001, cx));
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1001, 1003)), "the anchor stays at 1003");
    state.update(cx, |s, cx| s.set_selected(Some(1008), cx));
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1008, 1008)), "a plain selection clears the range");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --lib logview && cargo test -p rogg-ui --test logview`
Expected: compile errors — `ensure_visible`, `row_top`, `MAX_CACHED_HEIGHTS`, `ScrollTo`, `scroll_to_row`, `move_selection`, `select_edge`, `page`, `extend_selection`, `selection_range`, `is_selected`, `selected_text` not found.

- [ ] **Step 3: Implement**

In `crates/rogg-ui/src/logview/model.rs`, add these default methods at the end of `pub trait LogRowSource` (after `row_for_number`):

```rust
    /// Display number of the line at row `ix`; None for gap rows and out-of-range rows.
    fn number_at(&self, ix: usize) -> Option<u64> {
        if ix >= self.row_count() {
            return None;
        }
        match self.row(ix) {
            LogRow::Line(l) => Some(l.number),
            LogRow::Gap { .. } => None,
        }
    }

    /// The row showing `number`, or the closest row after it (the last row when none is) — used to keep the
    /// viewport on the same line when the rows change. Sources with ordered numbers should override this.
    fn nearest_row(&self, number: u64) -> Option<usize> {
        self.row_for_number(number)
    }

    /// Full text of the line at row `ix` for copying (rows may hold display-truncated text); None for gaps.
    fn copy_text(&self, ix: usize) -> Option<String> {
        if ix >= self.row_count() {
            return None;
        }
        match self.row(ix) {
            LogRow::Line(l) => Some(l.text.to_string()),
            LogRow::Gap { .. } => None,
        }
    }
```

and override `nearest_row` in `impl LogRowSource for VecRows`:

```rust
    fn nearest_row(&self, number: u64) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        let after = self.rows.iter().position(|r| matches!(r, LogRow::Line(l) if l.number >= number));
        Some(after.unwrap_or(self.rows.len() - 1))
    }
```

In `crates/rogg-ui/src/logview/scroll.rs`, add to `impl ScrollGeometry<'_>` (after `center_on`):

```rust
    /// Anchor with `row` at the top of the viewport (clamped at the end of the content).
    pub fn row_top(&self, row: usize) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        self.clamp(ScrollAnchor { row: row.min(self.row_count - 1), offset: 0. })
    }

    /// The smallest scroll that shows all of `row`: unchanged when it is visible, otherwise aligned with the
    /// nearer viewport edge. Walks only the rows between the anchor and the target that fit in the viewport.
    pub fn ensure_visible(&self, a: ScrollAnchor, row: usize) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        let row = row.min(self.row_count - 1);
        if row < a.row || (row == a.row && a.offset > 0.) {
            return self.row_top(row);
        }
        let h = (self.height_of)(row);
        let mut y = -a.offset;
        let mut r = a.row;
        while r < row && y <= self.viewport {
            y += (self.height_of)(r);
            r += 1;
        }
        if r == row && y + h <= self.viewport {
            return a;
        }
        if row == self.row_count - 1 {
            // The last row also shows the list's bottom padding.
            return self.bottom();
        }
        self.clamp(self.normalize(ScrollAnchor { row, offset: h - self.viewport }))
    }
```

Replace `crates/rogg-ui/src/logview/state.rs` above the new `#[cfg(test)]` module with:

```rust
use super::model::LogRowSource;
use super::scroll::{ScrollAnchor, ScrollGeometry};
use crate::theme::size;
use gpui_kit::{Bounds, Context, EventEmitter, Pixels, Size, px};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

/// Wrapped-row heights kept; when a measurement would exceed this, the cache is cleared first.
const MAX_CACHED_HEIGHTS: usize = 20_000;
/// A copied range holds at most this many rows (a shift-click across millions of lines copies its start).
pub const MAX_COPY_ROWS: usize = 100_000;

#[derive(Clone, Debug, PartialEq)]
pub struct RowGeometry {
    pub row: usize,
    pub bounds: Bounds<Pixels>,
    pub is_gap: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogViewEvent {
    Selected { row: usize, number: u64 },
    TogglePin { number: u64 },
}

/// Where `scroll_to_row` puts the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTo {
    Center,
    /// Only scroll if needed, by the smallest amount.
    Visible,
    Top,
}

/// Scroll position, selection, wrap and layout caches for one log panel.
pub struct LogViewState {
    pub(crate) rows: Rc<dyn LogRowSource>,
    pub(crate) anchor: ScrollAnchor,
    pub(crate) scroll_x: f32,
    pub(crate) max_scroll_x: f32,
    pub(crate) wrap: bool,
    /// The selection head (the line moved by the keyboard and shown in the status bar).
    pub(crate) selected: Option<u64>,
    /// The other end of a range selection; None for a single line.
    pub(crate) range_anchor: Option<u64>,
    pub(crate) force_hover_row: Option<usize>,
    pub(crate) pending_scroll: Option<(usize, ScrollTo)>,
    /// The request resolved by the current layout; re-applied once wrapped rows are measured.
    applied_scroll: Option<(usize, ScrollTo)>,
    pub(crate) viewport: Size<Pixels>,
    pub(crate) row_h: f32,
    /// Measured heights of wrapped rows for `heights_width`; unmeasured rows use one row height.
    pub(crate) heights: HashMap<usize, f32>,
    pub(crate) heights_width: f32,
    pub(crate) last_rows: Vec<RowGeometry>,
    initialized: bool,
}

impl EventEmitter<LogViewEvent> for LogViewState {}

impl LogViewState {
    pub fn new(rows: Rc<dyn LogRowSource>) -> Self {
        Self {
            rows,
            anchor: ScrollAnchor::default(),
            scroll_x: 0.,
            max_scroll_x: 0.,
            wrap: false,
            selected: None,
            range_anchor: None,
            force_hover_row: None,
            pending_scroll: None,
            applied_scroll: None,
            viewport: Size::default(),
            row_h: 23.,
            heights: HashMap::new(),
            heights_width: 0.,
            last_rows: Vec::new(),
            initialized: false,
        }
    }

    pub fn rows(&self) -> &Rc<dyn LogRowSource> {
        &self.rows
    }

    /// Replaces the rows, keeping the line at the top of the viewport in place (a list scrolled to its top
    /// stays at its top), so filter changes don't make the view jump.
    pub fn set_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        let at_top = !self.initialized || self.with_geometry(|g| self.anchor == g.top());
        let keep = if at_top {
            None
        } else {
            (0..3).find_map(|k| self.rows.number_at(self.anchor.row + k).map(|n| (n, k == 0)))
        };
        self.rows = rows;
        self.heights.clear();
        if let Some((number, same_row)) = keep
            && let Some(row) = self.rows.nearest_row(number)
        {
            self.anchor = ScrollAnchor { row, offset: if same_row { self.anchor.offset.max(0.) } else { 0. } };
        }
        if at_top && self.initialized {
            self.anchor = self.with_geometry(|g| g.top());
        }
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
        cx.notify();
    }

    pub fn wrap(&self) -> bool {
        self.wrap
    }

    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap;
        self.scroll_x = 0.;
        self.heights.clear();
        cx.notify();
    }

    /// Horizontal scroll offset of the text column (always zero in wrap mode).
    pub fn scroll_x(&self) -> Pixels {
        px(self.scroll_x)
    }

    pub fn selected(&self) -> Option<u64> {
        self.selected
    }

    /// Selects one line, or nothing; clears any range.
    pub fn set_selected(&mut self, number: Option<u64>, cx: &mut Context<Self>) {
        self.selected = number;
        self.range_anchor = None;
        cx.notify();
    }

    /// Extends the selection from its anchor (the line selected before the first extension) to `number`.
    pub fn extend_selection(&mut self, number: u64, cx: &mut Context<Self>) {
        if self.range_anchor.is_none() {
            self.range_anchor = Some(self.selected.unwrap_or(number));
        }
        self.selected = Some(number);
        cx.notify();
    }

    /// The selected lines as an inclusive `(first, last)` pair of display numbers.
    pub fn selection_range(&self) -> Option<(u64, u64)> {
        let head = self.selected?;
        let anchor = self.range_anchor.unwrap_or(head);
        Some((anchor.min(head), anchor.max(head)))
    }

    pub fn is_selected(&self, number: u64) -> bool {
        self.selection_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&number))
    }

    /// Scrolls `row` into place on the next layout (the viewport is only known during prepaint).
    pub fn scroll_to_row(&mut self, row: usize, how: ScrollTo, cx: &mut Context<Self>) {
        self.pending_scroll = Some((row, how));
        cx.notify();
    }

    /// Centres `row` on the next layout.
    pub fn center_on_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.scroll_to_row(row, ScrollTo::Center, cx);
    }

    /// Moves the selection by `delta` rows (skipping gap rows), scrolls it into view and emits `Selected`.
    /// With no selection, `delta >= 0` selects the first row and a negative delta the last.
    pub fn move_selection(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        if count == 0 {
            return;
        }
        let target = match self.selected.and_then(|n| self.rows.row_for_number(n)) {
            None if delta >= 0 => 0,
            None => count - 1,
            Some(row) => (row as isize + delta).clamp(0, count as isize - 1) as usize,
        };
        self.select_row(target, if delta >= 0 { 1 } else { -1 }, extend, cx);
    }

    /// PgUp / PgDn: moves the selection by one viewport of rows.
    pub fn page(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let page = self.with_geometry(|g| g.visible(self.anchor).len()).saturating_sub(1).max(1) as isize;
        self.move_selection(if down { page } else { -page }, extend, cx);
    }

    /// ⌘↑ / ⌘↓: selects the first or last line.
    pub fn select_edge(&mut self, last: bool, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        if count == 0 {
            return;
        }
        if last {
            self.select_row(count - 1, -1, extend, cx);
        } else {
            self.select_row(0, 1, extend, cx);
        }
    }

    fn select_row(&mut self, mut row: usize, step: isize, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        let mut guard = count;
        while self.rows.is_gap(row) && guard > 0 {
            let next = row as isize + step;
            if next < 0 || next >= count as isize {
                return;
            }
            row = next as usize;
            guard -= 1;
        }
        let Some(number) = self.rows.number_at(row) else { return };
        if extend {
            self.extend_selection(number, cx);
        } else {
            self.set_selected(Some(number), cx);
        }
        self.scroll_to_row(row, ScrollTo::Visible, cx);
        cx.emit(LogViewEvent::Selected { row, number });
    }

    /// The selected lines as text, one per line, gap rows skipped; `with_numbers` prefixes `number\t`.
    pub fn selected_text(&self, with_numbers: bool) -> Option<String> {
        let (lo, hi) = self.selection_range()?;
        let first = self.rows.row_for_number(lo)?;
        let last = self.rows.row_for_number(hi)?;
        let mut out = String::new();
        for ix in first..=last.min(first + MAX_COPY_ROWS - 1) {
            let (Some(number), Some(text)) = (self.rows.number_at(ix), self.rows.copy_text(ix)) else { continue };
            if with_numbers {
                let _ = write!(out, "{number}\t");
            }
            out.push_str(&text);
            out.push('\n');
        }
        Some(out)
    }

    /// Paint `row` as hovered (gallery specimens).
    pub fn force_hover_row(&mut self, row: Option<usize>, cx: &mut Context<Self>) {
        self.force_hover_row = row;
        cx.notify();
    }

    /// Bounds of the rows painted in the last frame (window coordinates); used by tests and capture geometry.
    pub fn last_rows(&self) -> &[RowGeometry] {
        &self.last_rows
    }

    pub(crate) fn height_of(&self, row: usize) -> f32 {
        if self.rows.is_gap(row) {
            size::GAP_ROW_H
        } else if self.wrap {
            self.heights.get(&row).copied().unwrap_or(self.row_h)
        } else {
            self.row_h
        }
    }

    pub(crate) fn with_geometry<R>(&self, f: impl FnOnce(&ScrollGeometry) -> R) -> R {
        let height_of = |row: usize| self.height_of(row);
        let geometry = ScrollGeometry {
            row_count: self.rows.row_count(),
            viewport: f32::from(self.viewport.height),
            pad: size::LOGS_PAD_Y,
            height_of: &height_of,
        };
        f(&geometry)
    }

    fn resolve_scroll(&self, (row, how): (usize, ScrollTo)) -> ScrollAnchor {
        self.with_geometry(|g| match how {
            ScrollTo::Center => g.center_on(row),
            ScrollTo::Visible => g.ensure_visible(self.anchor, row),
            ScrollTo::Top => g.row_top(row),
        })
    }

    /// Called at the start of prepaint with the element size and current metrics.
    pub(crate) fn begin_layout(&mut self, viewport: Size<Pixels>, text_width: f32, row_h: f32) {
        if self.row_h != row_h || (self.wrap && self.heights_width != text_width) {
            self.heights.clear();
        }
        self.row_h = row_h;
        self.heights_width = text_width;
        self.viewport = viewport;
        if !self.initialized {
            self.anchor = self.with_geometry(|g| g.top());
            self.initialized = true;
        }
        self.applied_scroll = self.pending_scroll.take();
        self.anchor = match self.applied_scroll {
            Some(request) => self.resolve_scroll(request),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    /// Stores measured wrapped heights and re-resolves this frame's scroll request with them, so a centred
    /// row stays centred once its real height is known.
    pub(crate) fn record_heights(&mut self, measured: &[(usize, f32)]) {
        if self.heights.len() + measured.len() > MAX_CACHED_HEIGHTS {
            self.heights.clear();
        }
        for &(row, h) in measured {
            self.heights.insert(row, h);
        }
        self.anchor = match self.applied_scroll {
            Some(request) => self.resolve_scroll(request),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    pub(crate) fn visible_rows(&self) -> Vec<(usize, f32)> {
        self.with_geometry(|g| g.visible(self.anchor))
    }

    pub(crate) fn thumb(&self) -> Option<(f32, f32)> {
        self.with_geometry(|g| g.thumb(self.anchor))
    }

    /// Updates the horizontal extent and re-clamps the offset (lines may have shrunk or the view widened).
    pub(crate) fn set_max_scroll_x(&mut self, max: f32) {
        self.max_scroll_x = max.max(0.);
        self.scroll_x = if self.wrap { 0. } else { self.scroll_x.clamp(0., self.max_scroll_x) };
    }

    pub(crate) fn scroll_by_px(&mut self, dy: f32, dx: f32) {
        self.anchor = self.with_geometry(|g| g.scroll_by(self.anchor, dy));
        if !self.wrap {
            self.scroll_x = (self.scroll_x + dx).clamp(0., self.max_scroll_x.max(0.));
        }
    }
}
```

The `heights_cache_is_bounded` test inserts more than the cap in one call (the cache was empty, so it is cleared then filled); the second call sees an over-full cache and clears it before inserting — that is the behaviour it asserts.

In `crates/rogg-ui/src/logview/mod.rs`, export the new items:

```rust
pub use state::{LogViewEvent, LogViewState, MAX_COPY_ROWS, RowGeometry, ScrollTo};
```

In `crates/rogg-ui/src/logview/element.rs`, paint every line of a range as selected:
- in `prepaint`, replace `s.selected()` in the tuple read from state with `s.selection_range()`, rename the tuple binding from `selected` to `selection`, and store it in `LogViewPrepaint` in place of the `selected` field (type `Option<(u64, u64)>`; update the struct definition accordingly);
- in `paint`, replace `let selected = pre.selected == Some(*number);` with

```rust
                        let selected = pre.selection.is_some_and(|(lo, hi)| (lo..=hi).contains(number));
```

- in the mouse-down handler, replace `s.selected = Some(number);` with `s.set_selected(Some(number), cx);`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery`
Expected: all pass (the gallery still compiles: `center_on_row` and `LogViewState::new` keep their signatures).

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo clippy -p rogg-ui --all-targets`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): log view scroll strategies, stable anchor, keyboard and range selection

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 3: Log view mouse — drag and shift-click range selection, scrollbar dragging, horizontal scrollbar

**Files:**
- Modify: `crates/rogg-ui/src/logview/state.rs`, `crates/rogg-ui/src/logview/scroll.rs`, `crates/rogg-ui/src/logview/element.rs`
- Test: `crates/rogg-ui/tests/logview.rs` (add tests), unit test in `scroll.rs`

**Interfaces:**
- Consumes: Task 2 `extend_selection`, `set_selected`, `ScrollGeometry`.
- Produces: `ScrollGeometry::at_position(pos: f32) -> ScrollAnchor` (inverse of `thumb`); `LogViewState` drag state (`pub(crate) drag: Option<Drag>`) and `pub(crate) fn drag_vthumb(&mut self, thumb_top: f32, track: f32, thumb_h: f32)`, `pub(crate) fn drag_hthumb(&mut self, thumb_left: f32, travel: f32)`. Mouse behaviour: left-press on a row selects it (shift extends) and starts a drag-select that extends the range while the pointer moves (auto-scrolling one row per move above/below the panel); press on the vertical thumb grabs it, press on its track jumps so the thumb centres on the pointer; without wrap a 6 px horizontal thumb appears at the bottom of the text column when lines are wider than the panel and can be dragged the same way.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `crates/rogg-ui/src/logview/scroll.rs`:

```rust
    #[test]
    fn at_position_inverts_thumb() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        assert_eq!(g.at_position(0.), g.top());
        let a = g.at_position(0.5);
        assert_eq!(a.row, 500);
        let (pos, _) = g.thumb(a).unwrap();
        assert!((pos - 0.5).abs() < 1e-3);
        assert_eq!(g.at_position(1.), g.bottom());
    }
```

Append to `crates/rogg-ui/tests/logview.rs`:

```rust
#[gpui_kit::test]
fn dragging_over_rows_extends_the_selection(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(300.), px(4. + 23. + 10.)), point(px(300.), px(4. + 23. * 4. + 10.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1001, 1004)));
}

#[gpui_kit::test]
fn dragging_the_scrollbar_thumb_scrolls(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    // The thumb sits at the right edge (x 792..798), 24 px tall at the top of the 238 px track.
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(795.), px(10.)), point(px(795.), px(117.)), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert!((400..600).contains(&first), "half-way thumb shows the middle rows, got {first}");
    assert_eq!(state.read_with(cx, |s, _| s.selected()), None, "grabbing the thumb does not select");
}

#[gpui_kit::test]
fn dragging_the_horizontal_thumb_scrolls_long_lines(cx: &mut TestAppContext) {
    common::setup(cx);
    let long = LogRow::Line(LogLine { number: 1, text: "x".repeat(500).into(), spans: vec![], pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![long]))));
    let w = open(cx, state.clone());
    // The horizontal thumb starts at the text column (x = 89) and sits 2 px above the bottom (y 230..236).
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.drag(point(px(99.), px(233.)), point(px(299.), px(233.)), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(state.read_with(cx, |s, _| s.scroll_x()) > px(0.));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --lib scroll && cargo test -p rogg-ui --test logview`
Expected: `at_position` does not compile; after stubbing, the three new log view tests fail (drag does not extend, thumb press selects a row, no horizontal thumb).

- [ ] **Step 3: Implement**

In `crates/rogg-ui/src/logview/scroll.rs`, add to `impl ScrollGeometry<'_>`:

```rust
    /// Inverse of `thumb`: the anchor whose thumb top is at fraction `pos` of the content (0 = top).
    pub fn at_position(&self, pos: f32) -> ScrollAnchor {
        if self.row_count == 0 || pos <= 0. {
            return self.top();
        }
        let exact = pos.min(1.) * self.row_count as f32;
        let row = (exact.floor() as usize).min(self.row_count - 1);
        let offset = (exact - row as f32) * (self.height_of)(row);
        self.clamp(ScrollAnchor { row, offset })
    }
```

In `crates/rogg-ui/src/logview/state.rs`:
- add after `ScrollTo`:

```rust
/// A mouse drag in progress, started by a press on a row or a scrollbar thumb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drag {
    Select,
    /// `grab`: pointer offset from the thumb's top (or left) edge.
    VThumb { grab: f32 },
    HThumb { grab: f32 },
}
```

- add the field `pub(crate) drag: Option<Drag>,` to `LogViewState` (after `last_rows`) and `drag: None,` to `new`;
- add these methods to `impl LogViewState` (after `scroll_by_px`):

```rust
    /// Scrolls so the vertical thumb's top sits `thumb_top` px below the track top (thumb of `thumb_h` px in a
    /// track of `track` px), using the same mapping the element uses to place the thumb.
    pub(crate) fn drag_vthumb(&mut self, thumb_top: f32, track: f32, thumb_h: f32) {
        let Some((_, len)) = self.thumb() else { return };
        let travel = (track - thumb_h).max(1.);
        let pos = (thumb_top / travel).clamp(0., 1.) * (1. - len);
        self.anchor = self.with_geometry(|g| g.at_position(pos));
    }

    /// Scrolls horizontally so the thumb's left edge sits `thumb_left` px into a track with `travel` px of play.
    pub(crate) fn drag_hthumb(&mut self, thumb_left: f32, travel: f32) {
        if self.wrap {
            return;
        }
        self.scroll_x = ((thumb_left / travel.max(1.)).clamp(0., 1.) * self.max_scroll_x).clamp(0., self.max_scroll_x);
    }
```

In `crates/rogg-ui/src/logview/element.rs`:
- add `MouseUpEvent` to the `gpui_kit` import list and `use super::state::Drag;`;
- add after `struct PaintRow`:

```rust
/// The horizontal scrollbar thumb (wrap off, lines wider than the panel).
#[derive(Clone, Copy)]
struct HBar {
    thumb: Bounds<Pixels>,
    track_left: f32,
    /// Track width minus thumb width.
    travel: f32,
}
```

- add `hbar: Option<HBar>,` to `LogViewPrepaint`;
- in `prepaint`, right after the `set_max_scroll_x` update, compute the bar:

```rust
        let (max_x, wrap_now, scroll_x_now) = {
            let s = self.state.read(cx);
            (s.max_scroll_x, s.wrap(), s.scroll_x)
        };
        let hbar = (!wrap_now && max_x > 0.).then(|| {
            let track_left = f32::from(text_left);
            let track_w = (f32::from(bounds.right() - text_left) - size::SCROLL_THUMB_W - 6.).max(1.);
            let len = (track_w * text_w / (text_w + max_x)).max(size::SCROLL_THUMB_MIN_H).min(track_w);
            let travel = track_w - len;
            let x = track_left + travel * (scroll_x_now / max_x).clamp(0., 1.);
            HBar {
                thumb: Bounds::new(point(px(x), bounds.bottom() - px(size::SCROLL_THUMB_W + 2.)), gsize(px(len), px(size::SCROLL_THUMB_W))),
                track_left,
                travel,
            }
        });
```

  and add `hbar` to the returned `LogViewPrepaint { … }`;
- in `paint`, after painting the vertical thumb (inside the content mask), add:

```rust
            if let Some(bar) = pre.hbar {
                window.paint_quad(fill(bar.thumb, p.scrollbar_thumb).corner_radii(Corners::all(px(3.))));
            }
```

- replace everything in `paint` from `let targets: Vec<(Hitbox, usize, u64)> = pre` to the end of the function (the mouse-down and mouse-move handlers; keep the scroll-wheel handler above it) with:

```rust
        let targets: Vec<(Hitbox, usize, u64)> = pre
            .rows
            .iter()
            .filter_map(|r| match r.body {
                RowBody::Line { number, .. } => Some((r.hitbox.clone(), r.row, number)),
                RowBody::Gap(_) => None,
            })
            .collect();
        let vthumb = pre.thumb;
        let hbar = pre.hbar;
        let track_top = bounds.top();
        let track_bottom = bounds.bottom();
        let track_h = f32::from(bounds.size.height);

        let state = self.state.clone();
        let hitbox = pre.hitbox.clone();
        let down_targets = targets.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || !hitbox.is_hovered(window) {
                return;
            }
            if let Some(thumb) = vthumb
                && e.position.x >= thumb.left() - px(4.)
            {
                let grab = if thumb.contains(&e.position) { f32::from(e.position.y - thumb.top()) } else { f32::from(thumb.size.height) / 2. };
                let thumb_h = f32::from(thumb.size.height);
                state.update(cx, |s, cx| {
                    s.drag = Some(Drag::VThumb { grab });
                    s.drag_vthumb(f32::from(e.position.y - track_top) - grab, track_h, thumb_h);
                    cx.notify();
                });
                cx.stop_propagation();
                return;
            }
            if let Some(bar) = hbar
                && e.position.y >= bar.thumb.top() - px(3.)
                && f32::from(e.position.x) >= bar.track_left
            {
                let grab = if bar.thumb.contains(&e.position) { f32::from(e.position.x - bar.thumb.left()) } else { f32::from(bar.thumb.size.width) / 2. };
                state.update(cx, |s, cx| {
                    s.drag = Some(Drag::HThumb { grab });
                    s.drag_hthumb(f32::from(e.position.x) - bar.track_left - grab, bar.travel);
                    cx.notify();
                });
                cx.stop_propagation();
                return;
            }
            if let Some((hb, row, number)) = down_targets.iter().find(|(hb, _, _)| hb.is_hovered(window)) {
                let in_gutter = e.position.x < hb.bounds.left() + px(size::GUTTER_PIN_W);
                let (row, number, shift) = (*row, *number, e.modifiers.shift);
                state.update(cx, |s, cx| {
                    if in_gutter {
                        cx.emit(LogViewEvent::TogglePin { number });
                    } else {
                        if shift {
                            s.extend_selection(number, cx);
                        } else {
                            s.set_selected(Some(number), cx);
                        }
                        s.drag = Some(Drag::Select);
                        cx.emit(LogViewEvent::Selected { row, number });
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }
        });

        let row_hitboxes: Vec<(usize, Hitbox)> = pre.rows.iter().map(|r| (r.row, r.hitbox.clone())).collect();
        let hovered_now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
        let state = self.state.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let drag = state.read(cx).drag;
            if e.pressed_button == Some(MouseButton::Left)
                && let Some(drag) = drag
            {
                match drag {
                    Drag::VThumb { grab } => {
                        if let Some(thumb) = vthumb {
                            let thumb_h = f32::from(thumb.size.height);
                            state.update(cx, |s, cx| {
                                s.drag_vthumb(f32::from(e.position.y - track_top) - grab, track_h, thumb_h);
                                cx.notify();
                            });
                        }
                    }
                    Drag::HThumb { grab } => {
                        if let Some(bar) = hbar {
                            state.update(cx, |s, cx| {
                                s.drag_hthumb(f32::from(e.position.x) - bar.track_left - grab, bar.travel);
                                cx.notify();
                            });
                        }
                    }
                    Drag::Select => {
                        if let Some((_, row, number)) = targets.iter().find(|(hb, _, _)| hb.is_hovered(window)) {
                            let (row, number) = (*row, *number);
                            state.update(cx, |s, cx| {
                                if s.selected != Some(number) {
                                    s.extend_selection(number, cx);
                                    cx.emit(LogViewEvent::Selected { row, number });
                                }
                            });
                        } else if e.position.y < track_top {
                            state.update(cx, |s, cx| {
                                s.scroll_by_px(-s.row_h, 0.);
                                cx.notify();
                            });
                        } else if e.position.y > track_bottom {
                            state.update(cx, |s, cx| {
                                s.scroll_by_px(s.row_h, 0.);
                                cx.notify();
                            });
                        }
                    }
                }
                return;
            }
            if drag.is_some() {
                // The button was released outside the window.
                state.update(cx, |s, _| s.drag = None);
            }
            let now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
            if now != hovered_now {
                window.refresh();
            }
        });

        let state = self.state.clone();
        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                state.update(cx, |s, cx| {
                    if s.drag.take().is_some() {
                        cx.notify();
                    }
                });
            }
        });
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery`
Expected: all pass, including `click_selects_and_gutter_click_toggles_pin` from M0.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo clippy -p rogg-ui --all-targets`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): drag and shift-click range selection, draggable scrollbars, horizontal bar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Log view focus, keyboard actions, copy and context menu

**Files:**
- Create: `crates/rogg-ui/src/logview/actions.rs`
- Modify: `crates/rogg-ui/src/lib.rs`, `crates/rogg-ui/src/logview/mod.rs`, `crates/rogg-ui/src/logview/state.rs`, `crates/rogg-ui/src/logview/element.rs`, `crates/rogg-ui/src/logview/text.rs`
- Test: `crates/rogg-ui/tests/logview.rs` (add tests), unit test in `text.rs`

**Interfaces:**
- Consumes: Task 2 selection API and `selected_text`; Task 3 element handlers.
- Produces (used by Tasks 6, 11, 13):
  - `rogg_ui::logview::actions::{KEY_CONTEXT = "LogView", SelectPrev, SelectNext, ExtendPrev, ExtendNext, PagePrev, PageNext, SelectFirst, SelectLast, CopySelection, CopySelectionWithNumbers}` (unit actions in namespace `log_view`; the app binds keys to them).
  - `LogView::new(id, state).focus(handle: FocusHandle)` — the panel tracks focus, dispatches its actions in `KEY_CONTEXT`, focuses itself on any mouse press, draws a 2 px accent ring while focused, and opens a context menu (Pin/Unpin, Copy, Copy with line numbers, `Filter on “word”`, `Exclude “word”`).
  - `LogViewElement::focus(handle)`.
  - `LogViewEvent::AddTerm { text: String, exclude: bool }`.
  - `rogg_ui::logview::ContextTarget { number: u64, pinned: bool, token: Option<String> }`, `LogViewState::context_target() -> Option<&ContextTarget>`.
  - `rogg_ui::logview::text::token_at(text: &str, index: usize) -> Option<String>`.
  - Right-click selects the line under the pointer (unless it is already in the selection) and records the target; "word" is the run of non-whitespace characters under the pointer (ruling: the log view selects whole lines, so the context-menu "selection" text is the word under the pointer).

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module at the end of `crates/rogg-ui/src/logview/text.rs` (create the module if the file has none):

```rust
#[cfg(test)]
mod token_tests {
    use super::token_at;

    #[test]
    fn token_at_takes_the_word_under_the_index() {
        let line = "14:32:08  ERROR  timeout waiting for metrics";
        assert_eq!(token_at(line, 12).as_deref(), Some("ERROR"));
        assert_eq!(token_at(line, 10).as_deref(), Some("ERROR"), "start of a word");
        assert_eq!(token_at(line, 9), None, "whitespace");
        assert_eq!(token_at(line, line.len()).as_deref(), Some("metrics"), "end of the line belongs to the last word");
        assert_eq!(token_at("a=é1 b", 3).as_deref(), Some("a=é1"), "indices inside a multi-byte char snap back");
        assert_eq!(token_at("cut…", 4), None, "the truncation marker is not a word");
        assert_eq!(token_at("", 0), None);
    }
}
```

Append to `crates/rogg-ui/tests/logview.rs`:

```rust
use gpui_kit::{FocusHandle, KeyBinding};
use rogg_ui::logview::actions::{self, CopySelection, CopySelectionWithNumbers, SelectLast, SelectNext};

fn open_focusable(cx: &mut TestAppContext, state: Entity<LogViewState>) -> (gpui_kit::AnyWindowHandle, FocusHandle) {
    let focus = cx.update(|cx| cx.focus_handle());
    let f = focus.clone();
    let w = common::open(cx, 800., 238., move |_, _| LogView::new("log-test", state.clone()).focus(f.clone()).into_any_element());
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("down", SelectNext, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("cmd-down", SelectLast, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("cmd-c", CopySelection, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("alt-cmd-c", CopySelectionWithNumbers, Some(actions::KEY_CONTEXT)),
        ])
    });
    (w, focus)
}

#[gpui_kit::test]
fn clicking_focuses_and_keys_move_and_copy(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let (w, focus) = open_focusable(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.click_at("log-test", point(px(300.), px(4. + 23. + 10.)), cx);
        assert!(focus.is_focused(window), "a click focuses the panel");
        window.press("down", cx);
        window.press("cmd-c", cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1002));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("line 2\n"));
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.press("cmd-down", cx);
        window.press("alt-cmd-c", cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1019));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("1019\tline 19\n"));
}

#[gpui_kit::test]
fn right_click_selects_and_records_the_context_target(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let (w, _) = open_focusable(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.right_click("log-test", cx); // the centre of a 238 px panel is row 5
    })
    .unwrap();
    cx.run_until_parked();
    state.read_with(cx, |s, _| {
        assert_eq!(s.selected(), Some(1005));
        let target = s.context_target().expect("target recorded");
        assert_eq!((target.number, target.pinned), (1005, false));
    });
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --lib token && cargo test -p rogg-ui --test logview`
Expected: compile errors — `token_at`, `actions`, `LogView::focus`, `context_target` not found.

- [ ] **Step 3: Implement**

At the top of `crates/rogg-ui/src/lib.rs`, after the crate doc comment, add:

```rust
// The Action derive emits `gpui::` paths; gpui-kit documents aliasing itself as `gpui` for that.
extern crate gpui_kit as gpui;
```

Create `crates/rogg-ui/src/logview/actions.rs`:

```rust
//! Keyboard actions handled by a focused log view. The app binds keys to them in the `LogView` context.

gpui_kit::actions!(
    log_view,
    [
        SelectPrev,
        SelectNext,
        ExtendPrev,
        ExtendNext,
        PagePrev,
        PageNext,
        SelectFirst,
        SelectLast,
        CopySelection,
        CopySelectionWithNumbers
    ]
);

/// Key context of a focused log view.
pub const KEY_CONTEXT: &str = "LogView";
```

In `crates/rogg-ui/src/logview/text.rs`, add after `display_text`:

```rust
/// The run of non-whitespace characters around byte `index` (snapped back to a char boundary); None on
/// whitespace, on the `…` truncation marker and for empty text. An index at the end belongs to the last word.
pub fn token_at(text: &str, index: usize) -> Option<String> {
    let index = text.floor_char_boundary(index.min(text.len()));
    let is_word = |c: char| !c.is_whitespace() && c != '…';
    let start = text[..index].char_indices().rev().take_while(|&(_, c)| is_word(c)).last().map_or(index, |(i, _)| i);
    let end = text[index..].char_indices().find(|&(_, c)| !is_word(c)).map_or(text.len(), |(i, _)| index + i);
    (start < end).then(|| text[start..end].to_string())
}
```

In `crates/rogg-ui/src/logview/state.rs`:
- add the variant `AddTerm { text: String, exclude: bool },` to `LogViewEvent` with the doc comment `/// Context menu "Filter on …" / "Exclude …": add the word as a filter term.`;
- add after `ScrollTo`:

```rust
/// The line under the pointer when the context menu opened, and the word under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextTarget {
    pub number: u64,
    pub pinned: bool,
    pub token: Option<String>,
}
```

- add the field `pub(crate) context_target: Option<ContextTarget>,` (and `context_target: None,` in `new`) and the accessor:

```rust
    pub fn context_target(&self) -> Option<&ContextTarget> {
        self.context_target.as_ref()
    }
```

In `crates/rogg-ui/src/logview/element.rs`:
- add `FocusHandle` to the `gpui_kit` import list, `use super::state::ContextTarget;` and `use super::text::token_at;`;
- change the element to carry an optional focus handle:

```rust
pub struct LogViewElement {
    state: Entity<LogViewState>,
    focus: Option<FocusHandle>,
}

impl LogViewElement {
    pub fn new(state: Entity<LogViewState>) -> Self {
        Self { state, focus: None }
    }

    /// Focus this handle on any mouse press inside the panel.
    pub fn focus(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }
}
```

- add to `impl Shaped`:

```rust
    /// The display text this row was shaped from.
    fn text(&self) -> SharedString {
        match self {
            Shaped::Line(line) => line.text.clone(),
            Shaped::Wrapped(lines) => lines.first().map(|l| l.text.clone()).unwrap_or_default(),
        }
    }
```

- add after `clip_spans`:

```rust
/// Byte index in `display` under `at` (relative to the text origin), by re-shaping the one row that was
/// right-clicked.
#[allow(clippy::too_many_arguments)]
fn index_at(display: &SharedString, at: Point<Pixels>, font: &Font, font_size: Pixels, wrap: Option<Pixels>, row_h: Pixels, color: Color, window: &mut Window) -> Option<usize> {
    if display.is_empty() {
        return None;
    }
    let runs = [run(display.len(), font, color, None)];
    match wrap {
        None => window.text_system().shape_line(display.clone(), font_size, &runs, None).index_for_x(at.x),
        Some(w) => window.text_system().shape_text(display.clone(), font_size, &runs, Some(w), None).ok()?.first()?.index_for_position(at, row_h).ok(),
    }
}

/// A painted line row, kept for the right-click handler.
struct MenuTarget {
    hitbox: Hitbox,
    row: usize,
    number: u64,
    pinned: bool,
    display: SharedString,
    origin: Point<Pixels>,
}
```

- add `wrap_width: Option<Pixels>,` to `LogViewPrepaint` and set it in `prepaint` to `wrap.then(|| px(text_w))`;
- in the mouse-down handler from Task 3, right after the `hitbox.is_hovered(window)` guard, focus the panel:

```rust
            if let Some(f) = &focus {
                f.focus(window, cx);
            }
```

  (declare `let focus = self.focus.clone();` next to `let hitbox = pre.hitbox.clone();` above that handler);
- at the end of `paint`, add the right-click handler (Capture phase, so it runs before the context menu opens):

```rust
        let menu_targets: Vec<MenuTarget> = pre
            .rows
            .iter()
            .filter_map(|r| match &r.body {
                RowBody::Line { number, pinned, text, .. } => Some(MenuTarget {
                    hitbox: r.hitbox.clone(),
                    row: r.row,
                    number: *number,
                    pinned: *pinned,
                    display: text.text(),
                    origin: point(pre.text_left - pre.scroll_x, r.bounds.top()),
                }),
                RowBody::Gap(_) => None,
            })
            .collect();
        let mono = font(pre.theme.fonts.mono.clone());
        let (font_size, wrap_width, text_color) = (m.log_font_size, pre.wrap_width, p.log_text);
        let state = self.state.clone();
        let focus = self.focus.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || e.button != MouseButton::Right {
                return;
            }
            let Some(t) = menu_targets.iter().find(|t| t.hitbox.is_hovered(window)) else { return };
            let index = index_at(&t.display, e.position - t.origin, &mono, font_size, wrap_width, row_h, text_color, window);
            let token = index.and_then(|i| token_at(&t.display, i));
            if let Some(f) = &focus {
                f.focus(window, cx);
            }
            let (row, number, pinned) = (t.row, t.number, t.pinned);
            state.update(cx, |s, cx| {
                if !s.is_selected(number) {
                    s.set_selected(Some(number), cx);
                    cx.emit(LogViewEvent::Selected { row, number });
                }
                s.context_target = Some(ContextTarget { number, pinned, token });
                cx.notify();
            });
        });
```

Replace `crates/rogg-ui/src/logview/mod.rs` with:

```rust
pub mod actions;
mod element;
mod model;
pub mod scroll;
mod state;
pub mod text;

pub use element::LogViewElement;
pub use model::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle, VecRows};
pub use state::{ContextTarget, LogViewEvent, LogViewState, MAX_COPY_ROWS, RowGeometry, ScrollTo};

use crate::theme::ActiveRoggTheme;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{App, ClipboardItem, Entity, FocusHandle, SharedString, TestSupportExt, Window, div};

/// A log panel: a test-support container filling its parent around the custom row element.
#[derive(IntoElement)]
pub struct LogView {
    id: SharedString,
    state: Entity<LogViewState>,
    focus: Option<FocusHandle>,
}

impl LogView {
    pub fn new(id: impl Into<SharedString>, state: Entity<LogViewState>) -> Self {
        Self { id: id.into(), state, focus: None }
    }

    /// Makes the panel focusable: mouse presses focus it, its keyboard actions dispatch in
    /// `actions::KEY_CONTEXT`, and a 2 px accent ring shows while it has focus.
    pub fn focus(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }
}

fn copy(state: &Entity<LogViewState>, with_numbers: bool, cx: &mut App) {
    if let Some(text) = state.read(cx).selected_text(with_numbers) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

fn emit(state: &Entity<LogViewState>, event: LogViewEvent, cx: &mut App) {
    state.update(cx, |_, cx| cx.emit(event));
}

/// Spec §5.5 context menu, built from the target recorded by the right-click.
fn context_menu(menu: PopupMenu, state: &Entity<LogViewState>, cx: &App) -> PopupMenu {
    let Some(target) = state.read(cx).context_target().cloned() else { return menu };
    let number = target.number;
    let (s1, s2, s3) = (state.clone(), state.clone(), state.clone());
    let menu = menu
        .item(PopupMenuItem::new(if target.pinned { "Unpin" } else { "Pin" }).on_click(move |_, _, cx| emit(&s1, LogViewEvent::TogglePin { number }, cx)))
        .separator()
        .item(PopupMenuItem::new("Copy").on_click(move |_, _, cx| copy(&s2, false, cx)))
        .item(PopupMenuItem::new("Copy with line numbers").on_click(move |_, _, cx| copy(&s3, true, cx)))
        .separator();
    match target.token {
        Some(token) => {
            let (include, exclude) = (token.clone(), token.clone());
            let (s4, s5) = (state.clone(), state.clone());
            menu.item(
                PopupMenuItem::new(format!("Filter on “{token}”"))
                    .on_click(move |_, _, cx| emit(&s4, LogViewEvent::AddTerm { text: include.clone(), exclude: false }, cx)),
            )
            .item(
                PopupMenuItem::new(format!("Exclude “{token}”"))
                    .on_click(move |_, _, cx| emit(&s5, LogViewEvent::AddTerm { text: exclude.clone(), exclude: true }, cx)),
            )
        }
        None => menu.item(PopupMenuItem::new("Filter on selection").disabled(true)).item(PopupMenuItem::new("Exclude selection").disabled(true)),
    }
}

impl RenderOnce for LogView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let focused = self.focus.as_ref().is_some_and(|f| f.is_focused(window));
        let mut element = LogViewElement::new(self.state.clone());
        if let Some(f) = self.focus.clone() {
            element = element.focus(f);
        }
        let s = self.state.clone();
        let menu_state = self.state.clone();
        div()
            .id(self.id)
            .test_support()
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .key_context(actions::KEY_CONTEXT)
            .when_some(self.focus, |d, f| d.track_focus(&f))
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectPrev, _, cx| s.update(cx, |s, cx| s.move_selection(-1, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectNext, _, cx| s.update(cx, |s, cx| s.move_selection(1, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::ExtendPrev, _, cx| s.update(cx, |s, cx| s.move_selection(-1, true, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::ExtendNext, _, cx| s.update(cx, |s, cx| s.move_selection(1, true, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::PagePrev, _, cx| s.update(cx, |s, cx| s.page(false, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::PageNext, _, cx| s.update(cx, |s, cx| s.page(true, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectFirst, _, cx| s.update(cx, |s, cx| s.select_edge(false, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::SelectLast, _, cx| s.update(cx, |s, cx| s.select_edge(true, false, cx))
            })
            .on_action({
                let s = s.clone();
                move |_: &actions::CopySelection, _, cx| copy(&s, false, cx)
            })
            .on_action(move |_: &actions::CopySelectionWithNumbers, _, cx| copy(&s, true, cx))
            .child(element)
            .when(focused, |d| d.child(div().absolute().inset_0().border_2().border_color(p.accent)))
            .context_menu(move |menu, _, cx| context_menu(menu, &menu_state, cx))
    }
}
```

If the gpui-component `ContextMenu` wrapper does not accept a `Stateful<Div>` built this way (compile error naming `ContextMenuExt`), wrap the panel in an outer `div().size_full().child(panel).context_menu(…)` instead and report the change.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery`
Expected: all pass.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo clippy -p rogg-ui --all-targets`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): log view focus, keyboard actions, copy and context menu

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 5: Chrome for the real window — placeholder layout, drag handles, toasts, title bar polish, history picker

**Files:**
- Create: `crates/rogg-ui/src/toast.rs`
- Modify: `crates/rogg-ui/src/layout.rs`, `crates/rogg-ui/src/filter.rs`, `crates/rogg-ui/src/titlebar.rs`, `crates/rogg-ui/src/dialogs.rs`, `crates/rogg-ui/src/lib.rs`
- Test: `crates/rogg-ui/tests/layout.rs`, `crates/rogg-ui/tests/chrome.rs`, `crates/rogg-ui/tests/dialogs.rs` (add tests), unit test in `layout.rs`

**Interfaces:**
- Consumes: M0 layout, filter band, title bar, dialogs.
- Produces (used by Tasks 6, 11, 12, 13, 15):
  - `rogg_ui::layout::{SingleParts { titlebar, sidebar: Option, info_bar: Option, body, status: Option, overlay: Option }, LayoutMeasure, BandDrag, SidebarDrag}`; `MainWindowLayout::{single(SingleParts), measure(Rc<LayoutMeasure>), sidebar_resizable(bool)}`; `LayoutMeasure::split_at(y: Pixels) -> Option<f32>`.
  - The filter band handle (`#band-handle`, 60×9 hit area around the 30×3 bar) starts a gpui drag carrying `BandDrag`; the sidebar's right edge (`#sidebar-resize`, 6 px) starts one carrying `SidebarDrag` when `sidebar_resizable(true)`.
  - `FilterBand::focused(bool)` — 2 px accent ring inside the band.
  - `TitleBar::tab_scroll(ScrollHandle)`; tabs show `TabItem.tooltip` on hover; double-click on the drag area zooms the window (`titlebar_double_click`), a press drags it; the traffic-light block and brand label drag the window too.
  - `rogg_ui::toast::ToastStack::new(Vec<(u64, SharedString)>).on_dismiss(Fn(u64, &mut Window, &mut App))` — bottom-right stack above the status bar; each toast `#toast-<id>` has a close button `#toast-close-<id>`.
  - `rogg_ui::dialogs::history_picker(entries: &[(SharedString, SharedString)], on_pick: IndexCallback) -> DialogFrame` — like `history`, but each row shows `(expression, states summary)` and is clickable (`#history-pick-<i>`).

- [ ] **Step 1: Write the failing tests**

Append to the end of `crates/rogg-ui/src/layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, size as gsize};

    #[test]
    fn split_at_maps_the_band_top_to_a_ratio() {
        let m = LayoutMeasure::default();
        assert_eq!(m.split_at(px(300.)), None, "no layout yet");
        // main column 600 px tall from y = 42; band 80 px tall: panes share 600 - 33 - 29 - 80 = 458 px from y = 75.
        m.main.set(Bounds::new(point(px(211.), px(42.)), gsize(px(1000.), px(600.))));
        m.band.set(Bounds::new(point(px(211.), px(300.)), gsize(px(1000.), px(80.))));
        let half = m.split_at(px(75. + 229.)).unwrap();
        assert!((half - 0.5).abs() < 1e-4, "{half}");
        assert_eq!(m.split_at(px(0.)), Some(0.1));
        assert_eq!(m.split_at(px(2000.)), Some(0.9));
    }
}
```

Append to `crates/rogg-ui/tests/layout.rs` (it already imports `gpui_kit::{TestAppContext, div, px}`, `MainWindowLayout` and `MainWindowParts`; add the two imports shown at the top):

```rust
use rogg_ui::layout::{LayoutMeasure, SingleParts};
use std::rc::Rc;

#[gpui_kit::test]
fn single_layout_has_no_panes(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1000., 700., |_, _| {
        MainWindowLayout::single(SingleParts {
            titlebar: div().id("titlebar").h(px(42.)).into_any_element(),
            sidebar: None,
            info_bar: None,
            body: div().id("placeholder-body").size_full().into_any_element(),
            status: None,
            overlay: None,
        })
        .into_any_element()
    });
    assert!(common::exists(cx, w, "placeholder-body"));
    assert!(!common::exists(cx, w, "pane-top"));
    assert_eq!(common::bounds(cx, w, "placeholder-body").size.height, px(700. - 42.));
}

#[gpui_kit::test]
fn measure_records_main_and_band_and_handles_exist(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let measure = Rc::new(LayoutMeasure::default());
    let m = measure.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let part = |id: &'static str, h: f32| div().id(id).h(px(h)).into_any_element();
        MainWindowLayout::new(MainWindowParts {
            titlebar: part("t", 42.),
            sidebar: Some(div().id("side").w(px(211.)).h_full().into_any_element()),
            info_bar: part("i", 33.),
            top_header: part("th", 35.),
            top_body: div().into_any_element(),
            band: rogg_ui::filter::FilterBand::new(vec![]).into_any_element(),
            bottom_header: part("bh", 35.),
            bottom_body: div().into_any_element(),
            status: part("s", 29.),
            overlay: None,
            split: MainWindowParts::DEFAULT_SPLIT,
        })
        .measure(m.clone())
        .sidebar_resizable(true)
        .into_any_element()
    });
    assert!(common::exists(cx, w, "band-handle"));
    assert!(common::exists(cx, w, "sidebar-resize"));
    let band = common::bounds(cx, w, "filter-band");
    let top = common::bounds(cx, w, "pane-top");
    let split = measure.split_at(band.top()).expect("measured");
    assert!((split - MainWindowParts::DEFAULT_SPLIT).abs() < 0.01, "band top maps back to the current split, got {split}");
    assert_eq!(top.top(), px(42. + 33.));
}
```

Append to `crates/rogg-ui/tests/chrome.rs` (it already imports `std::rc::Rc`; add the two imports shown at the top):

```rust
use rogg_ui::toast::ToastStack;
use std::cell::RefCell;

#[gpui_kit::test]
fn toasts_stack_bottom_right_and_dismiss(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let dismissed = Rc::new(RefCell::new(Vec::new()));
    let d = dismissed.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let d = d.clone();
        gpui_kit::div()
            .relative()
            .size_full()
            .child(ToastStack::new(vec![(7, "History reset".into()), (8, "Second".into())]).on_dismiss(move |id, _, _| d.borrow_mut().push(id)))
            .into_any_element()
    });
    let toast = common::bounds(cx, w, "toast-7");
    assert!(toast.right() <= px(1000. - 14.) + px(0.5));
    assert!(toast.bottom() <= px(700. - 29. - 12.) + px(0.5));
    common::click(cx, w, "toast-close-7");
    assert_eq!(*dismissed.borrow(), vec![7]);
}
```

Append to `crates/rogg-ui/tests/dialogs.rs` (it already imports `Cell` and `Rc`):

```rust
#[gpui_kit::test]
fn history_picker_rows_are_clickable(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let picked = Rc::new(Cell::new(None));
    let p = picked.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let p = p.clone();
        let on_pick: rogg_ui::IndexCallback = Rc::new(move |i: usize, _: &mut gpui_kit::Window, _: &mut gpui_kit::App| p.set(Some(i)));
        rogg_ui::dialogs::history_picker(&[("a || b".into(), "2 include · 0 exclude · 0 off".into()), ("c".into(), "1 include · 0 exclude · 0 off".into())], on_pick)
            .into_any_element()
    });
    common::click(cx, w, "history-pick-1");
    assert_eq!(picked.get(), Some(1));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui`
Expected: compile errors — `LayoutMeasure`, `SingleParts`, `MainWindowLayout::single/measure/sidebar_resizable`, `toast`, `history_picker` not found.

- [ ] **Step 3: Implement**

Replace `crates/rogg-ui/src/layout.rs` above the new tests with:

```rust
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Bounds, CursorStyle, EmptyView, Pixels, TestSupportExt, Window, canvas, div, px, relative};
use std::cell::Cell;
use std::rc::Rc;

pub struct MainWindowParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: AnyElement,
    pub top_header: AnyElement,
    pub top_body: AnyElement,
    pub band: AnyElement,
    pub bottom_header: AnyElement,
    pub bottom_body: AnyElement,
    pub status: AnyElement,
    pub overlay: Option<AnyElement>,
    /// Top pane share of the height left after fixed rows (reference `flex:1.1` vs `flex:1`).
    pub split: f32,
}

impl MainWindowParts {
    pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;
}

/// Window content without the log panes: the welcome view, and a tab that is loading or failed.
pub struct SingleParts {
    pub titlebar: AnyElement,
    pub sidebar: Option<AnyElement>,
    pub info_bar: Option<AnyElement>,
    pub body: AnyElement,
    pub status: Option<AnyElement>,
    pub overlay: Option<AnyElement>,
}

/// Drag payload of the filter band handle; the app turns it into a split ratio with `LayoutMeasure`.
#[derive(Clone, Copy, Debug)]
pub struct BandDrag;

/// Drag payload of the sidebar's right edge; the app turns the pointer x into a sidebar width.
#[derive(Clone, Copy, Debug)]
pub struct SidebarDrag;

/// Bounds of the main column and the filter band, recorded during layout so a band drag can be mapped to a
/// split ratio.
#[derive(Default)]
pub struct LayoutMeasure {
    main: Cell<Bounds<Pixels>>,
    band: Cell<Bounds<Pixels>>,
}

impl LayoutMeasure {
    /// The split that puts the band's top edge at window `y`, clamped to `0.1..=0.9`; None before the first layout.
    pub fn split_at(&self, y: Pixels) -> Option<f32> {
        let (main, band) = (self.main.get(), self.band.get());
        let panes = f32::from(main.size.height) - size::INFO_BAR_H - size::STATUS_BAR_H - f32::from(band.size.height);
        if panes < 1. {
            return None;
        }
        let top = f32::from(main.top()) + size::INFO_BAR_H;
        Some(((f32::from(y) - top) / panes).clamp(0.1, 0.9))
    }
}

enum Body {
    Panes(MainWindowParts),
    Single(SingleParts),
}

/// Composes the window exactly like the reference `.app`: title bar, then sidebar + main column.
#[derive(IntoElement)]
pub struct MainWindowLayout {
    body: Body,
    measure: Option<Rc<LayoutMeasure>>,
    sidebar_resizable: bool,
}

impl MainWindowLayout {
    pub fn new(parts: MainWindowParts) -> Self {
        Self { body: Body::Panes(parts), measure: None, sidebar_resizable: false }
    }

    pub fn single(parts: SingleParts) -> Self {
        Self { body: Body::Single(parts), measure: None, sidebar_resizable: false }
    }

    /// Record the main column and band bounds into `measure` on every layout.
    pub fn measure(mut self, measure: Rc<LayoutMeasure>) -> Self {
        self.measure = Some(measure);
        self
    }

    /// Show a drag edge on the sidebar's right border.
    pub fn sidebar_resizable(mut self, on: bool) -> Self {
        self.sidebar_resizable = on;
        self
    }
}

/// An invisible element filling its (relative) parent that records the parent's bounds during prepaint.
fn recorder(measure: Rc<LayoutMeasure>, band: bool) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            if band {
                measure.band.set(bounds)
            } else {
                measure.main.set(bounds)
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn side(sidebar: Option<AnyElement>, resizable: bool) -> Option<AnyElement> {
    sidebar.map(|s| {
        div()
            .relative()
            .flex_none()
            .h_full()
            .child(s)
            .when(resizable, |d| {
                d.child(
                    div()
                        .id("sidebar-resize")
                        .test_support()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right(px(-3.))
                        .w(px(6.))
                        .cursor(CursorStyle::ResizeColumn)
                        .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| EmptyView)),
                )
            })
            .into_any_element()
    })
}

impl RenderOnce for MainWindowLayout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let root = div()
            .id("app")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .line_height(relative(size::LINE_HEIGHT));
        match self.body {
            Body::Panes(parts) => {
                let split = parts.split.clamp(0.1, 0.9);
                let pane = |id: &'static str, grow: f32, header: AnyElement, body: AnyElement| {
                    div()
                        .id(id)
                        .test_support()
                        .relative()
                        .flex()
                        .flex_col()
                        .flex_grow(grow)
                        .flex_shrink(1.)
                        .flex_basis(relative(0.))
                        .min_h_0()
                        .child(header)
                        .child(div().flex_1().min_h_0().child(body))
                };
                let band = div().relative().flex_none().child(parts.band).when_some(self.measure.clone(), |d, m| d.child(recorder(m, true)));
                root.child(parts.titlebar)
                    .child(
                        div()
                            .id("workspace")
                            .test_support()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .children(side(parts.sidebar, self.sidebar_resizable))
                            .child(
                                div()
                                    .id("main")
                                    .test_support()
                                    .relative()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(parts.info_bar)
                                    .child(pane("pane-top", split, parts.top_header, parts.top_body))
                                    .child(band)
                                    .child(pane("pane-bottom", 1. - split, parts.bottom_header, parts.bottom_body))
                                    .child(parts.status)
                                    .when_some(self.measure, |d, m| d.child(recorder(m, false))),
                            ),
                    )
                    .children(parts.overlay)
            }
            Body::Single(parts) => root
                .child(parts.titlebar)
                .child(
                    div()
                        .id("workspace")
                        .test_support()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .children(side(parts.sidebar, self.sidebar_resizable))
                        .child(
                            div()
                                .id("main")
                                .test_support()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .children(parts.info_bar)
                                .child(div().flex_1().min_h_0().child(parts.body))
                                .children(parts.status),
                        ),
                )
                .children(parts.overlay),
        }
    }
}
```

In `crates/rogg-ui/src/filter.rs`:
- add `use crate::layout::BandDrag;` and `CursorStyle, EmptyView` to the `gpui_kit` import list;
- add a `focused: bool` field to `FilterBand` (`false` in `new`) with the builder:

```rust
    /// Draw the 2 px accent focus ring (the filter line is a Tab stop, spec §5.9).
    pub fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }
```

- replace the handle child (the `div().absolute().top(px(-2.)).left_0().right_0().flex().justify_center().child(div().w(px(30.))…)` block) with:

```rust
            .child(
                div().absolute().top(px(-4.)).left_0().right_0().flex().justify_center().child(
                    div()
                        .id("band-handle")
                        .test_support()
                        .flex()
                        .justify_center()
                        .w(px(60.))
                        .h(px(9.))
                        .pt(px(2.))
                        .cursor(CursorStyle::ResizeRow)
                        .on_drag(BandDrag, |_, _, _, cx| cx.new(|_| EmptyView))
                        .child(div().w(px(30.)).h(px(3.)).rounded(px(5.)).bg(p.band_handle)),
                ),
            )
```

  (the bar stays at `top: -2px`: container `-4` + padding `2`);
- after `.child(bottom)` at the end of `render`, add:

```rust
            .when(self.focused, |d| {
                d.child(div().absolute().top(px(2.)).bottom(px(2.)).left(px(2.)).right(px(2.)).rounded(px(5.)).border_2().border_color(p.accent))
            })
```

In `crates/rogg-ui/src/titlebar.rs`:
- add `ScrollHandle` to the `gpui_kit` import list and `use gpui_kit::component::tooltip::Tooltip;`;
- add a field `tab_scroll: Option<ScrollHandle>` (`None` in `new`) and the builder:

```rust
    /// Track the tab strip's scroll so the app can reveal the active tab (`ScrollHandle::scroll_to_item`).
    pub fn tab_scroll(mut self, handle: ScrollHandle) -> Self {
        self.tab_scroll = Some(handle);
        self
    }
```

- on the tab strip (`div().id("tabs")…`), add `.when_some(self.tab_scroll, |d, h| d.track_scroll(&h))` before `.children(…)`;
- add this free function and use it for the drag area, the traffic block and the brand label:

```rust
/// A press drags the window; a double-click zooms it like a native title bar.
fn drag_window(e: &gpui_kit::MouseDownEvent, window: &mut Window, _: &mut App) {
    if e.click_count >= 2 {
        window.titlebar_double_click();
    } else {
        window.start_window_move();
    }
}
```

  — replace `.on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())` on `#window-drag` with `.on_mouse_down(MouseButton::Left, drag_window)`; give the traffic div `.id("traffic").on_mouse_down(MouseButton::Left, drag_window)`; replace the brand label child with `div().id("brand").text_size(px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).on_mouse_down(MouseButton::Left, drag_window).child("rogg")`;
- in `TabShell::render`, on the inner tab div (`.id(format!("tab-{i}"))`), add before `.on_click(…)`:

```rust
                    .when(!self.item.tooltip.is_empty(), |d| {
                        let tip = self.item.tooltip.clone();
                        d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    })
```

  (clone `self.item.label` into the child before this if the borrow checker requires it).

Create `crates/rogg-ui/src/toast.rs`:

```rust
//! Transient notifications (spec §5.7): a stack at the bottom right, above the status bar.

use crate::controls::IconButton;
use crate::theme::{ActiveRoggTheme, size};
use gpui_kit::prelude::*;
use gpui_kit::{App, BoxShadow, SharedString, TestSupportExt, Window, div, px};
use std::rc::Rc;

type DismissCallback = Rc<dyn Fn(u64, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ToastStack {
    toasts: Vec<(u64, SharedString)>,
    on_dismiss: Option<DismissCallback>,
}

impl ToastStack {
    pub fn new(toasts: Vec<(u64, SharedString)>) -> Self {
        Self { toasts, on_dismiss: None }
    }

    pub fn on_dismiss(mut self, f: impl Fn(u64, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ToastStack {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.rogg().palette;
        let on_dismiss = self.on_dismiss;
        div()
            .id("toasts")
            .test_support()
            .absolute()
            .right(px(14.))
            .bottom(px(size::STATUS_BAR_H + 12.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(self.toasts.into_iter().map(|(id, message)| {
                let on_dismiss = on_dismiss.clone();
                div()
                    .id(SharedString::from(format!("toast-{id}")))
                    .test_support()
                    .occlude()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .max_w(px(360.))
                    .pl(px(14.))
                    .pr(px(6.))
                    .py(px(6.))
                    .bg(p.side)
                    .border_1()
                    .border_color(p.border)
                    .rounded(px(8.))
                    .shadow(vec![BoxShadow::new(px(0.), px(8.), p.dialog_shadow.hsla()).blur_radius(px(30.))])
                    .text_size(px(11.))
                    .text_color(p.text)
                    .child(div().flex_1().min_w_0().child(message))
                    .child(IconButton::glyph(SharedString::from(format!("toast-close-{id}")), "✕").compact().on_click(move |w, cx| {
                        if let Some(f) = &on_dismiss {
                            f(id, w, cx)
                        }
                    }))
            }))
    }
}
```

Add `pub mod toast;` to `crates/rogg-ui/src/lib.rs`.

In `crates/rogg-ui/src/dialogs.rs`, add after `history`:

```rust
/// Filter history whose rows restore an entry when clicked. Each entry is `(expression, states summary)`,
/// e.g. `("error || timeout", "1 include · 1 exclude · 0 off")` (spec §5.8: expression and states).
pub fn history_picker(entries: &[(SharedString, SharedString)], on_pick: IndexCallback) -> DialogFrame {
    entries.iter().enumerate().fold(DialogFrame::new("Filter history"), |d, (i, (expression, states))| {
        let on_pick = on_pick.clone();
        d.child(
            div()
                .id(SharedString::from(format!("history-pick-{i}")))
                .test_support()
                .cursor_pointer()
                .on_click(move |_, w, cx| on_pick(i, w, cx))
                .child(RuleRow::code(format!("history-{i}"), expression.clone()).trailing(states.clone())),
        )
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery`
Expected: all pass. The gallery's replica, filter and chrome pages render unchanged (same `MainWindowParts`, handle bar at the same place).

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo clippy -p rogg-ui --all-targets && just visual-check`
Expected: no warnings; `visual-check` passes against the M0 golden images (the band handle and title bar look identical).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-ui
git commit -m "feat(ui): placeholder layout, band and sidebar drag handles, toasts, title bar polish

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 6: rogg-app scaffold — crate, CLI, keymap table, menus, window, welcome view

**Files:**
- Create: `crates/rogg-app/Cargo.toml`, `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/main.rs`, `crates/rogg-app/src/args.rs`, `crates/rogg-app/src/paths.rs`, `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/menus.rs`, `crates/rogg-app/src/window.rs`, `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-app/tests/common/mod.rs`, `crates/rogg-app/tests/app.rs`
- Modify: `Cargo.toml` (workspace members), `justfile`
- Test: unit tests in `args.rs`, `actions.rs`, `menus.rs`; `crates/rogg-app/tests/app.rs`

**Interfaces:**
- Consumes: `rogg_ui::{assets::Assets, theme::{RoggTheme, Appearance, Density, size}, titlebar::TitleBar, layout::{MainWindowLayout, SingleParts}, empty::welcome, dialogs::{shortcuts, ModalLayer}, logview::actions::*}` (Tasks 4–5); `rogg_core::paths` (Task 1).
- Produces (used by every later app task):
  - `rogg_app::{run(Args), init_with(&mut App, AppPaths), install_globals(&mut App, AppPaths)}` — `init_with` = `gpui_kit::init` + dark compact theme + `install_globals` + menus; `install_globals` sets the app globals and binds keys (the gallery calls it in Task 15).
  - `rogg_app::args::{Args { demo: bool, paths: Vec<PathBuf> }, Args::parse(impl IntoIterator<Item = String>) -> Result<Args, String>, USAGE}`.
  - `rogg_app::paths::AppPaths { history, recent, cache }` (gpui `Global`), `AppPaths::{system(), under(&Path)}`.
  - `rogg_app::actions::{Quit, OpenFile, OpenFolder, CloseTab, NextTab, PrevTab, ActivateTab { index }, OpenRecent { index }, Find, EditFilters, ToggleRegex, ToggleCase, TogglePin, GoToLine, RevealInFullLog, FocusNext, FocusPrev, ToggleFollow, ToggleSidebar, ToggleWrap, ShowShortcuts, Dismiss, ThemeSystem, ThemeLight, ThemeDark, DensityCompact, DensityComfortable, Minimize, Zoom}`, `WORKSPACE_CONTEXT = "Workspace"`, `FILTER_CONTEXT = "FilterBand"`, `Shortcut { label, display, binding: fn() -> KeyBinding }`, `shortcuts() -> Vec<Shortcut>` (the single keymap table, spec §5.9), `bind(&mut App)`, `shortcut_rows() -> Vec<(SharedString, SharedString)>`.
  - `rogg_app::menus::app_menus(recent: &[RecentEntry]) -> Vec<Menu>`.
  - `rogg_app::window::{window_options(&App) -> WindowOptions, open_main_window(Args, &mut App) -> anyhow::Result<()>}`.
  - `rogg_app::workspace::{Workspace, Dialog}`; `Workspace::new(Args, &mut Window, &mut Context<Workspace>)`, `Workspace::open_dialog(Dialog, cx)`, `Workspace::close_dialog(cx)`, `Workspace::set_traffic_lights(bool, cx)`; root element `#root` with key context `Workspace`; with no tabs it renders the welcome view (`#welcome`).

- [ ] **Step 1: Write the crate manifest, the failing unit tests and the integration test**

Add `"crates/rogg-app"` to `members` in the root `Cargo.toml`:

```toml
members = ["crates/rogg-ui", "crates/rogg-gallery", "crates/rogg-core", "crates/rogg-genlog", "crates/rogg-app"]
```

Create `crates/rogg-app/Cargo.toml`:

```toml
[package]
name = "rogg-app"
version.workspace = true
edition.workspace = true
publish.workspace = true

[lib]
name = "rogg_app"
path = "src/lib.rs"

[[bin]]
name = "rogg"
path = "src/main.rs"

[dependencies]
rogg-core = { path = "../rogg-core" }
rogg-ui = { path = "../rogg-ui" }
gpui-kit.workspace = true
anyhow.workspace = true

[dev-dependencies]
gpui-kit = { workspace = true, features = ["test-support"] }
tempfile.workspace = true
```

Create `crates/rogg-app/src/args.rs` with its tests:

```rust
//! Command line: `rogg [--demo] [FILE...]`.

use std::path::PathBuf;

pub const USAGE: &str = "usage: rogg [--demo] [FILE...]";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Open the reference sample data (spec §9 M2).
    pub demo: bool,
    pub paths: Vec<PathBuf>,
}

impl Args {
    /// Parses the arguments after the program name. `-h`/`--help` returns `Err(USAGE)`.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
        let mut out = Args::default();
        for arg in args {
            match arg.as_str() {
                "--demo" => out.demo = true,
                "-h" | "--help" => return Err(USAGE.to_string()),
                flag if flag.starts_with('-') => return Err(format!("unknown option `{flag}`\n{USAGE}")),
                _ => out.paths.push(PathBuf::from(arg)),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Args, String> {
        Args::parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_paths_and_demo() {
        assert_eq!(parse("").unwrap(), Args::default());
        let a = parse("--demo a.log /tmp/b.log").unwrap();
        assert!(a.demo);
        assert_eq!(a.paths, [PathBuf::from("a.log"), PathBuf::from("/tmp/b.log")]);
    }

    #[test]
    fn rejects_unknown_flags_and_answers_help() {
        assert!(parse("--wat").unwrap_err().contains("unknown option `--wat`"));
        assert_eq!(parse("-h").unwrap_err(), USAGE);
    }
}
```

Create `crates/rogg-app/src/actions.rs`:

```rust
//! App actions and the single keymap table (spec §5.9). Bindings for log-view navigation live here too, so
//! every shortcut is defined in one place and can be remapped later.

use gpui_kit::{App, KeyBinding, SharedString};
use rogg_ui::logview::actions as log;

gpui_kit::actions!(
    rogg,
    [
        Quit,
        OpenFile,
        OpenFolder,
        CloseTab,
        NextTab,
        PrevTab,
        Find,
        EditFilters,
        ToggleRegex,
        ToggleCase,
        TogglePin,
        GoToLine,
        RevealInFullLog,
        FocusNext,
        FocusPrev,
        ToggleFollow,
        ToggleSidebar,
        ToggleWrap,
        ShowShortcuts,
        Dismiss,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        DensityCompact,
        DensityComfortable,
        Minimize,
        Zoom
    ]
);

/// ⌘1…⌘9: activate the tab at `index`.
#[derive(Clone, PartialEq, Debug, gpui_kit::Action)]
#[action(namespace = rogg, no_json)]
pub struct ActivateTab {
    pub index: usize,
}

/// File → Open Recent ▸ entry `index`.
#[derive(Clone, PartialEq, Debug, gpui_kit::Action)]
#[action(namespace = rogg, no_json)]
pub struct OpenRecent {
    pub index: usize,
}

pub const WORKSPACE_CONTEXT: &str = "Workspace";
/// Key context of the focused filter line (when not editing).
pub const FILTER_CONTEXT: &str = "FilterBand";

/// One keymap entry. `label` and `display` feed the shortcuts dialog; entries with an empty label are bound but
/// not listed (⌘2…⌘9 share the ⌘1 row).
pub struct Shortcut {
    pub label: &'static str,
    pub display: &'static str,
    pub binding: fn() -> KeyBinding,
}

macro_rules! shortcut {
    ($label:literal, $display:literal, $keys:literal, $action:expr, $context:expr) => {
        Shortcut { label: $label, display: $display, binding: || KeyBinding::new($keys, $action, $context) }
    };
}

/// The keymap. Shortcuts for features that arrive in later milestones (open as tab, export, settings, live
/// follow pause) are added with those features.
pub fn shortcuts() -> Vec<Shortcut> {
    const WS: Option<&str> = Some(WORKSPACE_CONTEXT);
    const LOG: Option<&str> = Some(log::KEY_CONTEXT);
    const FILTER: Option<&str> = Some(FILTER_CONTEXT);
    vec![
        shortcut!("Open file", "⌘ O", "cmd-o", OpenFile, None),
        shortcut!("Open folder", "⇧ ⌘ O", "shift-cmd-o", OpenFolder, None),
        shortcut!("Close tab", "⌘ W", "cmd-w", CloseTab, WS),
        shortcut!("Switch to tab 1…9", "⌘ 1…9", "cmd-1", ActivateTab { index: 0 }, WS),
        shortcut!("", "", "cmd-2", ActivateTab { index: 1 }, WS),
        shortcut!("", "", "cmd-3", ActivateTab { index: 2 }, WS),
        shortcut!("", "", "cmd-4", ActivateTab { index: 3 }, WS),
        shortcut!("", "", "cmd-5", ActivateTab { index: 4 }, WS),
        shortcut!("", "", "cmd-6", ActivateTab { index: 5 }, WS),
        shortcut!("", "", "cmd-7", ActivateTab { index: 6 }, WS),
        shortcut!("", "", "cmd-8", ActivateTab { index: 7 }, WS),
        shortcut!("", "", "cmd-9", ActivateTab { index: 8 }, WS),
        shortcut!("Next / previous tab", "⌃ Tab / ⌃ ⇧ Tab", "ctrl-tab", NextTab, WS),
        shortcut!("", "", "ctrl-shift-tab", PrevTab, WS),
        shortcut!("Find in focused panel", "⌘ F", "cmd-f", Find, WS),
        shortcut!("Edit filters", "⌘ L", "cmd-l", EditFilters, WS),
        shortcut!("", "", "enter", EditFilters, FILTER),
        shortcut!("Toggle regex / case", "⌥ ⌘ R / ⌥ ⌘ C", "alt-cmd-r", ToggleRegex, WS),
        shortcut!("", "", "alt-cmd-c", ToggleCase, WS),
        shortcut!("Pin / unpin selected line", "⌘ B", "cmd-b", TogglePin, WS),
        shortcut!("Go to line", "⌘ G", "cmd-g", GoToLine, WS),
        shortcut!("Move through results", "↑ ↓", "down", log::SelectNext, LOG),
        shortcut!("", "", "up", log::SelectPrev, LOG),
        shortcut!("Extend selection", "⇧ ↑ ↓", "shift-down", log::ExtendNext, LOG),
        shortcut!("", "", "shift-up", log::ExtendPrev, LOG),
        shortcut!("Page", "PgUp / PgDn", "pagedown", log::PageNext, LOG),
        shortcut!("", "", "pageup", log::PagePrev, LOG),
        shortcut!("Top / bottom", "⌘ ↑ / ⌘ ↓", "cmd-up", log::SelectFirst, LOG),
        shortcut!("", "", "cmd-down", log::SelectLast, LOG),
        shortcut!("Copy lines", "⌘ C", "cmd-c", log::CopySelection, LOG),
        shortcut!("Reveal in full log", "Return", "enter", RevealInFullLog, LOG),
        shortcut!("Cycle focus", "Tab / ⇧ Tab", "tab", FocusNext, WS),
        shortcut!("", "", "shift-tab", FocusPrev, WS),
        shortcut!("Toggle follow selection", "⇧ ⌘ F", "shift-cmd-f", ToggleFollow, WS),
        shortcut!("Toggle sidebar", "⌘ \\", "cmd-\\", ToggleSidebar, WS),
        shortcut!("Close dialog / cancel", "Esc", "escape", Dismiss, WS),
        shortcut!("Quit", "⌘ Q", "cmd-q", Quit, None),
    ]
}

/// Installs every keymap entry.
pub fn bind(cx: &mut App) {
    cx.bind_keys(shortcuts().into_iter().map(|s| (s.binding)()));
}

/// Rows for the keyboard shortcuts dialog: listed entries only.
pub fn shortcut_rows() -> Vec<(SharedString, SharedString)> {
    shortcuts().into_iter().filter(|s| !s.label.is_empty()).map(|s| (s.label.into(), s.display.into())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_binding_parses_and_is_unique_per_context() {
        let mut seen = HashSet::new();
        for s in shortcuts() {
            let binding = (s.binding)();
            let keys: Vec<String> = binding.keystrokes().iter().map(|k| format!("{k:?}")).collect();
            let context = binding.predicate().map(|p| format!("{p:?}"));
            assert!(seen.insert((keys.clone(), context.clone())), "duplicate binding {keys:?} in {context:?}");
        }
    }

    #[test]
    fn listed_rows_skip_the_unlabelled_entries() {
        let rows = shortcut_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Edit filters" && keys == "⌘ L"));
        assert!(rows.iter().all(|(label, _)| !label.is_empty()));
    }
}
```

If `KeyBinding::keystrokes()` or `predicate()` are named differently in gpui-pre 0.3.8, compare `format!("{:?}", binding)` strings instead and report it.

Create `crates/rogg-app/src/menus.rs`:

```rust
//! Native menu bar (spec §5.9): File, Edit, View and Window.

use crate::actions::*;
use gpui_kit::{Menu, MenuItem, OsAction, SharedString};
use rogg_core::recent::RecentEntry;
use rogg_ui::logview::actions::CopySelection;

pub fn app_menus(recent: &[RecentEntry]) -> Vec<Menu> {
    let recent_items: Vec<MenuItem> = recent
        .iter()
        .enumerate()
        .map(|(index, e)| MenuItem::action(SharedString::from(e.label()), OpenRecent { index }))
        .collect();
    vec![
        Menu::new("rogg").items([MenuItem::action("Quit rogg", Quit)]),
        Menu::new("File").items([
            MenuItem::action("Open…", OpenFile),
            MenuItem::action("Open Folder…", OpenFolder),
            MenuItem::submenu(Menu::new("Open Recent").items(recent_items)),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Copy", CopySelection, OsAction::Copy),
            MenuItem::action("Find", Find),
            MenuItem::action("Go to Line…", GoToLine),
        ]),
        Menu::new("View").items([
            MenuItem::action("Sidebar", ToggleSidebar),
            MenuItem::action("Wrap", ToggleWrap),
            MenuItem::submenu(Menu::new("Theme").items([
                MenuItem::action("System", ThemeSystem),
                MenuItem::action("Light", ThemeLight),
                MenuItem::action("Dark", ThemeDark),
            ])),
            MenuItem::submenu(Menu::new("Density").items([
                MenuItem::action("Compact", DensityCompact),
                MenuItem::action("Comfortable", DensityComfortable),
            ])),
        ]),
        Menu::new("Window").items([MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_follow_the_spec_and_list_recent_entries() {
        let menus = app_menus(&[RecentEntry::file("/var/log/a.log"), RecentEntry::folder("/logs")]);
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_ref()).collect();
        assert_eq!(names, ["rogg", "File", "Edit", "View", "Window"]);
        let file = &menus[1];
        let recent = file.items.iter().find_map(|i| match i {
            MenuItem::Submenu(m) if m.name.as_ref() == "Open Recent" => Some(m),
            _ => None,
        });
        assert_eq!(recent.expect("Open Recent submenu").items.len(), 2);
    }
}
```

If `MenuItem::os_action` takes its arguments in a different order in gpui-pre 0.3.8, follow the compiler and keep the Copy item bound to `CopySelection`.

Create `crates/rogg-app/src/paths.rs`:

```rust
//! Where the app keeps its state. A global, so tests and the gallery can point it at a temp directory.

use gpui_kit::Global;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct AppPaths {
    pub history: PathBuf,
    pub recent: PathBuf,
    /// Transcoded UTF-16 files (spec §4.2).
    pub cache: PathBuf,
}

impl Global for AppPaths {}

impl AppPaths {
    /// `~/Library/Application Support/rogg/…` and `~/Library/Caches/rogg`.
    pub fn system() -> Self {
        Self { history: rogg_core::paths::history_path(), recent: rogg_core::paths::recent_path(), cache: rogg_core::paths::cache_dir() }
    }

    /// Everything under `dir` (tests, gallery captures).
    pub fn under(dir: &Path) -> Self {
        Self { history: dir.join("history.json"), recent: dir.join("recent.json"), cache: dir.join("cache") }
    }
}
```

Create `crates/rogg-app/src/window.rs`:

```rust
//! The main window: transparent title bar with native traffic lights placed in the leading block.

use crate::args::Args;
use crate::workspace::Workspace;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};

/// Default 1440×900 like the reference captures; traffic lights at (13, 14) to sit in the 53 px block of the
/// 42 px title bar (tuned during the M2 visual review).
pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position: Some(point(px(13.), px(14.))) }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(760.), px(520.))),
        ..Default::default()
    }
}

pub fn open_main_window(args: Args, cx: &mut App) -> anyhow::Result<()> {
    gpui_kit::open_window(window_options(cx), cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx)))?;
    Ok(())
}
```

Create `crates/rogg-app/src/workspace/mod.rs`:

```rust
//! The main window view: title bar, tabs, sidebar, dialogs and the app's actions.

use crate::actions::{self, Dismiss, OpenFile, OpenFolder, ShowShortcuts};
use crate::args::Args;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, FocusHandle, Pixels, Window, div, px};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{MainWindowLayout, SingleParts};
use rogg_ui::theme::size;
use rogg_ui::titlebar::TitleBar;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    Shortcuts,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    pub(crate) sidebar_visible: bool,
    pub(crate) sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    traffic_lights: bool,
}

/// A `Fn(&mut Window, &mut App)` callback that runs `f` on the workspace.
pub(crate) fn act(this: &Entity<Workspace>, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| f(ws, window, cx))
}

/// Like `act`, for callbacks carrying an index.
pub(crate) fn act_ix(
    this: &Entity<Workspace>,
    f: impl Fn(&mut Workspace, usize, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| f(ws, i, window, cx))
}

impl Workspace {
    pub fn new(_args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self { focus, sidebar_visible: true, sidebar_width: px(size::SIDEBAR_W), dialog: None, traffic_lights: false }
    }

    /// Draw the traffic lights ourselves (headless captures have no native ones).
    pub fn set_traffic_lights(&mut self, on: bool, cx: &mut Context<Self>) {
        self.traffic_lights = on;
        cx.notify();
    }

    pub fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    pub(crate) fn dismiss(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        }
    }

    fn titlebar(&self, this: &Entity<Self>) -> TitleBar {
        TitleBar::new(vec![])
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
            .on_shortcuts(act(this, |ws, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
    }

    fn dialog_element(&self, dialog: &Dialog, this: &Entity<Self>) -> AnyElement {
        match dialog {
            Dialog::Shortcuts => dialogs::shortcuts(&actions::shortcut_rows())
                .description("Keyboard shortcuts for the full log, the filter line and the filtered log.")
                .on_close(act(this, |ws, _, cx| ws.close_dialog(cx)))
                .into_any_element(),
        }
    }

    fn overlay(&self, this: &Entity<Self>) -> Option<AnyElement> {
        self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))).into_any_element())
    }

    fn welcome(&self) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        empty::welcome(vec![], open_file, open_folder, Rc::new(|_: usize, _: &mut Window, _: &mut App| {})).into_any_element()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let content = MainWindowLayout::single(SingleParts {
            titlebar: self.titlebar(&this).into_any_element(),
            sidebar: None,
            info_bar: None,
            body: self.welcome(),
            status: None,
            overlay: self.overlay(&this),
        });
        div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(cx.listener(|ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .child(content)
    }
}
```

Create `crates/rogg-app/src/lib.rs`:

```rust
//! The rogg application: windows, tabs, actions, and the wiring between rogg-core and rogg-ui (spec §3.2).

// The Action derive emits `gpui::` paths; gpui-kit documents aliasing itself as `gpui` for that.
extern crate gpui_kit as gpui;

pub mod actions;
pub mod args;
pub mod menus;
pub mod paths;
pub mod window;
pub mod workspace;

use gpui_kit::{App, QuitMode};
use paths::AppPaths;
use rogg_ui::theme::{Appearance, Density, RoggTheme};

/// Runs the app until its last window closes.
pub fn run(args: args::Args) {
    gpui_kit::application().with_assets(rogg_ui::assets::Assets).with_quit_mode(QuitMode::LastWindowClosed).run(move |cx: &mut App| {
        init_with(cx, AppPaths::system());
        if let Err(e) = window::open_main_window(args, cx) {
            eprintln!("rogg: could not open the main window: {e:#}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}

/// Full initialisation: gpui-kit, the default theme, app globals and key bindings, menus.
pub fn init_with(cx: &mut App, paths: AppPaths) {
    gpui_kit::init(cx);
    RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
    install_globals(cx, paths);
    cx.set_menus(menus::app_menus(&[]));
}

/// App globals and key bindings only, for hosts that initialise gpui-kit and the theme themselves (the gallery).
pub fn install_globals(cx: &mut App, paths: AppPaths) {
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
}
```

Create `crates/rogg-app/src/main.rs`:

```rust
fn main() {
    match rogg_app::args::Args::parse(std::env::args().skip(1)) {
        Ok(args) => rogg_app::run(args),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message == rogg_app::args::USAGE { 0 } else { 2 });
        }
    }
}
```

Create `crates/rogg-app/tests/common/mod.rs`:

```rust
//! Headless app windows for integration tests. gpui's TestAppContext has a no-op text system: assert on
//! state and on fixed sizes, not on text widths.
#![allow(dead_code)]
use gpui_kit::prelude::*;
use gpui_kit::{AnyWindowHandle, Bounds, Entity, Pixels, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use rogg_app::args::Args;
use rogg_app::paths::AppPaths;
use rogg_app::workspace::Workspace;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Initialises the app with state files in a fresh temp dir (kept alive by the returned guard).
pub fn setup(cx: &mut TestAppContext) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    cx.update(|cx| rogg_app::init_with(cx, paths));
    dir
}

pub fn open_workspace(cx: &mut TestAppContext, args: Args) -> (AnyWindowHandle, Entity<Workspace>) {
    let handle = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx))).expect("open window")
    });
    cx.run_until_parked();
    handle
}

pub fn render(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
}

pub fn exists(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

pub fn bounds(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.find(id).bounds()
    })
    .unwrap()
}

pub fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

pub fn press(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &str) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.press(keys, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Advances the test clock (driving the 16 ms poll task) while real background threads work, until `done`.
pub fn wait_until(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..3000 {
        if done(cx) {
            return;
        }
        cx.executor().advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("condition not reached within the time limit");
}

/// Writes `lines` (joined with `\n`) to `dir/name` and returns the path.
pub fn write_log(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}
```

Create `crates/rogg-app/tests/app.rs`:

```rust
mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;

#[gpui_kit::test]
fn opens_to_the_welcome_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, _) = common::open_workspace(cx, Args::default());
    assert!(common::exists(cx, w, "welcome"));
    assert!(common::exists(cx, w, "titlebar"));
    assert!(common::exists(cx, w, "open-file"));
}

#[gpui_kit::test]
fn shortcuts_dialog_opens_from_the_title_bar_and_escape_closes_it(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, _) = common::open_workspace(cx, Args::default());
    common::click(cx, w, "shortcuts");
    assert!(common::exists(cx, w, "dialog"));
    common::press(cx, w, "escape");
    assert!(!common::exists(cx, w, "dialog"));
}
```

Append to `justfile`:

```just
run *args:
    cargo run -p rogg-app -- {{args}}

demo:
    cargo run -p rogg-app -- --demo
```

- [ ] **Step 2: Run the tests to verify the scaffold builds and passes**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: the unit tests in `args`, `actions`, `menus` and both tests in `tests/app.rs` pass. (Write the code first, then delete the body of `Workspace::dismiss` once and confirm `shortcuts_dialog_opens…` fails, then restore it — record that RED run in the report.)

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo build -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: builds, no warnings.

- [ ] **Step 3: Smoke-run the binary**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo run -p rogg-app -- --help; echo "exit $?"`
Expected: prints `usage: rogg [--demo] [FILE...]` and `exit 0`. (Do not leave a GUI window running.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock justfile crates/rogg-app
git commit -m "feat(app): rogg-app crate with CLI, keymap table, menus, window and welcome view

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 7: Document — opening files synchronously or in the background, with progress, cancel and retry

**Files:**
- Create: `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/document/load.rs`, `crates/rogg-app/tests/document.rs`
- Modify: `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes: `rogg_core::text::{LogText, OpenProgress}`, `CancelToken`, `CoreError` (M1); `AppPaths` (Task 6).
- Produces (used by Tasks 8–14):
  - `rogg_app::document::{POLL_INTERVAL, EngineConfig { sync_bytes }, Source::{File(PathBuf), Memory { name, bytes: Arc<Vec<u8>> }}, DocumentInit { number_base }, LoadState::{Loading { fraction, label }, Ready(Arc<LogText>), Failed { message }}, Document}`.
  - `EngineConfig::get(&App) -> EngineConfig` (default 4 MiB); `Source::{title() -> SharedString, path() -> Option<&Path>}`.
  - `Document::{new(Source, DocumentInit, &mut Context<Document>), title(), number_base(), text() -> Option<&Arc<LogText>>, text_revision() -> u64, retry(cx), cancel_load(cx)}`, `pub(crate) fn ensure_polling(cx)`, private `poll_jobs(cx) -> bool` and `finish_load(result, cx)` (Task 8 and 10 extend both).
  - Behaviour: memory sources and files up to `sync_bytes` load during `new`; larger files load on a `rogg-open` thread whose progress is polled every 16 ms (`Loading { label: "Indexing" | "Converting UTF-16 to UTF-8", fraction }`); I/O failures become `Failed { message }` with the OS error text (the path is shown separately); a lost worker becomes `Failed { "Opening stopped unexpectedly" }`; `cancel_load` → `Failed { "Opening cancelled" }`; `retry` reloads a failed document.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/document.rs`:

```rust
mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, LoadState, Source};

fn open(cx: &mut TestAppContext, source: Source) -> Entity<Document> {
    cx.new(|cx| Document::new(source, DocumentInit::default(), cx))
}

fn lines(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<u64> {
    doc.read_with(cx, |d, _| d.text().map(|t| t.line_count()))
}

fn failure(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| match &d.load {
        LoadState::Failed { message } => Some(message.to_string()),
        _ => None,
    })
}

#[gpui_kit::test]
fn small_files_open_synchronously(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "a.log", &["one", "two", "three"]);
    let doc = open(cx, Source::File(path));
    assert_eq!(lines(&doc, cx), Some(3));
    assert_eq!(doc.read_with(cx, |d, _| (d.title().to_string(), d.text_revision(), d.number_base())), ("a.log".to_string(), 1, 1));
}

#[gpui_kit::test]
fn large_files_open_in_the_background(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let path = common::write_log(dir.path(), "big.log", &["x"; 1000]);
    let doc = open(cx, Source::File(path));
    assert!(matches!(doc.read_with(cx, |d, _| d.load.clone()), LoadState::Loading { .. }));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    assert_eq!(lines(&doc, cx), Some(1000));
}

#[gpui_kit::test]
fn empty_and_missing_files(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let empty = common::write_log(dir.path(), "empty.log", &[]);
    assert_eq!(lines(&open(cx, Source::File(empty)), cx), Some(0));
    let later = dir.path().join("later.log");
    let doc = open(cx, Source::File(later.clone()));
    let message = failure(&doc, cx).expect("missing file fails");
    assert!(message.contains("No such file"), "{message}");
    std::fs::write(&later, "back").unwrap();
    doc.update(cx, |d, cx| d.retry(cx));
    assert_eq!(lines(&doc, cx), Some(1));
}

#[gpui_kit::test]
fn cancelling_a_background_open_fails_with_a_retry(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let path = common::write_log(dir.path(), "big.log", &["x"; 1000]);
    let doc = open(cx, Source::File(path));
    doc.update(cx, |d, cx| d.cancel_load(cx));
    assert_eq!(failure(&doc, cx).as_deref(), Some("Opening cancelled"));
    doc.update(cx, |d, cx| d.retry(cx));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
}

#[gpui_kit::test]
fn memory_sources_load_immediately(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let bytes = std::sync::Arc::new(b"a\nb".to_vec());
    let doc = cx.new(|cx| Document::new(Source::Memory { name: "mem".into(), bytes }, DocumentInit { number_base: 100 }, cx));
    assert_eq!(lines(&doc, cx), Some(2));
    assert_eq!(doc.read_with(cx, |d, _| (d.title().to_string(), d.number_base())), ("mem".to_string(), 100));
}
```

Add `pub mod document;` to `crates/rogg-app/src/lib.rs` (after `pub mod args;`) and create an empty `crates/rogg-app/src/document/mod.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test document`
Expected: compile errors — `Document`, `DocumentInit`, `EngineConfig`, `LoadState`, `Source` not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/document/load.rs`:

```rust
//! Opening a large file on a background thread; the document polls the channel.

use rogg_core::text::{LogText, OpenProgress};
use rogg_core::{CancelToken, CoreError};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum Message {
    Progress(OpenProgress),
    Done(Result<LogText, CoreError>),
}

pub(crate) enum OpenPoll {
    /// Still working; the latest progress since the last poll, if any.
    Pending(Option<OpenProgress>),
    Done(Result<LogText, CoreError>),
    /// The thread ended without a result (it panicked or could not be spawned).
    Died,
}

pub(crate) struct OpenJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
}

impl OpenJob {
    pub(crate) fn spawn(path: PathBuf, cache: PathBuf) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        // Detached: on cancel, `LogText::open` returns at its next check and the thread ends.
        let _detached = std::thread::Builder::new().name("rogg-open".into()).spawn(move || {
            let progress = tx.clone();
            let result = LogText::open(&path, &cache, &token, |p| {
                let _ = progress.send(Message::Progress(p));
            });
            let _ = tx.send(Message::Done(result));
        });
        Self { cancel, rx }
    }

    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Drains the channel: the final result, or the latest progress.
    pub(crate) fn poll(&self) -> OpenPoll {
        let mut last = None;
        loop {
            match self.rx.try_recv() {
                Ok(Message::Progress(p)) => last = Some(p),
                Ok(Message::Done(result)) => return OpenPoll::Done(result),
                Err(TryRecvError::Empty) => return OpenPoll::Pending(last),
                Err(TryRecvError::Disconnected) => return OpenPoll::Died,
            }
        }
    }
}

impl Drop for OpenJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
```

Replace `crates/rogg-app/src/document/mod.rs` with:

```rust
//! One open log: its source, its text once loaded, and the background jobs working on it. Inputs up to
//! `EngineConfig::sync_bytes` are handled on the UI thread; larger ones on background threads whose channels
//! one task polls every `POLL_INTERVAL` (spec §3.3).

mod load;

use crate::paths::AppPaths;
use gpui_kit::{App, Context, Global, SharedString};
use load::{OpenJob, OpenPoll};
use rogg_core::text::{LogText, OpenProgress};
use rogg_core::{CancelToken, CoreError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// How often background job channels are polled: at most one UI update per frame.
pub const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// Inputs at or below `sync_bytes` are opened, filtered and searched synchronously.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    pub sync_bytes: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { sync_bytes: 4 << 20 }
    }
}

impl Global for EngineConfig {}

impl EngineConfig {
    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf),
    /// In-memory text (the demo and tests); `name` is the tab title.
    Memory { name: SharedString, bytes: Arc<Vec<u8>> },
}

impl Source {
    pub fn title(&self) -> SharedString {
        match self {
            Source::File(path) => path
                .file_name()
                .map(|n| SharedString::from(n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| SharedString::from(path.display().to_string())),
            Source::Memory { name, .. } => name.clone(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Source::File(path) => Some(path),
            Source::Memory { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DocumentInit {
    /// Display number of the first line (1 for files; the demo uses the reference's 120480).
    pub number_base: u64,
}

impl Default for DocumentInit {
    fn default() -> Self {
        Self { number_base: 1 }
    }
}

#[derive(Clone)]
pub enum LoadState {
    Loading { fraction: f32, label: SharedString },
    Ready(Arc<LogText>),
    Failed { message: SharedString },
}

pub struct Document {
    pub source: Source,
    pub load: LoadState,
    number_base: u64,
    text_revision: u64,
    open_job: Option<OpenJob>,
    polling: bool,
}

fn loading(progress: OpenProgress) -> LoadState {
    let (label, done, total) = match progress {
        OpenProgress::Transcoding { done, total } => ("Converting UTF-16 to UTF-8", done, total),
        OpenProgress::Indexing { done, total } => ("Indexing", done, total),
    };
    LoadState::Loading { fraction: if total == 0 { 0. } else { done as f32 / total as f32 }, label: label.into() }
}

impl Document {
    pub fn new(source: Source, init: DocumentInit, cx: &mut Context<Self>) -> Self {
        let mut doc = Self {
            source,
            load: LoadState::Loading { fraction: 0., label: "Opening".into() },
            number_base: init.number_base,
            text_revision: 0,
            open_job: None,
            polling: false,
        };
        doc.start_load(cx);
        doc
    }

    pub fn title(&self) -> SharedString {
        self.source.title()
    }

    pub fn number_base(&self) -> u64 {
        self.number_base
    }

    pub fn text(&self) -> Option<&Arc<LogText>> {
        match &self.load {
            LoadState::Ready(text) => Some(text),
            _ => None,
        }
    }

    /// Bumped every time a text finishes loading; tabs rebuild their rows when it changes.
    pub fn text_revision(&self) -> u64 {
        self.text_revision
    }

    pub fn retry(&mut self, cx: &mut Context<Self>) {
        if matches!(self.load, LoadState::Failed { .. }) {
            self.start_load(cx);
        }
    }

    pub fn cancel_load(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = self.open_job.take() {
            job.cancel();
            self.load = LoadState::Failed { message: "Opening cancelled".into() };
            cx.notify();
        }
    }

    fn start_load(&mut self, cx: &mut Context<Self>) {
        self.load = LoadState::Loading { fraction: 0., label: "Opening".into() };
        match self.source.clone() {
            Source::Memory { bytes, .. } => self.finish_load(Ok(LogText::from_bytes(bytes.to_vec())), cx),
            Source::File(path) => {
                let cache = cx.try_global::<AppPaths>().map(|p| p.cache.clone()).unwrap_or_else(rogg_core::paths::cache_dir);
                match std::fs::metadata(&path) {
                    Err(source) => self.finish_load(Err(CoreError::Io { path, source }), cx),
                    Ok(meta) if meta.len() <= EngineConfig::get(cx).sync_bytes => {
                        let result = LogText::open(&path, &cache, &CancelToken::new(), |_| {});
                        self.finish_load(result, cx);
                    }
                    Ok(_) => {
                        self.open_job = Some(OpenJob::spawn(path, cache));
                        cx.notify();
                        self.ensure_polling(cx);
                    }
                }
            }
        }
    }

    fn finish_load(&mut self, result: Result<LogText, CoreError>, cx: &mut Context<Self>) {
        self.load = match result {
            Ok(text) => {
                self.text_revision += 1;
                LoadState::Ready(Arc::new(text))
            }
            Err(CoreError::Cancelled) => LoadState::Failed { message: "Opening cancelled".into() },
            // The error state shows the path on its own line, so the title is just the OS error.
            Err(CoreError::Io { source, .. }) => LoadState::Failed { message: source.to_string().into() },
            Err(e) => LoadState::Failed { message: e.to_string().into() },
        };
        cx.notify();
    }

    /// Starts the poll task unless it is already running. It stops by itself once no job is left.
    pub(crate) fn ensure_polling(&mut self, cx: &mut Context<Self>) {
        if self.polling {
            return;
        }
        self.polling = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                match this.update(cx, |doc, cx| doc.poll_jobs(cx)) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// Drains every job channel once; returns whether any job is still running.
    fn poll_jobs(&mut self, cx: &mut Context<Self>) -> bool {
        let active = self.poll_open(cx);
        if !active {
            self.polling = false;
        }
        active
    }

    fn poll_open(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(job) = &self.open_job else { return false };
        match job.poll() {
            OpenPoll::Pending(Some(progress)) => {
                self.load = loading(progress);
                cx.notify();
                true
            }
            OpenPoll::Pending(None) => true,
            OpenPoll::Done(result) => {
                self.open_job = None;
                self.finish_load(result, cx);
                false
            }
            OpenPoll::Died => {
                self.open_job = None;
                self.load = LoadState::Failed { message: "Opening stopped unexpectedly".into() };
                cx.notify();
                false
            }
        }
    }
}
```

In `crates/rogg-app/src/lib.rs`, make `install_globals` set the engine configuration (keep the rest):

```rust
pub fn install_globals(cx: &mut App, paths: AppPaths) {
    cx.set_global(document::EngineConfig::default());
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): document loading with background open, progress, cancel and retry

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Filter pipeline — spec changes, scans, progressive results, pins and context, history

**Files:**
- Create: `crates/rogg-app/src/document/filter.rs`, `crates/rogg-app/src/history.rs`, `crates/rogg-app/tests/filter.rs`
- Modify: `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes: `FilterSpec`, `Term`, `TermState`, `CompiledFilter`, `FilterJob`, `FilterEvent`, `run_filter`, `DEFAULT_CHUNK_LINES`, `FilteredView`, `Highlighter`, `FilterStats::from_parts` (M1 + Task 1); `History`, `HistoryEntry` (M1); Task 7 `Document`.
- Produces (used by Tasks 9–14):
  - `DocumentInit { number_base, spec: FilterSpec, pins: BTreeSet<u64> }`.
  - `Document.filter: FilterState` with pub fields `spec`, `error: Option<SharedString>`, `highlighter: Option<Arc<Highlighter>>`, `matches: Arc<Vec<u64>>`, `pins: BTreeSet<u64>` (0-based), `view: Arc<FilteredView>`, `stats: Option<FilterStats>`, `progress: Option<f32>`, `scans: u64`, `revision: u64`.
  - `Document::{apply_spec(FilterSpec, cx) -> Result<(), SharedString>, set_expression(&str, cx) -> Result<(), SharedString>, cycle_term(usize, cx), toggle_regex(cx), toggle_case(cx), set_context(u32, cx), add_term(&str, exclude: bool, cx), clear_filter_error(cx), toggle_pin(line: u64, cx)}`.
  - `rogg_app::history::HistoryStore` (gpui `Global`): `load(PathBuf) -> Self`, `entries() -> &[HistoryEntry]`, `record(&FilterSpec)`, pub `recovered: bool`, `last_error: Option<String>`. Installed by `install_globals`.
  - Behaviour: terms/flags changes rescan; context-only changes and pin toggles rebuild the view without a scan; an invalid spec is rejected (`Err(message)`, `filter.error` set, previous spec and results kept); a successful active spec is recorded in history; texts up to `sync_bytes` scan synchronously, larger ones run a `FilterJob` whose chunks are republished at most every 100 ms; a superseded or dropped job is cancelled and joined on a short-lived thread; a job channel that ends without `Finished` shows `Filtering stopped unexpectedly; showing partial results`.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/filter.rs`:

```rust
mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, Source};
use rogg_app::history::HistoryStore;
use rogg_core::filter::{FilterSpec, TermState};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn doc_from(cx: &mut TestAppContext, text: String, init: DocumentInit) -> Entity<Document> {
    let bytes = Arc::new(text.into_bytes());
    cx.new(|cx| Document::new(Source::Memory { name: "mem.log".into(), bytes }, init, cx))
}

fn small_doc(cx: &mut TestAppContext) -> Entity<Document> {
    doc_from(cx, LINES.join("\n"), DocumentInit::default())
}

fn matches(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone())
}

fn shown(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.view.shown_lines().as_ref().clone())
}

#[gpui_kit::test]
fn expression_states_and_flags_filter_the_text(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    assert_eq!(matches(&doc, cx), [1, 3, 5, 6]);
    doc.update(cx, |d, cx| d.set_expression("error || timeout || healthcheck", cx)).unwrap();
    doc.update(cx, |d, cx| {
        d.cycle_term(2, cx); // Include → Off
        d.cycle_term(2, cx); // Off → Exclude
    });
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[2].state), TermState::Exclude);
    assert_eq!(matches(&doc, cx), [1, 3, 5]);
    doc.update(cx, |d, cx| d.toggle_case(cx)); // "error" no longer matches "ERROR"
    assert_eq!(matches(&doc, cx), [3, 5]);
    let stats = doc.read_with(cx, |d, _| d.filter.stats.clone().unwrap());
    assert_eq!((stats.matched, stats.total_lines), (2, 8));
}

#[gpui_kit::test]
fn no_active_terms_match_nothing(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    assert!(matches(&doc, cx).is_empty());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.cycle_term(0, cx)); // Off
    assert!(matches(&doc, cx).is_empty());
}

#[gpui_kit::test]
fn pins_and_context_recompose_without_rescanning(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("disk", cx)).unwrap();
    let scans = doc.read_with(cx, |d, _| d.filter.scans);
    doc.update(cx, |d, cx| d.toggle_pin(7, cx));
    assert_eq!(shown(&doc, cx), [1, 7]);
    doc.update(cx, |d, cx| d.set_context(1, cx));
    assert_eq!(shown(&doc, cx), [0, 1, 2, 6, 7]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().context_lines), 3);
    doc.update(cx, |d, cx| d.toggle_pin(7, cx));
    assert_eq!(shown(&doc, cx), [0, 1, 2]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.scans), scans, "pins and context never rescan");
}

#[gpui_kit::test]
fn invalid_regex_is_rejected_and_previous_results_stay(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.toggle_regex(cx));
    let err = doc.update(cx, |d, cx| d.set_expression("error || (", cx)).unwrap_err();
    assert!(err.contains("invalid regex"), "{err}");
    assert_eq!(matches(&doc, cx), [1, 5, 6]);
    doc.read_with(cx, |d, _| {
        assert_eq!(d.filter.spec.expression(), "error");
        assert_eq!(d.filter.error.as_ref(), Some(&err));
    });
    doc.update(cx, |d, cx| d.clear_filter_error(cx));
    assert!(doc.read_with(cx, |d, _| d.filter.error.is_none()));
}

#[gpui_kit::test]
fn add_term_appends_or_restates(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.add_term("timeout", false, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "error || timeout");
    assert_eq!(matches(&doc, cx), [1, 3, 5, 6]);
    doc.update(cx, |d, cx| d.add_term("error", true, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[0].state), TermState::Exclude);
    assert_eq!(matches(&doc, cx), [3]);
}

#[gpui_kit::test]
fn applied_filters_are_recorded_in_history(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_context(2, cx));
    cx.update(|cx| {
        let store = cx.global::<HistoryStore>();
        assert_eq!(store.entries().len(), 1, "a context-only change is not a new entry");
        assert_eq!(store.entries()[0].expression(), "error || timeout");
    });
    assert!(dir.path().join("history.json").exists());
}

#[gpui_kit::test]
fn initial_spec_and_pins_apply_when_the_text_loads(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let init = DocumentInit { number_base: 1, spec: FilterSpec::from_expression("timeout"), pins: BTreeSet::from([0]) };
    let doc = doc_from(cx, LINES.join("\n"), init);
    assert_eq!(matches(&doc, cx), [3, 5]);
    assert_eq!(shown(&doc, cx), [0, 3, 5]);
}

fn numbered(n: usize, every: usize, word: &str) -> (String, Vec<u64>) {
    let mut text = String::new();
    let mut expected = Vec::new();
    for i in 0..n {
        if i % every == 0 {
            text.push_str(word);
            expected.push(i as u64);
        } else {
            text.push_str("filler line");
        }
        text.push('\n');
    }
    (text, expected)
}

#[gpui_kit::test]
fn background_scan_matches_the_synchronous_result(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let (text, expected) = numbered(20_000, 7, "needle here");
    let doc = doc_from(cx, text, DocumentInit { spec: FilterSpec::from_expression("needle"), ..Default::default() });
    assert!(doc.read_with(cx, |d, _| d.filter.progress.is_some()), "large texts scan in the background");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    assert_eq!(matches(&doc, cx), expected);
}

#[gpui_kit::test]
fn superseded_async_scan_never_leaks_results(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("beta", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    let found = matches(&doc, cx);
    assert_eq!(found.len(), 100_000);
    assert!(found.iter().all(|l| l % 2 == 1), "only the newest scan's matches appear");
}

#[gpui_kit::test]
fn dropping_a_document_mid_scan_does_not_block(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..2_000_000).map(|i| if i % 3 == 0 { "x marks\n" } else { "nothing\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("marks", cx)).unwrap();
    let started = Instant::now();
    drop(doc);
    cx.run_until_parked();
    assert!(started.elapsed() < Duration::from_millis(50), "closing waited {:?} for the scan", started.elapsed());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test filter`
Expected: compile errors — `Document::filter`, `set_expression`, `HistoryStore`, `DocumentInit::spec` not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/history.rs`:

```rust
//! Global filter history (spec §4.12): loaded at start, saved after each recorded filter. Saves run on the UI
//! thread one at a time, so the history file's single `.partial` name is never shared.

use gpui_kit::Global;
use rogg_core::filter::FilterSpec;
use rogg_core::history::{History, HistoryEntry};
use std::path::PathBuf;

pub struct HistoryStore {
    history: History,
    path: PathBuf,
    /// The file was unreadable and was moved to `history.json.bak` (shown as a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for HistoryStore {}

impl HistoryStore {
    pub fn load(path: PathBuf) -> Self {
        match History::load(&path) {
            Ok(outcome) => Self { history: outcome.history, path, recovered: outcome.recovered, last_error: None },
            Err(e) => Self { history: History::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        self.history.entries()
    }

    pub fn record(&mut self, spec: &FilterSpec) {
        self.history.record(HistoryEntry::from_spec(spec));
        self.last_error = self.history.save(&self.path).err().map(|e| e.to_string());
    }
}
```

Create `crates/rogg-app/src/document/filter.rs`:

```rust
//! A document's filter pipeline (spec §4.6–4.8, §4.11): compiles the spec, scans (synchronously for small
//! texts, as a `FilterJob` otherwise), republishes progressive results, and composes the filtered view from
//! matches, pins and context without rescanning.

use super::{Document, EngineConfig};
use crate::history::HistoryStore;
use gpui_kit::{Context, SharedString};
use rogg_core::CancelToken;
use rogg_core::engine::{DEFAULT_CHUNK_LINES, FilterEvent, FilterJob, run_filter};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::highlight::Highlighter;
use rogg_core::stats::FilterStats;
use rogg_core::view::FilteredView;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

/// Partial results of a background scan are republished at most this often.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(100);

pub struct FilterState {
    /// The applied filter: what the chips, flags and context select show.
    pub spec: FilterSpec,
    /// Why the last change was rejected (shown as `Previous results retained · …`).
    pub error: Option<SharedString>,
    compiled: Option<Arc<CompiledFilter>>,
    pub highlighter: Option<Arc<Highlighter>>,
    job: Option<FilterJob>,
    partial: Vec<u64>,
    last_publish: Option<Instant>,
    /// Matches of the latest scan (partial while `progress` is Some).
    pub matches: Arc<Vec<u64>>,
    term_hits: Vec<u64>,
    duration: Duration,
    /// Pinned lines, 0-based.
    pub pins: BTreeSet<u64>,
    pub view: Arc<FilteredView>,
    pub stats: Option<FilterStats>,
    /// Fraction of the running background scan; None when no scan runs.
    pub progress: Option<f32>,
    /// Scans started so far (tests check that pins and context never rescan).
    pub scans: u64,
    /// Bumped whenever `view`, `matches` or `pins` change.
    pub revision: u64,
}

fn empty_view() -> FilteredView {
    FilteredView::build(Arc::new(Vec::new()), &BTreeSet::new(), 0, 0, |_| false)
}

impl FilterState {
    pub(super) fn new(spec: FilterSpec, pins: BTreeSet<u64>) -> Self {
        let (compiled, highlighter, error) = match (CompiledFilter::compile(&spec), Highlighter::new(&spec)) {
            (Ok(c), Ok(h)) => (Some(Arc::new(c)), Some(Arc::new(h)), None),
            (Err(e), _) | (_, Err(e)) => (None, None, Some(SharedString::from(e.to_string()))),
        };
        Self {
            spec,
            error,
            compiled,
            highlighter,
            job: None,
            partial: Vec::new(),
            last_publish: None,
            matches: Arc::new(Vec::new()),
            term_hits: Vec::new(),
            duration: Duration::ZERO,
            pins,
            view: Arc::new(empty_view()),
            stats: None,
            progress: None,
            scans: 0,
            revision: 0,
        }
    }
}

/// Cancels `job` and joins it on a short-lived thread: the UI thread never waits for a scan to wind down, and a
/// `FilterJob` is never dropped on a rayon thread (M1 contract).
pub(super) fn retire(job: FilterJob) {
    job.cancel();
    let _ = std::thread::Builder::new().name("rogg-filter-retire".into()).spawn(move || drop(job));
}

impl Document {
    /// Applies `spec`. Changed terms or flags rescan and record history; a context-only change rebuilds the view.
    /// An invalid spec is rejected: the previous filter and results stay and `filter.error` is set.
    pub fn apply_spec(&mut self, spec: FilterSpec, cx: &mut Context<Self>) -> Result<(), SharedString> {
        let old = &self.filter.spec;
        let rescan = self.filter.compiled.is_none() || spec.terms != old.terms || spec.regex != old.regex || spec.case_sensitive != old.case_sensitive;
        if !rescan {
            self.filter.spec = spec;
            self.filter.error = None;
            self.rebuild_view(cx);
            return Ok(());
        }
        match (CompiledFilter::compile(&spec), Highlighter::new(&spec)) {
            (Ok(compiled), Ok(highlighter)) => {
                if spec.is_active() && cx.has_global::<HistoryStore>() {
                    cx.update_global::<HistoryStore, _>(|history, _| history.record(&spec));
                }
                self.filter.error = None;
                self.filter.spec = spec;
                self.filter.compiled = Some(Arc::new(compiled));
                self.filter.highlighter = Some(Arc::new(highlighter));
                self.start_filter(cx);
                Ok(())
            }
            (Err(e), _) | (_, Err(e)) => {
                let message = SharedString::from(e.to_string());
                self.filter.error = Some(message.clone());
                cx.notify();
                Err(message)
            }
        }
    }

    /// Re-parses an edited expression; terms whose text is unchanged keep their state (spec §4.6).
    pub fn set_expression(&mut self, text: &str, cx: &mut Context<Self>) -> Result<(), SharedString> {
        let spec = self.filter.spec.with_expression(text);
        self.apply_spec(spec, cx)
    }

    pub fn cycle_term(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut spec = self.filter.spec.clone();
        spec.cycle(index);
        let _ = self.apply_spec(spec, cx);
    }

    pub fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        let mut spec = self.filter.spec.clone();
        spec.regex = !spec.regex;
        let _ = self.apply_spec(spec, cx);
    }

    pub fn toggle_case(&mut self, cx: &mut Context<Self>) {
        let mut spec = self.filter.spec.clone();
        spec.case_sensitive = !spec.case_sensitive;
        let _ = self.apply_spec(spec, cx);
    }

    pub fn set_context(&mut self, context: u32, cx: &mut Context<Self>) {
        let mut spec = self.filter.spec.clone();
        spec.set_context(context);
        let _ = self.apply_spec(spec, cx);
    }

    /// Context menu "Filter on …" / "Exclude …": sets the state of the term with this text, or appends one.
    pub fn add_term(&mut self, text: &str, exclude: bool, cx: &mut Context<Self>) {
        let state = if exclude { TermState::Exclude } else { TermState::Include };
        let mut spec = self.filter.spec.clone();
        match spec.terms.iter_mut().find(|t| t.text == text) {
            Some(term) => term.state = state,
            None => spec.terms.push(Term::new(text, state)),
        }
        let _ = self.apply_spec(spec, cx);
    }

    pub fn clear_filter_error(&mut self, cx: &mut Context<Self>) {
        if self.filter.error.take().is_some() {
            cx.notify();
        }
    }

    /// Pins or unpins 0-based `line`; recomposes the view without rescanning.
    pub fn toggle_pin(&mut self, line: u64, cx: &mut Context<Self>) {
        if !self.filter.pins.remove(&line) {
            self.filter.pins.insert(line);
        }
        self.rebuild_view(cx);
    }

    /// Scans the loaded text with the compiled filter, replacing any running scan.
    pub(super) fn start_filter(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.filter.job.take() {
            retire(old);
        }
        self.filter.partial.clear();
        self.filter.last_publish = None;
        self.filter.term_hits.clear();
        self.filter.duration = Duration::ZERO;
        self.filter.progress = None;
        let (Some(text), Some(filter)) = (self.text().cloned(), self.filter.compiled.clone()) else {
            self.filter.matches = Arc::new(Vec::new());
            self.rebuild_view(cx);
            return;
        };
        if !filter.is_active() {
            self.filter.matches = Arc::new(Vec::new());
            self.rebuild_view(cx);
            return;
        }
        self.filter.scans += 1;
        if text.bytes().len() as u64 <= EngineConfig::get(cx).sync_bytes {
            match run_filter(&text, &filter, DEFAULT_CHUNK_LINES, &CancelToken::new()) {
                Ok(result) => self.finish_scan(result.matches, result.term_hits, result.duration, cx),
                Err(e) => {
                    self.filter.error = Some(e.to_string().into());
                    self.filter.matches = Arc::new(Vec::new());
                    self.rebuild_view(cx);
                }
            }
        } else {
            self.filter.matches = Arc::new(Vec::new());
            self.filter.progress = Some(0.);
            self.filter.job = Some(FilterJob::spawn(text, filter, DEFAULT_CHUNK_LINES));
            self.rebuild_view(cx);
            self.ensure_polling(cx);
        }
    }

    /// Drains the running scan's events; returns whether it is still running.
    pub(super) fn poll_filter(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(job) = self.filter.job.as_ref() else { return false };
        let mut finished = None;
        let mut died = false;
        let mut grew = false;
        loop {
            match job.events().try_recv() {
                Ok(FilterEvent::Chunk { matches, .. }) => {
                    grew |= !matches.is_empty();
                    self.filter.partial.extend(matches);
                }
                Ok(FilterEvent::Progress { lines_done, total_lines }) => {
                    self.filter.progress = Some(if total_lines == 0 { 1. } else { lines_done as f32 / total_lines as f32 });
                }
                Ok(FilterEvent::Finished { term_hits, duration, .. }) => {
                    finished = Some((term_hits, duration));
                    break;
                }
                // Only a cancelled job reports Cancelled, and a cancelled job is retired at once, so the current
                // job never yields it: treat it like a lost worker (M1 contract).
                Ok(FilterEvent::Cancelled) | Err(TryRecvError::Disconnected) => {
                    died = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if let Some((term_hits, duration)) = finished {
            // The worker has finished, so dropping the job joins at once.
            self.filter.job = None;
            let matches = std::mem::take(&mut self.filter.partial);
            self.finish_scan(matches, term_hits, duration, cx);
            return false;
        }
        if died {
            if let Some(job) = self.filter.job.take() {
                retire(job);
            }
            self.filter.progress = None;
            self.filter.error = Some("Filtering stopped unexpectedly; showing partial results".into());
            self.filter.matches = Arc::new(std::mem::take(&mut self.filter.partial));
            self.rebuild_view(cx);
            return false;
        }
        if grew && self.filter.last_publish.is_none_or(|t| t.elapsed() >= PUBLISH_INTERVAL) {
            self.filter.last_publish = Some(Instant::now());
            self.filter.matches = Arc::new(self.filter.partial.clone());
            self.rebuild_view(cx);
        } else {
            cx.notify();
        }
        true
    }

    fn finish_scan(&mut self, matches: Vec<u64>, term_hits: Vec<u64>, duration: Duration, cx: &mut Context<Self>) {
        self.filter.matches = Arc::new(matches);
        self.filter.term_hits = term_hits;
        self.filter.duration = duration;
        self.filter.progress = None;
        self.filter.last_publish = None;
        self.rebuild_view(cx);
    }

    /// Recomposes the filtered view and statistics from the current matches, pins and context.
    pub(super) fn rebuild_view(&mut self, cx: &mut Context<Self>) {
        let total = self.text().map_or(0, |t| t.line_count());
        let f = &self.filter;
        let view = match (self.text(), &f.compiled) {
            (Some(text), Some(filter)) if f.spec.context > 0 => {
                FilteredView::build(f.matches.clone(), &f.pins, f.spec.context, total, |n| filter.is_excluded(text.line(n)))
            }
            _ => FilteredView::build(f.matches.clone(), &f.pins, f.spec.context, total, |_| false),
        };
        let stats = FilterStats::from_parts(&f.spec, f.matches.len() as u64, &f.term_hits, total, f.duration, &view);
        self.filter.view = Arc::new(view);
        self.filter.stats = Some(stats);
        self.filter.revision += 1;
        cx.notify();
    }
}
```

In `crates/rogg-app/src/document/mod.rs`:
- add `mod filter;` next to `mod load;` and `pub use filter::FilterState;`; add `use rogg_core::filter::FilterSpec;` and `use std::collections::BTreeSet;`;
- extend `DocumentInit` and its `Default`:

```rust
#[derive(Clone, Debug)]
pub struct DocumentInit {
    /// Display number of the first line (1 for files; the demo uses the reference's 120480).
    pub number_base: u64,
    /// The filter to apply once the text loads.
    pub spec: FilterSpec,
    /// Pinned lines, 0-based.
    pub pins: BTreeSet<u64>,
}

impl Default for DocumentInit {
    fn default() -> Self {
        Self { number_base: 1, spec: FilterSpec::default(), pins: BTreeSet::new() }
    }
}
```

- add the field `pub filter: FilterState,` to `Document` (after `load`), and in `Document::new` initialise it with `filter: FilterState::new(init.spec, init.pins),` (build the struct before `start_load`, as now);
- in `finish_load`, start the filter when the text is ready — replace the `cx.notify();` at its end with:

```rust
        if matches!(self.load, LoadState::Ready(_)) {
            self.start_filter(cx);
        }
        cx.notify();
```

- poll the filter job too — replace `poll_jobs` with:

```rust
    fn poll_jobs(&mut self, cx: &mut Context<Self>) -> bool {
        let active = self.poll_open(cx) | self.poll_filter(cx);
        if !active {
            self.polling = false;
        }
        active
    }
```

- add at the end of the file:

```rust
impl Drop for Document {
    fn drop(&mut self) {
        if let Some(job) = self.filter.job.take() {
            filter::retire(job);
        }
    }
}
```

  (`job` is private to `filter.rs`: make the field `pub(super) job: Option<FilterJob>` there).

In `crates/rogg-app/src/lib.rs`, add `pub mod history;` and install the store in `install_globals`:

```rust
pub fn install_globals(cx: &mut App, paths: AppPaths) {
    cx.set_global(document::EngineConfig::default());
    cx.set_global(history::HistoryStore::load(paths.history.clone()));
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
}
```

Update `crates/rogg-app/tests/document.rs` for the new `DocumentInit` fields: replace `DocumentInit { number_base: 100 }` with `DocumentInit { number_base: 100, ..Default::default() }`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass (run `--test filter` three times to check the background tests are stable), no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): filter pipeline with background scans, progressive results, pins, context and history

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 9: Row sources — the engine as log view rows, with built-in decoration

**Files:**
- Create: `crates/rogg-app/src/rows.rs`, `crates/rogg-app/src/decorate.rs`
- Modify: `crates/rogg-app/src/lib.rs`
- Test: unit tests in `rows.rs` and `decorate.rs`

**Interfaces:**
- Consumes: `LogText`, `FilteredView`, `ViewRow`, `Highlighter`, `CompiledSearch` (M1); `LogRowSource` with Task 2's `number_at`/`nearest_row`/`copy_text`; `MAX_DISPLAY_BYTES`; Task 8 `Document.filter`.
- Produces (used by Tasks 10, 11):
  - `rogg_app::decorate::{timestamp_len(&str) -> usize, is_error(&str) -> bool}` — until highlighting profiles (M4) a leading timestamp renders faint and an `ERROR` level tints the row, exactly what the reference shows.
  - `rogg_app::rows::{display_line(&LogText, u64) -> SharedString, FullRows { text, base, pins: Arc<BTreeSet<u64>>, search: Option<Arc<CompiledSearch>> }, FilteredRows { text, base, view: Arc<FilteredView>, highlighter: Option<Arc<Highlighter>>, search: Option<Arc<CompiledSearch>> }}` implementing `LogRowSource` (display numbers = `base + line`; context rows dimmed; gap rows from the view; filter hits as `SpanStyle::Hit(term index % 4)` on the filtered pane only; search hits as `SpanStyle::SearchHit`).
  - `Document::{full_rows() -> Option<Rc<FullRows>>, filtered_rows() -> Option<Rc<FilteredRows>>}` (None until the text is loaded; `search` stays None until Task 10).

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/src/decorate.rs` with only its tests:

```rust
//! Built-in row decoration until highlighting profiles arrive (M4): a leading timestamp renders faint and a line
//! whose level is ERROR gets the error row tint, as in the reference.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_timestamps() {
        assert_eq!(timestamp_len("2026-10-05 14:32:08.142  ERROR  client       upstream timeout"), 23);
        assert_eq!(timestamp_len("2026-10-05T14:32:08Z start"), 19);
        assert_eq!(timestamp_len("2026-10-05 14:32:08,5 x"), 21);
        assert_eq!(timestamp_len("2026-10-05 14:32:08."), 19, "a dot without digits is not a fraction");
        assert_eq!(timestamp_len("14:32:08 no date"), 0);
        assert_eq!(timestamp_len(""), 0);
    }

    #[test]
    fn error_level_words() {
        assert!(is_error("2026-10-05 14:32:08.142  ERROR  client  x"));
        assert!(is_error("ERROR: boom"));
        assert!(!is_error("2026-10-05 14:32:08.142  INFO   server  error=UPSTREAM_UNAVAILABLE"));
        assert!(!is_error("ERRORS were few"));
        assert!(!is_error(&format!("{}ERROR", "x ".repeat(60))), "only the level column (first 96 bytes) counts");
    }
}
```

Create `crates/rogg-app/src/rows.rs` with only its tests:

```rust
//! The engine as `LogRowSource`s for the two panes. Rows are materialised on demand, for the visible range only;
//! each raw line is cut to the display limit before it is decoded (M1 carry-over).

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::filter::FilterSpec;
    use rogg_core::search::SearchQuery;
    use rogg_ui::logview::{LogRow, LogRowSource, RowTint, SpanStyle};

    fn text(lines: &[&str]) -> Arc<LogText> {
        Arc::new(LogText::from_bytes(lines.join("\n").into_bytes()))
    }

    fn line(row: LogRow) -> rogg_ui::logview::LogLine {
        match row {
            LogRow::Line(l) => l,
            LogRow::Gap { omitted } => panic!("expected a line, got a gap of {omitted}"),
        }
    }

    #[test]
    fn full_rows_number_pin_and_decorate() {
        let t = text(&["2026-10-05 14:32:08.142  ERROR  disk full", "plain"]);
        let rows = FullRows { text: t, base: 100, pins: Arc::new(BTreeSet::from([1])), search: None };
        assert_eq!(rows.row_count(), 2);
        let first = line(rows.row(0));
        assert_eq!(first.number, 100);
        assert_eq!(first.tint, Some(RowTint::Error));
        assert_eq!(first.spans[0].range, 0..23);
        assert_eq!(first.spans[0].style, SpanStyle::Faint);
        assert!(!first.pinned && !first.context);
        let second = line(rows.row(1));
        assert!(second.pinned && second.spans.is_empty() && second.tint.is_none());
        assert_eq!(rows.row_for_number(101), Some(1));
        assert_eq!(rows.row_for_number(99), None);
        assert_eq!(rows.number_at(1), Some(101));
        assert_eq!(rows.nearest_row(500), Some(1));
        assert_eq!(rows.copy_text(0).as_deref(), Some("2026-10-05 14:32:08.142  ERROR  disk full"));
    }

    #[test]
    fn filtered_rows_show_hits_context_and_gaps() {
        let t = text(&["a", "ERROR disk full", "b", "c", "d", "pinned line", "e", "f"]);
        let spec = FilterSpec::from_expression("disk");
        let view = FilteredView::build(Arc::new(vec![1]), &BTreeSet::from([5]), 0, 8, |_| false);
        let rows = FilteredRows { text: t.clone(), base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: None };
        // shown lines 1 and 5 with a gap of three between them
        assert_eq!(rows.row_count(), 3);
        let hit = line(rows.row(0));
        assert_eq!(hit.number, 2);
        assert!(hit.spans.iter().any(|s| s.range == (6..10) && s.style == SpanStyle::Hit(0)));
        assert_eq!(rows.row(1), LogRow::Gap { omitted: 3 });
        assert!(rows.is_gap(1) && !rows.is_gap(0));
        let pinned = line(rows.row(2));
        assert!(pinned.pinned && !pinned.context);
        assert_eq!(rows.row_for_number(6), Some(2));
        assert_eq!(rows.row_for_number(3), None, "hidden lines have no row");
        assert_eq!(rows.nearest_row(3), Some(2), "the next shown line");
        assert_eq!(rows.number_at(1), None);
        assert_eq!(rows.copy_text(2).as_deref(), Some("pinned line"));

        let ctx = FilteredView::build(Arc::new(vec![1]), &BTreeSet::new(), 1, 8, |_| false);
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(ctx), highlighter: None, search: None };
        assert!(line(rows.row(0)).context && !line(rows.row(1)).context && line(rows.row(2)).context);
    }

    #[test]
    fn search_hits_are_outlined() {
        let t = text(&["one timeout two timeout"]);
        let search = Arc::new(SearchQuery { text: "timeout".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FullRows { text: t, base: 1, pins: Arc::new(BTreeSet::new()), search: Some(search) };
        let ranges: Vec<_> = line(rows.row(0)).spans.into_iter().filter(|s| s.style == SpanStyle::SearchHit).map(|s| s.range).collect();
        assert_eq!(ranges, [4..11, 16..23]);
    }

    #[test]
    fn long_lines_are_truncated_for_display_but_copied_in_full() {
        let long = format!("{} needle", "a".repeat(70_000));
        let t = text(&[&long]);
        let display = display_line(&t, 0);
        assert_eq!(display.len(), MAX_DISPLAY_BYTES + 4, "only a little past the display limit is decoded");
        let spec = FilterSpec::from_expression("needle || a");
        let view = FilteredView::build(Arc::new(vec![0]), &BTreeSet::new(), 0, 1, |_| false);
        let search = Arc::new(SearchQuery { text: "aaa".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: Some(search) };
        let row = line(rows.row(0));
        assert!(row.spans.iter().all(|s| s.range.end <= row.text.len()), "spans stay inside the display text");
        assert_eq!(rows.copy_text(0).map(|s| s.len()), Some(long.len()));
    }
}
```

Add to `crates/rogg-app/src/lib.rs`: `pub mod decorate;` and `pub mod rows;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --lib`
Expected: compile errors — `timestamp_len`, `is_error`, `FullRows`, `FilteredRows`, `display_line` not found.

- [ ] **Step 3: Implement**

Put above the tests in `crates/rogg-app/src/decorate.rs`:

```rust
/// Byte length of a leading `YYYY-MM-DD HH:MM:SS` timestamp (`T` separator allowed) with an optional `.fff` or
/// `,fff` fraction of 1–9 digits; 0 when the line does not start with one.
pub fn timestamp_len(line: &str) -> usize {
    let b = line.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|s| s.iter().all(|c| c.is_ascii_digit()));
    let at = |i: usize, allowed: &[u8]| b.get(i).is_some_and(|c| allowed.contains(c));
    let stamp = digits(0..4)
        && at(4, b"-")
        && digits(5..7)
        && at(7, b"-")
        && digits(8..10)
        && at(10, b" T")
        && digits(11..13)
        && at(13, b":")
        && digits(14..16)
        && at(16, b":")
        && digits(17..19);
    if !stamp {
        return 0;
    }
    if at(19, b".,") {
        let fraction = b[20..].iter().take(9).take_while(|c| c.is_ascii_digit()).count();
        if fraction > 0 {
            return 20 + fraction;
        }
    }
    19
}

/// True when `ERROR` appears as a whole word within the first 96 bytes (the level column).
pub fn is_error(line: &str) -> bool {
    let head = &line.as_bytes()[..line.len().min(96)];
    head.windows(5).enumerate().any(|(i, w)| {
        w == b"ERROR" && (i == 0 || !head[i - 1].is_ascii_alphanumeric()) && head.get(i + 5).is_none_or(|c| !c.is_ascii_alphanumeric())
    })
}
```

Put above the tests in `crates/rogg-app/src/rows.rs`:

```rust
use crate::decorate;
use crate::document::Document;
use gpui_kit::SharedString;
use rogg_core::highlight::Highlighter;
use rogg_core::search::CompiledSearch;
use rogg_core::text::LogText;
use rogg_core::view::{FilteredView, ViewRow};
use rogg_ui::logview::text::MAX_DISPLAY_BYTES;
use rogg_ui::logview::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

/// Colour slots in the filter-hit palette (spec §5.1).
const HIT_SLOTS: usize = 4;

/// At most `MAX_DISPLAY_BYTES` + 4 bytes of line `i`, lossily decoded: enough for the log view to cut the line
/// at the display limit and append `…`, without decoding a multi-megabyte line.
pub fn display_line(text: &LogText, i: u64) -> SharedString {
    let raw = text.line(i);
    let cut = raw.len().min(MAX_DISPLAY_BYTES + 4);
    String::from_utf8_lossy(&raw[..cut]).into_owned().into()
}

fn decorated(display: SharedString, number: u64, pinned: bool, context: bool, hits: Vec<Span>, search: Option<&CompiledSearch>) -> LogLine {
    let mut spans = Vec::with_capacity(hits.len() + 1);
    let stamp = decorate::timestamp_len(&display);
    if stamp > 0 {
        spans.push(Span { range: 0..stamp, style: SpanStyle::Faint });
    }
    spans.extend(hits);
    if let Some(search) = search {
        spans.extend(search.spans(&display).into_iter().map(|range| Span { range, style: SpanStyle::SearchHit }));
    }
    let tint = decorate::is_error(&display).then_some(RowTint::Error);
    LogLine { number, text: display, spans, pinned, context, tint }
}

/// Every line of the text (the full log pane).
pub struct FullRows {
    pub text: Arc<LogText>,
    /// Display number of line 0.
    pub base: u64,
    pub pins: Arc<BTreeSet<u64>>,
    pub search: Option<Arc<CompiledSearch>>,
}

impl LogRowSource for FullRows {
    fn row_count(&self) -> usize {
        self.text.line_count() as usize
    }

    fn row(&self, ix: usize) -> LogRow {
        let i = ix as u64;
        LogRow::Line(decorated(display_line(&self.text, i), self.base + i, self.pins.contains(&i), false, Vec::new(), self.search.as_deref()))
    }

    fn is_gap(&self, _: usize) -> bool {
        false
    }

    fn longest_line_chars(&self) -> usize {
        self.text.index().longest_line() as usize
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        let i = number.checked_sub(self.base)?;
        (i < self.text.line_count()).then_some(i as usize)
    }

    fn number_at(&self, ix: usize) -> Option<u64> {
        ((ix as u64) < self.text.line_count()).then(|| self.base + ix as u64)
    }

    fn nearest_row(&self, number: u64) -> Option<usize> {
        let count = self.text.line_count();
        (count > 0).then(|| number.saturating_sub(self.base).min(count - 1) as usize)
    }

    fn copy_text(&self, ix: usize) -> Option<String> {
        ((ix as u64) < self.text.line_count()).then(|| self.text.line_text(ix as u64).into_owned())
    }
}

/// Matches, pins, context and gap rows (the filtered log pane).
pub struct FilteredRows {
    pub text: Arc<LogText>,
    pub base: u64,
    pub view: Arc<FilteredView>,
    pub highlighter: Option<Arc<Highlighter>>,
    pub search: Option<Arc<CompiledSearch>>,
}

impl LogRowSource for FilteredRows {
    fn row_count(&self) -> usize {
        self.view.row_count() as usize
    }

    fn row(&self, ix: usize) -> LogRow {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, matched, pinned }) => {
                let display = display_line(&self.text, number);
                let hits = self
                    .highlighter
                    .as_ref()
                    .map(|h| h.spans(&display, HIT_SLOTS).into_iter().map(|(range, slot)| Span { range, style: SpanStyle::Hit(slot) }).collect())
                    .unwrap_or_default();
                LogRow::Line(decorated(display, self.base + number, pinned, !matched && !pinned, hits, self.search.as_deref()))
            }
            Some(ViewRow::Gap { omitted }) => LogRow::Gap { omitted },
            None => LogRow::Gap { omitted: 0 },
        }
    }

    fn is_gap(&self, ix: usize) -> bool {
        matches!(self.view.row(ix as u64), Some(ViewRow::Gap { .. }))
    }

    fn longest_line_chars(&self) -> usize {
        self.text.index().longest_line() as usize
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        let line = number.checked_sub(self.base)?;
        self.view.row_for_line(line).map(|r| r as usize)
    }

    fn number_at(&self, ix: usize) -> Option<u64> {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, .. }) => Some(self.base + number),
            _ => None,
        }
    }

    fn nearest_row(&self, number: u64) -> Option<usize> {
        let shown = self.view.shown_lines();
        if shown.is_empty() {
            return None;
        }
        let line = number.saturating_sub(self.base);
        let i = shown.partition_point(|&l| l < line).min(shown.len() - 1);
        self.view.row_for_line(shown[i]).map(|r| r as usize)
    }

    fn copy_text(&self, ix: usize) -> Option<String> {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, .. }) => Some(self.text.line_text(number).into_owned()),
            _ => None,
        }
    }
}

impl Document {
    /// Rows for the full log pane; None until the text is loaded.
    pub fn full_rows(&self) -> Option<Rc<FullRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FullRows { text, base: self.number_base(), pins: Arc::new(self.filter.pins.clone()), search: None }))
    }

    /// Rows for the filtered log pane; None until the text is loaded.
    pub fn filtered_rows(&self) -> Option<Rc<FilteredRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FilteredRows {
            text,
            base: self.number_base(),
            view: self.filter.view.clone(),
            highlighter: self.filter.highlighter.clone(),
            search: None,
        }))
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): engine-backed rows for both panes with built-in decoration

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Per-pane search

**Files:**
- Create: `crates/rogg-app/src/document/search.rs`, `crates/rogg-app/tests/search.rs`
- Modify: `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/document/filter.rs`, `crates/rogg-app/src/rows.rs`

**Interfaces:**
- Consumes: `SearchQuery`, `CompiledSearch`, `SearchScope`, `SearchResult` (M1); Task 7–9 `Document`.
- Produces (used by Tasks 11, 13):
  - `rogg_app::document::{Pane::{Full, Filtered}, PaneSearch}`; `Pane::index() -> usize` (0 full, 1 filtered).
  - `PaneSearch` pub fields `query: String`, `compiled: Option<Arc<CompiledSearch>>`, `result: Option<SearchResult>`, `invalid: bool`, `revision: u64`; `PaneSearch::{is_running(), label() -> Option<SharedString>}` (`1,204 hits`, `1 hit`, `…` while counting, `invalid`, None without a query).
  - `Document::{search(Pane) -> &PaneSearch, set_search(Pane, &str, cx), next_hit(Pane, from_line: Option<u64>, backwards: bool) -> Option<u64>}` (0-based lines, wrapping).
  - Queries are plain and case-insensitive (the reference find field has no flags; spec §4.10 flags arrive with the find popover later). The full pane searches the whole text; the filtered pane searches the shown lines and recounts whenever the view changes. Inputs above `sync_bytes` search on a `rogg-search` thread. `FullRows`/`FilteredRows` now outline the hits of their pane's query.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/search.rs`:

```rust
mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, Pane, Source};
use std::sync::Arc;

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn doc(cx: &mut TestAppContext, text: String) -> Entity<Document> {
    let bytes = Arc::new(text.into_bytes());
    cx.new(|cx| Document::new(Source::Memory { name: "mem.log".into(), bytes }, DocumentInit::default(), cx))
}

fn label(doc: &Entity<Document>, pane: Pane, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| d.search(pane).label().map(|s| s.to_string()))
}

#[gpui_kit::test]
fn full_log_search_counts_and_steps_through_hits(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    assert_eq!(label(&doc, Pane::Full, cx), None);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "error", cx));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("3 hits"));
    doc.read_with(cx, |d, _| {
        assert_eq!(d.next_hit(Pane::Full, None, false), Some(1));
        assert_eq!(d.next_hit(Pane::Full, Some(1), false), Some(5));
        assert_eq!(d.next_hit(Pane::Full, Some(6), false), Some(1), "wraps forward");
        assert_eq!(d.next_hit(Pane::Full, Some(1), true), Some(6), "wraps backward");
    });
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "", cx));
    assert_eq!(label(&doc, Pane::Full, cx), None);
}

#[gpui_kit::test]
fn filtered_search_follows_the_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    doc.update(cx, |d, cx| d.set_expression("timeout", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_search(Pane::Filtered, "error", cx));
    assert_eq!(label(&doc, Pane::Filtered, cx).as_deref(), Some("1 hit"));
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    assert_eq!(label(&doc, Pane::Filtered, cx).as_deref(), Some("3 hits"), "recounted for the new view");
}

#[gpui_kit::test]
fn rows_outline_the_query(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "disk", cx));
    let rows = doc.read_with(cx, |d, _| d.full_rows().unwrap());
    use rogg_ui::logview::{LogRow, LogRowSource, SpanStyle};
    let LogRow::Line(line) = rows.row(1) else { panic!("line") };
    assert!(line.spans.iter().any(|s| s.style == SpanStyle::SearchHit && s.range == (6..10)));
}

#[gpui_kit::test]
fn large_texts_search_in_the_background(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..50_000).map(|i| if i % 10 == 0 { "find me\n" } else { "other\n" }).collect();
    let doc = doc(cx, text);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "find", cx));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("…"));
    common::wait_until(cx, |cx| label(&doc, Pane::Full, cx).as_deref() != Some("…"));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("5,000 hits"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test search`
Expected: compile errors — `Pane`, `search`, `set_search`, `next_hit` not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/document/search.rs`:

```rust
//! Per-pane search (spec §4.10): a plain, case-insensitive query over the whole text (full log) or the shown
//! lines (filtered log), counted on a background thread for large inputs.

use super::{Document, EngineConfig};
use gpui_kit::{Context, SharedString};
use rogg_core::engine::DEFAULT_CHUNK_LINES;
use rogg_core::search::{CompiledSearch, SearchQuery, SearchResult, SearchScope};
use rogg_core::stats::{group_digits, noun};
use rogg_core::text::LogText;
use rogg_core::{CancelToken, CoreError};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Full,
    Filtered,
}

impl Pane {
    pub fn index(self) -> usize {
        match self {
            Pane::Full => 0,
            Pane::Filtered => 1,
        }
    }
}

struct SearchJob {
    cancel: CancelToken,
    rx: Receiver<Result<SearchResult, CoreError>>,
}

impl SearchJob {
    fn spawn(text: Arc<LogText>, search: Arc<CompiledSearch>, scope: SearchScope) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        // Detached: a replaced job is cancelled and its thread ends at the next chunk.
        let _detached = std::thread::Builder::new().name("rogg-search".into()).spawn(move || {
            let _ = tx.send(search.run(&text, &scope, DEFAULT_CHUNK_LINES, &token));
        });
        Self { cancel, rx }
    }
}

impl Drop for SearchJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub struct PaneSearch {
    pub query: String,
    pub compiled: Option<Arc<CompiledSearch>>,
    pub result: Option<SearchResult>,
    pub invalid: bool,
    job: Option<SearchJob>,
    /// Bumped when the query or its scope changes; rows outline the new query when it moves.
    pub revision: u64,
}

impl PaneSearch {
    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    /// `1,204 hits`, `1 hit`, `…` while counting, `invalid` for a bad query; None without a query.
    pub fn label(&self) -> Option<SharedString> {
        if self.query.is_empty() {
            return None;
        }
        if self.invalid {
            return Some("invalid".into());
        }
        match &self.result {
            Some(result) => {
                let n = result.count() as u64;
                Some(format!("{} {}", group_digits(n), noun(n, "hit", "hits")).into())
            }
            None => Some("…".into()),
        }
    }
}

#[derive(Default)]
pub struct SearchState {
    panes: [PaneSearch; 2],
}

impl Document {
    pub fn search(&self, pane: Pane) -> &PaneSearch {
        &self.search.panes[pane.index()]
    }

    pub fn set_search(&mut self, pane: Pane, query: &str, cx: &mut Context<Self>) {
        let s = &mut self.search.panes[pane.index()];
        s.query = query.to_string();
        s.invalid = false;
        s.compiled = None;
        if !query.is_empty() {
            match (SearchQuery { text: query.to_string(), regex: false, case_sensitive: false }).compile() {
                Ok(compiled) => s.compiled = Some(Arc::new(compiled)),
                Err(_) => s.invalid = true,
            }
        }
        self.run_search(pane, cx);
    }

    /// Counts `pane`'s query against the current text (the filtered pane: the current shown lines).
    pub(super) fn run_search(&mut self, pane: Pane, cx: &mut Context<Self>) {
        let text = self.text().cloned();
        let scope = match pane {
            Pane::Full => SearchScope::All,
            Pane::Filtered => SearchScope::Lines(self.filter.view.shown_lines().clone()),
        };
        let sync_bytes = EngineConfig::get(cx).sync_bytes;
        let s = &mut self.search.panes[pane.index()];
        s.job = None;
        s.result = None;
        s.revision += 1;
        let mut spawned = false;
        if let (Some(text), Some(search)) = (text, s.compiled.clone()) {
            if text.bytes().len() as u64 <= sync_bytes {
                s.result = search.run(&text, &scope, DEFAULT_CHUNK_LINES, &CancelToken::new()).ok();
            } else {
                s.job = Some(SearchJob::spawn(text, search, scope));
                spawned = true;
            }
        }
        cx.notify();
        if spawned {
            self.ensure_polling(cx);
        }
    }

    /// Re-counts a pane whose query is set (after its scope changed).
    pub(super) fn refresh_search(&mut self, pane: Pane, cx: &mut Context<Self>) {
        if !self.search(pane).query.is_empty() {
            self.run_search(pane, cx);
        }
    }

    /// Collects finished counts; returns whether any search is still running.
    pub(super) fn poll_search(&mut self, cx: &mut Context<Self>) -> bool {
        let mut active = false;
        let mut changed = false;
        for s in &mut self.search.panes {
            let Some(job) = &s.job else { continue };
            match job.rx.try_recv() {
                Ok(result) => {
                    s.result = result.ok();
                    s.job = None;
                    changed = true;
                }
                Err(TryRecvError::Empty) => active = true,
                Err(TryRecvError::Disconnected) => {
                    s.job = None;
                    changed = true;
                }
            }
        }
        if changed {
            cx.notify();
        }
        active
    }

    /// The next (or previous) line with a hit after 0-based `from`, wrapping; the first hit when `from` is None.
    pub fn next_hit(&self, pane: Pane, from: Option<u64>, backwards: bool) -> Option<u64> {
        let result = self.search(pane).result.as_ref()?;
        if backwards { result.prev_before(from) } else { result.next_after(from) }
    }
}
```

In `crates/rogg-app/src/document/mod.rs`:
- add `mod search;` and `pub use search::{Pane, PaneSearch};`;
- add the field `search: search::SearchState,` to `Document` and `search: Default::default(),` in `new`;
- in `finish_load`, after `self.start_filter(cx);` inside the `if`, add `self.refresh_search(Pane::Full, cx);`;
- poll searches too: `let active = self.poll_open(cx) | self.poll_filter(cx) | self.poll_search(cx);`.

In `crates/rogg-app/src/document/filter.rs`, at the end of `rebuild_view` (before `cx.notify();`), recount the filtered pane's search for the new view:

```rust
        self.refresh_search(super::Pane::Filtered, cx);
```

In `crates/rogg-app/src/rows.rs`, use `crate::document::Pane` and fill the `search` fields in `Document::full_rows` / `filtered_rows`: `search: self.search(Pane::Full).compiled.clone()` and `search: self.search(Pane::Filtered).compiled.clone()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): per-pane search with background counting and outlined hits

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 11: Tabs in the window — panes, filter band, find, follow selection, pins, status bar

**Files:**
- Create: `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`, `crates/rogg-app/tests/tab.rs`
- Modify: `crates/rogg-app/src/workspace/mod.rs` (replace), `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 2–5 (`LogView::focus`, `LogViewEvent::{Selected, TogglePin, AddTerm}`, `ScrollTo`, `MainWindowLayout::{new, single, measure, sidebar_resizable}`, `LayoutMeasure`, `ToastStack`, `FilterBand::focused`); Tasks 7–10 (`Document`, `Source`, `Pane`, rows, search).
- Produces (used by Tasks 12–15):
  - `rogg_app::tab::{Tab, TabInit { doc: DocumentInit, selection: Option<u64>, labels: TabLabels }, TabLabels { crumbs, profile, note, full_count, tooltip }, TabEvent::{Toast(SharedString), ShowHistory}}`.
  - `Tab` pub fields `doc`, `full`, `filtered` (`Entity<LogViewState>`), `focus: [FocusHandle; 3]` (full log, filter line, filtered log — the Tab-cycle order), `find: [Entity<InputState>; 2]`, `editing: Option<Entity<InputState>>`, `follow: bool`, `split: f32`, `labels`, `last_pane: Pane`; methods `new(Source, TabInit, &mut Window, &mut Context<Tab>)`, `title(&App)`, `state(Pane)`, `selection()`, `select(Pane, number, center: bool, cx)`, `toggle_pin(number, cx)`, `pin_selected(cx)`, `reveal_selected(cx)`, `go_to_line(number, cx) -> bool`, `jump_to_hit(Pane, backwards, cx)`, `toggle_wrap(Pane, cx)`, `toggle_follow(cx)`, `start_edit(window, cx)`, `apply_edit(window, cx)`, `cancel_edit(window, cx)`.
  - `rogg_app::tab_view::{parts(&Entity<Tab>, &Window, &App) -> TabParts, TabParts { info_bar, body, status }, TabBody::{Panes { … split }, Placeholder(AnyElement)}, chip_state(TermState) -> ChipState}`.
  - `Workspace::{tabs(), active_index(), active_tab(), open_source(Source, TabInit, window, cx) -> Entity<Tab>, open_paths(Vec<PathBuf>, window, cx), activate(usize, window, cx), toast(impl Into<SharedString>, cx), dismiss_toast(u64, cx)}`; `Workspace::new` opens `args.paths` (canonicalised; an already open path activates its tab).
  - Behaviour (spec §5.6): selecting a filtered row selects and centres the line in the full log while Follow is on; selecting a full-log row selects the matching filtered row when it is shown (scrolling only if it is off-screen) and clears it otherwise; gutter clicks, ⌘B and the context menu pin in both panes; "Filter on/Exclude" adds terms; Enter/Shift-Enter in a find field selects the next/previous hit; Escape cancels editing; a tab that is loading or failed shows the loading (with Cancel) or error (with Retry) placeholder; the filtered pane shows `No matching lines` when nothing is shown; Profile, Export and Open as tab show a "later version" toast.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/tab.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, Entity, TestAppContext, point, px};
use rogg_app::args::Args;
use rogg_app::document::Pane;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn open_one(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &LINES);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().expect("a tab"));
    (dir, w, ws, tab)
}

fn selected(tab: &Entity<Tab>, pane: Pane, cx: &mut TestAppContext) -> Option<u64> {
    tab.read_with(cx, |t, cx| t.state(pane).read(cx).selected())
}

fn click_at(cx: &mut TestAppContext, w: AnyWindowHandle, id: &'static str, x: f32, y: f32) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at(id, point(px(x), px(y)), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_file_opens_into_both_panes_with_the_reference_chrome(cx: &mut TestAppContext) {
    let (_dir, w, _, _) = open_one(cx);
    for id in ["titlebar", "tab-shell-0", "meta", "pane-top-head", "log-top", "filter-band", "band-handle", "pane-bottom-head", "statusbar"] {
        assert!(common::exists(cx, w, id), "missing {id}");
    }
    assert!(common::exists(cx, w, "empty-results"), "no filter yet: the filtered pane shows the no-matches state");
}

#[gpui_kit::test]
fn filter_follow_selection_and_pins(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    assert!(common::exists(cx, w, "log-bottom"));
    // filtered rows: line 2, gap, line 4, gap, line 6, line 7 (display numbers)
    assert_eq!(tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count()), 6);
    click_at(cx, w, "log-bottom", 300., 4. + 10.);
    assert_eq!(selected(&tab, Pane::Filtered, cx), Some(2));
    assert_eq!(selected(&tab, Pane::Full, cx), Some(2), "follow selection mirrors into the full log");
    // full-log row 4 is line 5 ("ok"), which the filter hides
    click_at(cx, w, "log-top", 300., 4. + 4. * 23. + 10.);
    assert_eq!(selected(&tab, Pane::Full, cx), Some(5));
    assert_eq!(selected(&tab, Pane::Filtered, cx), None, "a hidden line clears the filtered selection");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(5));
    // the gutter pins it, and the filtered view shows it without a rescan
    let scans = doc.read_with(cx, |d, _| d.filter.scans);
    click_at(cx, w, "log-top", 10., 4. + 4. * 23. + 10.);
    assert!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().contains(&4)));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.scans), scans);
    tab.update(cx, |t, cx| t.toggle_follow(cx));
    click_at(cx, w, "log-bottom", 300., 4. + 10.);
    assert_eq!(selected(&tab, Pane::Full, cx), Some(5), "with follow off the full log keeps its selection");
}

#[gpui_kit::test]
fn find_counts_and_enter_steps_through_hits(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let input = tab.read_with(cx, |t, _| t.find[0].clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        input.update(cx, |i, cx| i.focus(window, cx));
        window.input("error", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Full).label().map(|s| s.to_string())).as_deref(), Some("3 hits"));
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(2));
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(6));
    common::press(cx, w, "shift-enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(2));
}

#[gpui_kit::test]
fn invalid_regex_keeps_results_and_editor(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| {
        d.set_expression("error", cx).unwrap();
        d.toggle_regex(cx);
    });
    cx.update_window(w, |_, window, cx| tab.update(cx, |t, cx| t.start_edit(window, cx))).unwrap();
    assert!(common::exists(cx, w, "expression"));
    let input = tab.read_with(cx, |t, _| t.editing.clone().expect("editing"));
    cx.update_window(w, |_, window, cx| input.update(cx, |i, cx| i.set_value("error || (", window, cx))).unwrap();
    common::click(cx, w, "apply");
    assert!(common::exists(cx, w, "filter-error"));
    assert!(common::exists(cx, w, "expression"), "the editor stays open");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone()), [1, 5, 6]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "error");
}

#[gpui_kit::test]
fn missing_file_shows_error_state_with_retry(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("later.log");
    let (w, _) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    assert!(common::exists(cx, w, "error"));
    assert!(!common::exists(cx, w, "log-top"));
    std::fs::write(&path, "now here").unwrap();
    common::click(cx, w, "retry");
    assert!(common::exists(cx, w, "log-top"));
}

#[gpui_kit::test]
fn not_yet_buttons_explain_themselves(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open_one(cx);
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::click(cx, w, "tool-open-tab");
    assert_eq!(ws.read_with(cx, |ws, _| ws.toasts.len()), 1);
    assert!(common::exists(cx, w, "toast-0"));
}
```

Make `toasts` readable by the test: declare the field `pub toasts: Vec<(u64, SharedString)>` in the new `Workspace` below.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test tab`
Expected: compile errors — `rogg_app::tab`, `Workspace::active_tab`, `toasts` not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/tab.rs`:

```rust
//! One open log in the window: its document, the two log panes with their find fields, selection, follow and
//! the expression editor (spec §5.6).

use crate::document::{Document, DocumentInit, Pane, Source};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, FocusHandle, SharedString, Subscription, Window};
use rogg_ui::controls::FindField;
use rogg_ui::layout::MainWindowParts;
use rogg_ui::logview::{LogRowSource, LogViewEvent, LogViewState, ScrollTo, VecRows};
use std::rc::Rc;

/// Labels the demo sets to match the reference; None means "derive from the document".
#[derive(Clone, Debug, Default)]
pub struct TabLabels {
    pub crumbs: Option<Vec<SharedString>>,
    pub profile: Option<SharedString>,
    pub note: Option<SharedString>,
    pub full_count: Option<SharedString>,
    pub tooltip: Option<SharedString>,
}

#[derive(Clone, Debug, Default)]
pub struct TabInit {
    pub doc: DocumentInit,
    /// Display number to select in both panes once the text loads (not reported in the status bar until the user
    /// selects, as in the reference).
    pub selection: Option<u64>,
    pub labels: TabLabels,
}

pub enum TabEvent {
    Toast(SharedString),
    ShowHistory,
}

/// Document revisions already turned into rows.
#[derive(Clone, Copy, Default, PartialEq)]
struct Applied {
    text: u64,
    filter: u64,
    search: [u64; 2],
}

pub struct Tab {
    pub doc: Entity<Document>,
    pub full: Entity<LogViewState>,
    pub filtered: Entity<LogViewState>,
    /// Tab-cycle order: full log, filter line, filtered log.
    pub focus: [FocusHandle; 3],
    pub find: [Entity<InputState>; 2],
    pub editing: Option<Entity<InputState>>,
    pub follow: bool,
    pub split: f32,
    pub labels: TabLabels,
    /// The pane ⌘F, ⌘G and Wrap act on.
    pub last_pane: Pane,
    selection: Option<u64>,
    pending_selection: Option<u64>,
    applied: Applied,
    edit_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TabEvent> for Tab {}

impl Tab {
    pub fn new(source: Source, init: TabInit, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let doc = cx.new(|cx| Document::new(source, init.doc, cx));
        let empty: Rc<dyn LogRowSource> = Rc::new(VecRows::new(Vec::new()));
        let full = cx.new(|_| LogViewState::new(empty.clone()));
        let filtered = cx.new(|_| LogViewState::new(empty));
        let find = [FindField::state("Find in full log", window, cx), FindField::state("Find in results", window, cx)];
        let subscriptions = vec![
            cx.observe(&doc, |tab, _, cx| tab.sync_rows(cx)),
            cx.subscribe(&full, |tab, _, ev: &LogViewEvent, cx| tab.on_log_event(Pane::Full, ev, cx)),
            cx.subscribe(&filtered, |tab, _, ev: &LogViewEvent, cx| tab.on_log_event(Pane::Filtered, ev, cx)),
            cx.subscribe_in(&find[0], window, |tab, input, ev: &InputEvent, _, cx| tab.on_find_event(Pane::Full, input, ev, cx)),
            cx.subscribe_in(&find[1], window, |tab, input, ev: &InputEvent, _, cx| tab.on_find_event(Pane::Filtered, input, ev, cx)),
        ];
        let mut tab = Self {
            doc,
            full,
            filtered,
            focus: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
            find,
            editing: None,
            follow: true,
            split: MainWindowParts::DEFAULT_SPLIT,
            labels: init.labels,
            last_pane: Pane::Full,
            selection: None,
            pending_selection: init.selection,
            applied: Applied::default(),
            edit_subscription: None,
            _subscriptions: subscriptions,
        };
        tab.sync_rows(cx);
        tab
    }

    pub fn title(&self, cx: &App) -> SharedString {
        self.doc.read(cx).title()
    }

    pub fn state(&self, pane: Pane) -> &Entity<LogViewState> {
        match pane {
            Pane::Full => &self.full,
            Pane::Filtered => &self.filtered,
        }
    }

    /// The display number the user last selected in either pane (shown as `Line N`).
    pub fn selection(&self) -> Option<u64> {
        self.selection
    }

    /// Rebuilds the row sources whose document data changed.
    pub fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let (now, full_rows, filtered_rows) = {
            let doc = self.doc.read(cx);
            let now = Applied {
                text: doc.text_revision(),
                filter: doc.filter.revision,
                search: [doc.search(Pane::Full).revision, doc.search(Pane::Filtered).revision],
            };
            let base_changed = now.text != self.applied.text || now.filter != self.applied.filter;
            let full = (base_changed || now.search[0] != self.applied.search[0]).then(|| doc.full_rows()).flatten();
            let filtered = (base_changed || now.search[1] != self.applied.search[1]).then(|| doc.filtered_rows()).flatten();
            (now, full, filtered)
        };
        let loaded = now.text != self.applied.text;
        self.applied = now;
        if let Some(rows) = full_rows {
            self.full.update(cx, |s, cx| s.set_rows(rows, cx));
        }
        if let Some(rows) = filtered_rows {
            self.filtered.update(cx, |s, cx| s.set_rows(rows, cx));
        }
        if loaded && let Some(number) = self.pending_selection.take() {
            let in_filtered = self.filtered.read(cx).rows().row_for_number(number).is_some();
            self.full.update(cx, |s, cx| s.set_selected(Some(number), cx));
            self.filtered.update(cx, |s, cx| s.set_selected(in_filtered.then_some(number), cx));
        }
        cx.notify();
    }

    /// Selects display `number` in `pane` (centring it when `center`) and mirrors it into the other pane.
    pub fn select(&mut self, pane: Pane, number: u64, center: bool, cx: &mut Context<Self>) {
        let state = self.state(pane).clone();
        let row = state.read(cx).rows().row_for_number(number);
        state.update(cx, |s, cx| {
            s.set_selected(Some(number), cx);
            if center && let Some(row) = row {
                s.scroll_to_row(row, ScrollTo::Center, cx);
            }
        });
        self.sync_selection(pane, number, cx);
    }

    fn on_log_event(&mut self, pane: Pane, ev: &LogViewEvent, cx: &mut Context<Self>) {
        self.last_pane = pane;
        match ev {
            LogViewEvent::Selected { number, .. } => self.sync_selection(pane, *number, cx),
            LogViewEvent::TogglePin { number } => self.toggle_pin(*number, cx),
            LogViewEvent::AddTerm { text, exclude } => {
                let (text, exclude) = (text.clone(), *exclude);
                self.doc.update(cx, |d, cx| d.add_term(&text, exclude, cx));
            }
        }
    }

    /// Spec §5.6: a filtered selection reveals the line in the full log while following; a full-log selection
    /// selects the filtered row when it is shown (scrolling only if needed) and clears it otherwise.
    fn sync_selection(&mut self, from: Pane, number: u64, cx: &mut Context<Self>) {
        self.selection = Some(number);
        match from {
            Pane::Filtered if self.follow => {
                let row = self.full.read(cx).rows().row_for_number(number);
                self.full.update(cx, |s, cx| {
                    s.set_selected(Some(number), cx);
                    if let Some(row) = row {
                        s.scroll_to_row(row, ScrollTo::Center, cx);
                    }
                });
            }
            Pane::Filtered => {}
            Pane::Full => {
                let row = self.filtered.read(cx).rows().row_for_number(number);
                self.filtered.update(cx, |s, cx| {
                    s.set_selected(row.map(|_| number), cx);
                    if let Some(row) = row {
                        s.scroll_to_row(row, ScrollTo::Visible, cx);
                    }
                });
            }
        }
        cx.notify();
    }

    /// Pins or unpins display `number` in both panes.
    pub fn toggle_pin(&mut self, number: u64, cx: &mut Context<Self>) {
        let base = self.doc.read(cx).number_base();
        if let Some(line) = number.checked_sub(base) {
            self.doc.update(cx, |d, cx| d.toggle_pin(line, cx));
        }
    }

    /// ⌘B.
    pub fn pin_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(number) = self.selection {
            self.toggle_pin(number, cx);
        }
    }

    /// Return: centres the selected line in the full log.
    pub fn reveal_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(number) = self.selection {
            self.select(Pane::Full, number, true, cx);
        }
    }

    /// ⌘G: selects and centres display `number` in the full log; false when the log has no such line.
    pub fn go_to_line(&mut self, number: u64, cx: &mut Context<Self>) -> bool {
        if self.full.read(cx).rows().row_for_number(number).is_none() {
            return false;
        }
        self.last_pane = Pane::Full;
        self.select(Pane::Full, number, true, cx);
        true
    }

    /// Enter / Shift-Enter in a find field: the next / previous hit after the selection.
    pub fn jump_to_hit(&mut self, pane: Pane, backwards: bool, cx: &mut Context<Self>) {
        let base = self.doc.read(cx).number_base();
        let from = self.selection.and_then(|n| n.checked_sub(base));
        let Some(line) = self.doc.read(cx).next_hit(pane, from, backwards) else { return };
        self.last_pane = pane;
        self.select(pane, base + line, true, cx);
    }

    pub fn toggle_wrap(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.state(pane).clone().update(cx, |s, cx| {
            let wrap = !s.wrap();
            s.set_wrap(wrap, cx);
        });
        cx.notify();
    }

    pub fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        self.follow = !self.follow;
        cx.notify();
    }

    fn on_find_event(&mut self, pane: Pane, input: &Entity<InputState>, ev: &InputEvent, cx: &mut Context<Self>) {
        match ev {
            InputEvent::Change => {
                let query = input.read(cx).value().to_string();
                self.doc.update(cx, |d, cx| d.set_search(pane, &query, cx));
            }
            InputEvent::PressEnter { shift, .. } => self.jump_to_hit(pane, *shift, cx),
            InputEvent::Focus => self.last_pane = pane,
            _ => {}
        }
    }

    /// Edit mode (edit button, ⌘L, Enter on the filter line): the chips become an expression input.
    pub fn start_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let text = self.doc.read(cx).filter.spec.expression();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        input.update(cx, |i, cx| i.focus(window, cx));
        self.edit_subscription = Some(cx.subscribe_in(&input, window, |tab, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                tab.apply_edit(window, cx);
            }
        }));
        self.editing = Some(input);
        cx.notify();
    }

    /// Applies the edited expression; an invalid one keeps the editor open with the error under it.
    pub fn apply_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.editing.clone() else { return };
        let text = input.read(cx).value().to_string();
        if self.doc.update(cx, |d, cx| d.set_expression(&text, cx)).is_ok() {
            self.cancel_edit(window, cx);
        }
    }

    /// Escape: closes the editor, keeps the applied filter and clears a rejected expression's error.
    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.edit_subscription = None;
        self.doc.update(cx, |d, cx| d.clear_filter_error(cx));
        self.focus[1].focus(window, cx);
        cx.notify();
    }
}
```

Create `crates/rogg-app/src/tab_view.rs`:

```rust
//! A tab as window parts: info bar, pane headers, panes, filter band and status bar — or a loading/error
//! placeholder (spec §5.3, §5.7).

use crate::actions::FILTER_CONTEXT;
use crate::document::{LoadState, Pane};
use crate::tab::{Tab, TabEvent};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Entity, SharedString, Window, div};
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, group_digits, noun};
use rogg_ui::chrome::{InfoBar, PaneHeader, Progress, StatPart, StatusBar};
use rogg_ui::controls::{FindField, ToolButton};
use rogg_ui::empty;
use rogg_ui::filter::{ChipState, FilterBand};
use rogg_ui::icons::IconName;
use rogg_ui::logview::LogView;
use std::rc::Rc;

pub enum TabBody {
    Panes { top_header: AnyElement, top_body: AnyElement, band: AnyElement, bottom_header: AnyElement, bottom_body: AnyElement, split: f32 },
    Placeholder(AnyElement),
}

pub struct TabParts {
    pub info_bar: AnyElement,
    pub body: TabBody,
    pub status: AnyElement,
}

const CONTEXT_OPTIONS: [&str; 4] = ["None", "±1 line", "±2 lines", "±3 lines"];

fn tab_act(tab: &Entity<Tab>, f: impl Fn(&mut Tab, &mut Window, &mut Context<Tab>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let tab = tab.clone();
    move |window, cx| tab.update(cx, |t, cx| f(t, window, cx))
}

fn tab_act_ix(tab: &Entity<Tab>, f: impl Fn(&mut Tab, usize, &mut Window, &mut Context<Tab>) + 'static) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let tab = tab.clone();
    move |i, window, cx| tab.update(cx, |t, cx| f(t, i, window, cx))
}

/// A button for a feature of a later milestone: says so in a toast.
fn later(tab: &Entity<Tab>, message: &'static str) -> impl Fn(&mut Window, &mut App) + 'static {
    tab_act(tab, move |_, _, cx| cx.emit(TabEvent::Toast(message.into())))
}

pub fn chip_state(state: TermState) -> ChipState {
    match state {
        TermState::Include => ChipState::Include,
        TermState::Off => ChipState::Off,
        TermState::Exclude => ChipState::Exclude,
    }
}

/// `6 matches / 40 lines · 1 pinned (1 added) · 0 context` with the counts in bold (reference footer).
fn stat_parts(stats: &FilterStats) -> Vec<StatPart> {
    let mut parts = vec![
        StatPart::Strong(group_digits(stats.matched).into()),
        StatPart::Text(
            format!(" {} / {} {} · ", noun(stats.matched, "match", "matches"), group_digits(stats.total_lines), noun(stats.total_lines, "line", "lines")).into(),
        ),
        StatPart::Strong(group_digits(stats.pinned).into()),
        StatPart::Text(" pinned".into()),
    ];
    if stats.pinned_unmatched > 0 {
        parts.push(StatPart::Text(format!(" ({} added)", group_digits(stats.pinned_unmatched)).into()));
    }
    parts.push(StatPart::Text(" · ".into()));
    parts.push(StatPart::Strong(group_digits(stats.context_lines).into()));
    parts.push(StatPart::Text(" context".into()));
    parts
}

pub fn parts(tab: &Entity<Tab>, window: &Window, cx: &App) -> TabParts {
    let t = tab.read(cx);
    let d = t.doc.read(cx);
    let title = d.title();
    let crumbs = t.labels.crumbs.clone().unwrap_or_else(|| match d.source.path().and_then(|p| p.parent()).and_then(|p| p.file_name()) {
        Some(dir) => vec![dir.to_string_lossy().into_owned().into(), title.clone()],
        None => vec![title.clone()],
    });
    let info_bar = InfoBar::new(crumbs, t.labels.profile.clone().unwrap_or_else(|| "Default".into()))
        .on_profile(later(tab, "Highlighting profiles arrive in a later version"))
        .on_export(later(tab, "Export arrives in a later version"))
        .into_any_element();
    let note = t.labels.note.clone().or_else(|| d.text().map(|text| SharedString::from(text.status_label()))).unwrap_or_default();
    let progress = d.filter.progress.map(|f| Progress { label: format!("Filtering {:.0}%", f * 100.).into(), fraction: f });
    let status = StatusBar::new(note)
        .stats(d.filter.stats.as_ref().map(stat_parts).unwrap_or_default())
        .selection(t.selection().map(|n| SharedString::from(format!("Line {n}"))))
        .progress(progress)
        .into_any_element();

    let body = match &d.load {
        LoadState::Loading { fraction, label } => {
            TabBody::Placeholder(empty::loading(label.clone(), Some(*fraction), Rc::new(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.cancel_load(cx))))).into_any_element())
        }
        LoadState::Failed { message } => {
            let path = d.source.path().map(|p| p.display().to_string()).unwrap_or_default();
            TabBody::Placeholder(empty::error(message.clone(), path, Rc::new(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.retry(cx))))).into_any_element())
        }
        LoadState::Ready(text) => {
            let lines = text.line_count();
            let full_count = t.labels.full_count.clone().unwrap_or_else(|| format!("{} {}", group_digits(lines), noun(lines, "line", "lines")).into());
            let shown = d.filter.view.shown_lines().len() as u64;
            let top_header = PaneHeader::new("pane-top-head", "Full log", full_count)
                .find(FindField::new("find-top", t.find[0].clone()).count(d.search(Pane::Full).label()))
                .tool(ToolButton::new("tool-wrap-top", IconName::Wrap, "Wrap").pressed(t.full.read(cx).wrap()).on_click(tab_act(tab, |t, _, cx| t.toggle_wrap(Pane::Full, cx))))
                .into_any_element();
            let bottom_header = PaneHeader::new("pane-bottom-head", "Filtered log", format!("{} shown", group_digits(shown)))
                .find(FindField::new("find-bottom", t.find[1].clone()).count(d.search(Pane::Filtered).label()))
                .tool(ToolButton::new("tool-open-tab", IconName::OpenTab, "Open as tab").on_click(later(tab, "Open as tab arrives in a later version")))
                .tool(ToolButton::new("tool-follow", IconName::Follow, "Follow selection").pressed(t.follow).on_click(tab_act(tab, |t, _, cx| t.toggle_follow(cx))))
                .tool(
                    ToolButton::new("tool-wrap-bottom", IconName::Wrap, "Wrap")
                        .pressed(t.filtered.read(cx).wrap())
                        .on_click(tab_act(tab, |t, _, cx| t.toggle_wrap(Pane::Filtered, cx))),
                )
                .into_any_element();
            let spec = &d.filter.spec;
            let band = FilterBand::new(spec.terms.iter().map(|term| (SharedString::from(term.text.clone()), chip_state(term.state))).collect())
                .regex(spec.regex)
                .case_sensitive(spec.case_sensitive)
                .editing(t.editing.clone())
                .error(d.filter.error.clone())
                .context(CONTEXT_OPTIONS.iter().map(|o| SharedString::from(*o)).collect(), (spec.context as usize).min(CONTEXT_OPTIONS.len() - 1))
                .summary(spec.summary().into())
                .focused(t.focus[1].is_focused(window))
                .on_cycle(tab_act_ix(tab, |t, i, _, cx| t.doc.update(cx, |d, cx| d.cycle_term(i, cx))))
                .on_edit(tab_act(tab, |t, window, cx| t.start_edit(window, cx)))
                .on_apply(tab_act(tab, |t, window, cx| t.apply_edit(window, cx)))
                .on_toggle_regex(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.toggle_regex(cx))))
                .on_toggle_case(tab_act(tab, |t, _, cx| t.doc.update(cx, |d, cx| d.toggle_case(cx))))
                .on_history(tab_act(tab, |_, _, cx| cx.emit(TabEvent::ShowHistory)))
                .on_context(tab_act_ix(tab, |t, i, _, cx| t.doc.update(cx, |d, cx| d.set_context(i as u32, cx))));
            let band = div().track_focus(&t.focus[1]).key_context(FILTER_CONTEXT).child(band).into_any_element();
            let top_body = LogView::new("log-top", t.full.clone()).focus(t.focus[0].clone()).into_any_element();
            let bottom_body = if shown == 0 && d.filter.progress.is_none() {
                empty::no_matches("empty-results").into_any_element()
            } else {
                LogView::new("log-bottom", t.filtered.clone()).focus(t.focus[2].clone()).into_any_element()
            };
            TabBody::Panes { top_header, top_body, band, bottom_header, bottom_body, split: t.split }
        }
    };
    TabParts { info_bar, body, status }
}
```

Replace `crates/rogg-app/src/workspace/mod.rs` with:

```rust
//! The main window view: title bar, tabs, sidebar, dialogs, toasts and the app's actions.

use crate::actions::{self, Dismiss, OpenFile, OpenFolder, ShowShortcuts};
use crate::args::Args;
use crate::document::Source;
use crate::tab::{Tab, TabEvent, TabInit};
use crate::tab_view::{self, TabBody};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, Context, Div, Entity, EntityId, FocusHandle, Pixels, SharedString, Stateful, Subscription, Window, div, px};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{LayoutMeasure, MainWindowLayout, MainWindowParts, SingleParts};
use rogg_ui::theme::size;
use rogg_ui::titlebar::{TabItem, TitleBar};
use rogg_ui::toast::ToastStack;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// How long a toast stays up.
const TOAST_TIME: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub enum Dialog {
    Shortcuts,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    pub(crate) sidebar_visible: bool,
    pub(crate) sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) tabs: Vec<Entity<Tab>>,
    pub(crate) active: usize,
    tab_subscriptions: HashMap<EntityId, Vec<Subscription>>,
    pub(crate) measure: Rc<LayoutMeasure>,
    pub toasts: Vec<(u64, SharedString)>,
    next_toast: u64,
    traffic_lights: bool,
}

/// A `Fn(&mut Window, &mut App)` callback that runs `f` on the workspace.
pub(crate) fn act(this: &Entity<Workspace>, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| f(ws, window, cx))
}

/// Like `act`, for callbacks carrying an index.
pub(crate) fn act_ix(
    this: &Entity<Workspace>,
    f: impl Fn(&mut Workspace, usize, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| f(ws, i, window, cx))
}

impl Workspace {
    pub fn new(args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut ws = Self {
            focus,
            sidebar_visible: true,
            sidebar_width: px(size::SIDEBAR_W),
            dialog: None,
            tabs: Vec::new(),
            active: 0,
            tab_subscriptions: HashMap::new(),
            measure: Rc::new(LayoutMeasure::default()),
            toasts: Vec::new(),
            next_toast: 0,
            traffic_lights: false,
        };
        ws.open_paths(args.paths, window, cx);
        ws
    }

    pub fn tabs(&self) -> &[Entity<Tab>] {
        &self.tabs
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> Option<Entity<Tab>> {
        self.tabs.get(self.active).cloned()
    }

    pub fn open_source(&mut self, source: Source, init: TabInit, window: &mut Window, cx: &mut Context<Self>) -> Entity<Tab> {
        let tab = cx.new(|cx| Tab::new(source, init, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&tab, window, |ws, _, ev: &TabEvent, window, cx| ws.on_tab_event(ev, window, cx)),
            cx.observe(&tab, |_, _, cx| cx.notify()),
        ];
        self.tab_subscriptions.insert(tab.entity_id(), subscriptions);
        self.tabs.push(tab.clone());
        self.activate(self.tabs.len() - 1, window, cx);
        tab
    }

    /// Opens each path in a tab, or activates the tab already showing it.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            let existing = self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.path() == Some(path.as_path()));
            match existing {
                Some(index) => self.activate(index, window, cx),
                None => {
                    self.open_source(Source::File(path), TabInit::default(), window, cx);
                }
            }
        }
    }

    pub fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else { return };
        self.active = index;
        let title = tab.read(cx).title(cx);
        window.set_window_title(&title);
        cx.notify();
    }

    pub fn toast(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push((id, message.into()));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TIME).await;
            this.update(cx, |ws, cx| ws.dismiss_toast(id, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|(t, _)| *t != id);
        cx.notify();
    }

    fn on_tab_event(&mut self, event: &TabEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TabEvent::Toast(message) => self.toast(message.clone(), cx),
            TabEvent::ShowHistory => {}
        }
    }

    /// Draw the traffic lights ourselves (headless captures have no native ones).
    pub fn set_traffic_lights(&mut self, on: bool, cx: &mut Context<Self>) {
        self.traffic_lights = on;
        cx.notify();
    }

    pub fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    /// Escape: closes a dialog, else cancels the expression editor.
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        } else if let Some(tab) = self.active_tab() {
            tab.update(cx, |t, cx| {
                if t.editing.is_some() {
                    t.cancel_edit(window, cx);
                }
            });
        }
    }

    fn titlebar(&self, this: &Entity<Self>, cx: &App) -> TitleBar {
        let items = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let t = tab.read(cx);
                let d = t.doc.read(cx);
                let tooltip = t.labels.tooltip.clone().or_else(|| d.source.path().map(|p| SharedString::from(p.display().to_string()))).unwrap_or_else(|| d.title());
                TabItem { label: d.title(), tooltip, active: i == self.active }
            })
            .collect();
        TitleBar::new(items)
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
            .on_select_tab(act_ix(this, |ws, i, window, cx| ws.activate(i, window, cx)))
            .on_shortcuts(act(this, |ws, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
    }

    fn dialog_element(&self, dialog: &Dialog, this: &Entity<Self>) -> AnyElement {
        match dialog {
            Dialog::Shortcuts => dialogs::shortcuts(&actions::shortcut_rows())
                .description("Keyboard shortcuts for the full log, the filter line and the filtered log.")
                .on_close(act(this, |ws, _, cx| ws.close_dialog(cx)))
                .into_any_element(),
        }
    }

    /// The modal layer and the toasts, over everything else. The container has no handlers, so it does not catch
    /// the mouse.
    fn overlay(&self, this: &Entity<Self>) -> Option<AnyElement> {
        let modal = self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))));
        if modal.is_none() && self.toasts.is_empty() {
            return None;
        }
        let dismiss = this.clone();
        Some(
            div()
                .absolute()
                .inset_0()
                .children(modal)
                .when(!self.toasts.is_empty(), |d| {
                    d.child(ToastStack::new(self.toasts.clone()).on_dismiss(move |id, _, cx| dismiss.update(cx, |ws, cx| ws.dismiss_toast(id, cx))))
                })
                .into_any_element(),
        )
    }

    fn welcome(&self) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        empty::welcome(vec![], open_file, open_folder, Rc::new(|_: usize, _: &mut Window, _: &mut App| {})).into_any_element()
    }

    fn root(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(cx.listener(|ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep keys flowing to the window's actions when the focused element went away (an applied editor, a
        // closed dialog's input).
        if window.focused(cx).is_none() {
            cx.defer_in(window, |ws, window, cx| ws.focus.focus(window, cx));
        }
        let this = cx.entity();
        let titlebar = self.titlebar(&this, cx).into_any_element();
        let overlay = self.overlay(&this);
        let sidebar: Option<AnyElement> = None;
        let content = match self.active_tab() {
            None => MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: None, body: self.welcome(), status: None, overlay }).into_any_element(),
            Some(tab) => {
                let parts = tab_view::parts(&tab, window, cx);
                match parts.body {
                    TabBody::Panes { top_header, top_body, band, bottom_header, bottom_body, split } => MainWindowLayout::new(MainWindowParts {
                        titlebar,
                        sidebar,
                        info_bar: parts.info_bar,
                        top_header,
                        top_body,
                        band,
                        bottom_header,
                        bottom_body,
                        status: parts.status,
                        overlay,
                        split,
                    })
                    .measure(self.measure.clone())
                    .sidebar_resizable(true)
                    .into_any_element(),
                    TabBody::Placeholder(body) => {
                        MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: Some(parts.info_bar), body, status: Some(parts.status), overlay })
                            .into_any_element()
                    }
                }
            }
        };
        self.root(cx).child(content)
    }
}
```

Add `pub mod tab;` and `pub mod tab_view;` to `crates/rogg-app/src/lib.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass (including Task 6's `tests/app.rs`), no warnings.

- [ ] **Step 5: Smoke-run the real window**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; just genlog 8M target/genlog/smoke.log && (cargo run -p rogg-app -- target/genlog/smoke.log & sleep 8; kill %1)`
Expected: the window opens with the file in both panes and no panic is printed. If the sandbox refuses background jobs, skip this step and say so in the report.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): tabs with both panes, filter band, find, follow selection, pins and status bar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 12: Window chrome — tab management, sidebar, recent files, open prompts, drag-and-drop, split handles, dialogs

**Files:**
- Create: `crates/rogg-app/src/workspace/chrome.rs`, `crates/rogg-app/src/sidebar_model.rs`, `crates/rogg-app/src/recent.rs`, `crates/rogg-app/src/format.rs`, `crates/rogg-app/tests/workspace.rs`
- Modify: `crates/rogg-app/src/workspace/mod.rs` (replace), `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `RecentEntry`/`RecentList`; Task 5 `history_picker`, `BandDrag`, `SidebarDrag`, `LayoutMeasure::split_at`, `TitleBar::tab_scroll`, `ToastStack`; Task 6 menus/actions; Task 8 `HistoryStore`; Task 11 `Tab`, `Workspace`.
- Produces (used by Tasks 13–15):
  - `rogg_app::recent::RecentStore` (gpui `Global`): `load(PathBuf)`, `entries()`, `record(RecentEntry)`, `remove(&Path)`, pub `recovered`, `last_error`. Installed by `install_globals`; `init_with` builds the menus from it.
  - `rogg_app::format::format_size(u64) -> String` (`820 B`, `14 KB`, `3.2 MB`, `1.2 GB`, decimal units).
  - `rogg_app::sidebar_model::{SidebarEntry { label, size, tab: Option<EntityId>, open: Option<(Source, TabInit)> }, SidebarGroup { label, entries }, FixedSidebar { groups, count, footer, recent }, RowAction::{ToggleGroup, Activate, Open, Nothing}, OpenFile { tab, title, path, bytes, failed }, group_open_files(&[OpenFile]) -> Vec<SidebarGroup>, rows(&[SidebarGroup], Option<EntityId>, &HashSet<SharedString>) -> (Vec<TreeRow>, Vec<RowAction>)}`.
  - `rogg_app::workspace::{Dialog::{Shortcuts, AllTabs, History}, chrome::folder_files(&Path) -> io::Result<Vec<PathBuf>>, chrome::FOLDER_FILE_LIMIT = 50}`.
  - `Workspace` pub fields `sidebar_visible`, `sidebar_width`, `toasts`; methods `open_paths` (folders too; existing files recorded in Recent), `open_folder(PathBuf, window, cx)`, `open_recent(usize, window, cx)`, `close_tab(usize, window, cx)`, `cycle_tab(forward, window, cx)`, `toggle_sidebar(cx)`, `drag_band(Pixels, cx)`, `drag_sidebar(Pixels, cx)`, `restore_history(usize, cx)`, `set_fixed_sidebar(FixedSidebar, cx)`, `refresh_menus(&mut App)`.
  - Behaviour: ⌘O/⇧⌘O open native panels; dropped files and folders open; Open Folder opens the regular, non-hidden text files directly inside the folder (binary files — a NUL in the first 8 KB, unless UTF-16 — are skipped; at most 50; the recursive folder tree is M3); the sidebar lists open files grouped by folder (`N files`, sizes, active row, failed rows in the error colour, collapsible folders, footer with the active file's lines and size) and is resizable (155–360 px) and toggleable (⌘\\); the band handle resizes the panes; tabs close with ⌘W/close button/middle click (closing the active tab activates the tab now at its index), switch with ⌘1…9 and ⌃Tab/⌃⇧Tab, show the path as tooltip and scroll into view; All tabs (`⌄`) and History open picker dialogs; Settings shows a "later version" toast; unreadable history or recent files show a toast at start; Recent entries that no longer exist render faint and are removed (with a toast) when clicked; File → Open Recent ▸ is rebuilt whenever the list changes.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/workspace.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, Entity, TestAppContext, px};
use rogg_app::args::Args;
use rogg_app::recent::RecentStore;
use rogg_app::workspace::Workspace;
use rogg_ui::layout::MainWindowParts;

fn open_two(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let a = common::write_log(dir.path(), "a.log", &["alpha one", "beta two"]);
    let b = common::write_log(dir.path(), "b.log", &["gamma"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![a, b] });
    (dir, w, ws)
}

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> (usize, usize) {
    ws.read_with(cx, |ws, _| (ws.tabs().len(), ws.active_index()))
}

#[gpui_kit::test]
fn tabs_switch_close_and_reopen(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_two(cx);
    assert_eq!(active(&ws, cx), (2, 1));
    common::click(cx, w, "tab-0");
    assert_eq!(active(&ws, cx), (2, 0));
    common::press(cx, w, "cmd-2");
    assert_eq!(active(&ws, cx), (2, 1));
    common::press(cx, w, "ctrl-tab");
    assert_eq!(active(&ws, cx), (2, 0), "next wraps around");
    let b = dir.path().join("b.log");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![b], window, cx))).unwrap();
    assert_eq!(active(&ws, cx), (2, 1), "an open file activates its tab");
    common::click(cx, w, "tab-close-1");
    assert_eq!(active(&ws, cx), (1, 0));
    common::press(cx, w, "cmd-w");
    assert_eq!(active(&ws, cx), (0, 0));
    assert!(common::exists(cx, w, "welcome"));
}

#[gpui_kit::test]
fn sidebar_lists_open_files_by_folder(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    assert!(common::exists(cx, w, "sidebar"));
    assert!(common::exists(cx, w, "tree-row-0"), "the folder row");
    assert!(common::exists(cx, w, "tree-row-2") && !common::exists(cx, w, "tree-row-3"));
    common::click(cx, w, "tree-row-1");
    assert_eq!(active(&ws, cx), (2, 0));
    common::click(cx, w, "tree-row-0");
    assert!(!common::exists(cx, w, "tree-row-1"), "the folder collapses");
    common::press(cx, w, "cmd-\\");
    assert!(!common::exists(cx, w, "sidebar"));
}

#[gpui_kit::test]
fn recent_files_are_recorded_persisted_and_pruned(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_two(cx);
    cx.update(|cx| {
        let entries = cx.global::<RecentStore>().entries();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].path.ends_with("b.log"), "newest first");
    });
    assert!(dir.path().join("recent.json").exists());
    assert!(common::exists(cx, w, "recent-1"));
    std::fs::remove_file(dir.path().join("a.log")).unwrap();
    common::click(cx, w, "recent-1");
    cx.update(|cx| assert_eq!(cx.global::<RecentStore>().entries().len(), 1));
    assert!(ws.read_with(cx, |ws, _| !ws.toasts.is_empty()), "the removal is announced");
}

#[gpui_kit::test]
fn folders_open_their_text_files(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let logs = dir.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    common::write_log(&logs, "one.log", &["1"]);
    common::write_log(&logs, "two.log", &["2"]);
    common::write_log(&logs, ".hidden.log", &["h"]);
    std::fs::write(logs.join("blob.bin"), [0u8, 159, 146, 150]).unwrap();
    std::fs::write(logs.join("utf16.log"), [0xFF, 0xFE, b'h', 0, b'i', 0]).unwrap();
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![logs.clone()] });
    let titles: Vec<String> = ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect());
    assert_eq!(titles, ["one.log", "two.log", "utf16.log"]);
    cx.update(|cx| {
        let entries = cx.global::<RecentStore>().entries();
        assert_eq!(entries.len(), 1, "the folder is recorded, not each file");
        assert!(entries[0].is_folder);
    });
}

#[gpui_kit::test]
fn drag_handles_resize_panes_and_sidebar(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let band = common::bounds(cx, w, "filter-band");
    ws.update(cx, |ws, cx| ws.drag_band(band.top() - px(100.), cx));
    let split = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).split);
    assert!(split < MainWindowParts::DEFAULT_SPLIT - 0.05, "split {split}");
    ws.update(cx, |ws, cx| ws.drag_sidebar(px(1000.), cx));
    assert_eq!(ws.read_with(cx, |ws, _| ws.sidebar_width), px(360.));
    ws.update(cx, |ws, cx| ws.drag_sidebar(px(10.), cx));
    assert_eq!(ws.read_with(cx, |ws, _| ws.sidebar_width), px(155.));
}

#[gpui_kit::test]
fn history_dialog_restores_an_older_filter(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("gamma", cx)).unwrap();
    common::click(cx, w, "history");
    assert!(common::exists(cx, w, "dialog"));
    common::click(cx, w, "history-pick-1");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "alpha");
    assert!(!common::exists(cx, w, "dialog"));
}

#[gpui_kit::test]
fn all_tabs_dialog_activates_a_tab(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "all-tabs");
    assert!(common::exists(cx, w, "dialog"));
    common::click(cx, w, "picker-0");
    assert_eq!(active(&ws, cx), (2, 0));
    assert!(!common::exists(cx, w, "dialog"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test workspace`
Expected: compile errors — `rogg_app::recent`, `drag_band`, `drag_sidebar`, `sidebar_width` (private) not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/format.rs`:

```rust
//! Human-readable sizes for the sidebar and its footer.

/// Decimal units, as Finder shows them: `820 B`, `14 KB`, `3.2 MB`, `1.2 GB`.
pub fn format_size(bytes: u64) -> String {
    const K: f64 = 1000.;
    let b = bytes as f64;
    if b < K {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.0} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.1} GB", b / (K * K * K))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(format_size(820), "820 B");
        assert_eq!(format_size(14_200), "14 KB");
        assert_eq!(format_size(3_240_000), "3.2 MB");
        assert_eq!(format_size(1_200_000_000), "1.2 GB");
    }
}
```

Create `crates/rogg-app/src/recent.rs`:

```rust
//! Recent files and folders (spec §6.3), shared by the sidebar, the welcome view and File → Open Recent.

use gpui_kit::Global;
use rogg_core::recent::{RecentEntry, RecentList};
use std::path::{Path, PathBuf};

pub struct RecentStore {
    list: RecentList,
    path: PathBuf,
    /// The file was unreadable and was moved to `recent.json.bak` (shown as a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for RecentStore {}

impl RecentStore {
    pub fn load(path: PathBuf) -> Self {
        match RecentList::load(&path) {
            Ok((list, recovered)) => Self { list, path, recovered, last_error: None },
            Err(e) => Self { list: RecentList::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn entries(&self) -> &[RecentEntry] {
        self.list.entries()
    }

    pub fn record(&mut self, entry: RecentEntry) {
        self.list.record(entry);
        self.save();
    }

    pub fn remove(&mut self, path: &Path) {
        self.list.remove(path);
        self.save();
    }

    fn save(&mut self) {
        self.last_error = self.list.save(&self.path).err().map(|e| e.to_string());
    }
}
```

Create `crates/rogg-app/src/sidebar_model.rs`:

```rust
//! The sidebar's file tree (spec §5.3): open files grouped by folder, or a fixed tree (the demo bundle), and the
//! action behind each row.

use crate::document::Source;
use crate::format::format_size;
use crate::tab::TabInit;
use gpui_kit::{EntityId, SharedString};
use rogg_ui::sidebar::{RecentItem, SidebarFooter, TreeRow};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct SidebarEntry {
    pub label: SharedString,
    pub size: SharedString,
    /// The open tab showing this entry.
    pub tab: Option<EntityId>,
    /// How to open the entry when no tab shows it (fixed trees only).
    pub open: Option<(Source, TabInit)>,
}

#[derive(Clone, Debug)]
pub struct SidebarGroup {
    pub label: SharedString,
    pub entries: Vec<SidebarEntry>,
}

/// A sidebar that does not follow the open tabs (the demo's bundle).
#[derive(Clone, Debug)]
pub struct FixedSidebar {
    pub groups: Vec<SidebarGroup>,
    pub count: SharedString,
    pub footer: SidebarFooter,
    pub recent: Vec<RecentItem>,
}

#[derive(Clone, Debug)]
pub enum RowAction {
    ToggleGroup(SharedString),
    Activate(EntityId),
    Open(Source, TabInit),
    Nothing,
}

/// An open tab as the sidebar sees it.
pub struct OpenFile {
    pub tab: EntityId,
    pub title: SharedString,
    pub path: Option<PathBuf>,
    /// Text size once loaded.
    pub bytes: Option<u64>,
    pub failed: bool,
}

/// Groups open files by their folder's name, in tab order; in-memory sources go under "In memory".
pub fn group_open_files(files: &[OpenFile]) -> Vec<SidebarGroup> {
    let mut groups: Vec<SidebarGroup> = Vec::new();
    for f in files {
        let label: SharedString = match f.path.as_ref().and_then(|p| p.parent()) {
            Some(dir) => dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string()).into(),
            None => "In memory".into(),
        };
        let entry = SidebarEntry { label: f.title.clone(), size: f.bytes.map(format_size).unwrap_or_default().into(), tab: Some(f.tab), open: None };
        match groups.iter_mut().find(|g| g.label == label) {
            Some(group) => group.entries.push(entry),
            None => groups.push(SidebarGroup { label, entries: vec![entry] }),
        }
    }
    groups
}

/// Flattens `groups` into sidebar rows and the action of each row. Collapsed groups hide their entries; the
/// entry of the `active` tab is highlighted; `failed` marks entries whose tab failed to open.
pub fn rows(groups: &[SidebarGroup], active: Option<EntityId>, collapsed: &HashSet<SharedString>) -> (Vec<TreeRow>, Vec<RowAction>) {
    rows_with_failures(groups, active, collapsed, &HashSet::new())
}

pub fn rows_with_failures(
    groups: &[SidebarGroup],
    active: Option<EntityId>,
    collapsed: &HashSet<SharedString>,
    failed: &HashSet<EntityId>,
) -> (Vec<TreeRow>, Vec<RowAction>) {
    let mut rows = Vec::new();
    let mut actions = Vec::new();
    for group in groups {
        let expanded = !collapsed.contains(&group.label);
        rows.push(TreeRow::Folder { label: group.label.clone(), expanded, depth: 0 });
        actions.push(RowAction::ToggleGroup(group.label.clone()));
        if !expanded {
            continue;
        }
        for entry in &group.entries {
            rows.push(TreeRow::File {
                label: entry.label.clone(),
                size: entry.size.clone(),
                active: entry.tab.is_some() && entry.tab == active,
                depth: 1,
                failed: entry.tab.is_some_and(|t| failed.contains(&t)),
            });
            actions.push(match (&entry.tab, &entry.open) {
                (Some(tab), _) => RowAction::Activate(*tab),
                (None, Some((source, init))) => RowAction::Open(source.clone(), init.clone()),
                (None, None) => RowAction::Nothing,
            });
        }
    }
    (rows, actions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_folder_and_flattens_with_actions() {
        let a = EntityId::from(1u64);
        let b = EntityId::from(2u64);
        let c = EntityId::from(3u64);
        let files = [
            OpenFile { tab: a, title: "a.log".into(), path: Some("/var/log/a.log".into()), bytes: Some(14_200), failed: false },
            OpenFile { tab: b, title: "mem".into(), path: None, bytes: None, failed: false },
            OpenFile { tab: c, title: "c.log".into(), path: Some("/var/log/c.log".into()), bytes: None, failed: true },
        ];
        let groups = group_open_files(&files);
        assert_eq!(groups.iter().map(|g| g.label.as_ref()).collect::<Vec<_>>(), ["log", "In memory"]);
        assert_eq!(groups[0].entries[0].size.as_ref(), "14 KB");
        let failed = HashSet::from([c]);
        let (rows, actions) = rows_with_failures(&groups, Some(c), &HashSet::new(), &failed);
        assert_eq!(rows.len(), 5);
        assert!(matches!(&rows[2], TreeRow::File { active: true, failed: true, .. }));
        assert!(matches!(actions[1], RowAction::Activate(t) if t == a));
        let collapsed = HashSet::from([SharedString::from("log")]);
        let (rows, _) = rows(&groups, None, &collapsed);
        assert_eq!(rows.len(), 3, "collapsed folder hides its two files");
    }
}
```

If `EntityId::from(u64)` is not available in gpui-pre 0.3.8, build the ids from real entities in a `#[gpui_kit::test]` instead and report it.

Create `crates/rogg-app/src/workspace/chrome.rs`:

```rust
//! Tab management, the sidebar, recent files, open prompts, drag-and-drop and the split handles.

use super::{Workspace, act_ix};
use crate::document::{LoadState, Source};
use crate::history::HistoryStore;
use crate::menus;
use crate::recent::RecentStore;
use crate::sidebar_model::{self, FixedSidebar, OpenFile, RowAction, SidebarEntry, SidebarGroup};
use crate::tab::TabInit;
use gpui_kit::{AnyElement, App, Context, Entity, EntityId, IntoElement, PathPromptOptions, Pixels, SharedString, Window, px};
use rogg_core::encoding::Encoding;
use rogg_core::recent::RecentEntry;
use rogg_core::stats::{group_digits, noun};
use rogg_ui::sidebar::{RecentItem, Sidebar, SidebarFooter};
use rogg_ui::theme::size;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Open Folder (M2) opens at most this many files: the regular, non-hidden text files directly inside the folder.
/// Recursive folder trees and archives arrive with folder sources (M3).
pub const FOLDER_FILE_LIMIT: usize = 50;

/// The regular, non-hidden files directly in `dir` that look like text (no NUL byte in the first 8 KB, unless the
/// file starts with a UTF-16 BOM), sorted by name, at most `FOLDER_FILE_LIMIT`.
pub fn folder_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if looks_like_text(&path) {
            files.push(path);
        }
    }
    files.sort();
    files.truncate(FOLDER_FILE_LIMIT);
    Ok(files)
}

fn looks_like_text(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut head = [0u8; 8192];
    let mut len = 0;
    while len < head.len() {
        match file.read(&mut head[len..]) {
            Ok(0) => break,
            Ok(n) => len += n,
            Err(_) => return false,
        }
    }
    Encoding::detect(&head[..len]).is_utf16() || !head[..len].contains(&0)
}

pub(crate) fn recent_items(cx: &App) -> Vec<RecentItem> {
    cx.try_global::<RecentStore>()
        .map(|s| s.entries().iter().map(|e| RecentItem { label: e.label().into(), is_folder: e.is_folder, missing: e.is_missing() }).collect())
        .unwrap_or_default()
}

impl Workspace {
    /// Opens each path: folders through `open_folder`, files in a tab or by activating the tab already showing them.
    /// Files that exist are recorded in Recent.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            if path.is_dir() {
                self.open_folder(path, window, cx);
            } else {
                if path.exists() {
                    self.record_recent(RecentEntry::file(&path), cx);
                }
                self.open_file(path, window, cx);
            }
        }
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let existing = self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.path() == Some(path.as_path()));
        match existing {
            Some(index) => self.activate(index, window, cx),
            None => {
                self.open_source(Source::File(path), TabInit::default(), window, cx);
            }
        }
    }

    pub fn open_folder(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match folder_files(&dir) {
            Ok(files) if files.is_empty() => self.toast(format!("No text files in {}", dir.display()), cx),
            Ok(files) => {
                self.record_recent(RecentEntry::folder(&dir), cx);
                for file in files {
                    self.open_file(file, window, cx);
                }
            }
            Err(e) => self.toast(format!("Could not read {}: {e}", dir.display()), cx),
        }
    }

    /// ⌘O / ⇧⌘O: the native open panel.
    pub(crate) fn prompt_open(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: !folder, directories: folder, multiple: !folder, prompt: Some("Open".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                this.update_in(cx, |ws, window, cx| ws.open_paths(paths, window, cx)).ok();
            }
        })
        .detach();
    }

    pub fn open_recent(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.fixed_sidebar.is_some() {
            self.toast("Recent entries are examples in the demo", cx);
            return;
        }
        let Some(entry) = cx.try_global::<RecentStore>().and_then(|s| s.entries().get(index).cloned()) else { return };
        if entry.is_missing() {
            cx.update_global::<RecentStore, _>(|s, _| s.remove(&entry.path));
            self.refresh_menus(cx);
            self.toast(format!("{} no longer exists and was removed from Recent", entry.label()), cx);
        } else {
            self.open_paths(vec![entry.path], window, cx);
        }
    }

    fn record_recent(&mut self, entry: RecentEntry, cx: &mut Context<Self>) {
        if cx.has_global::<RecentStore>() {
            cx.update_global::<RecentStore, _>(|s, _| s.record(entry));
            self.refresh_menus(cx);
        }
    }

    /// Rebuilds the menu bar so File → Open Recent ▸ lists the current entries.
    pub fn refresh_menus(&self, cx: &mut App) {
        let entries = cx.try_global::<RecentStore>().map(|s| s.entries().to_vec()).unwrap_or_default();
        cx.set_menus(menus::app_menus(&entries));
    }

    /// Closes a tab; closing the active tab activates the tab now at its index (reference `closeTab`).
    pub fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(index);
        self.tab_subscriptions.remove(&tab.entity_id());
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        if self.tabs.is_empty() {
            window.set_window_title("rogg");
        } else {
            self.activate(self.active, window, cx);
        }
        cx.notify();
    }

    pub fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        let next = if forward { (self.active + 1) % n } else { (self.active + n - 1) % n };
        self.activate(next, window, cx);
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    /// Band handle drag: puts the band's top edge at window `y`.
    pub fn drag_band(&mut self, y: Pixels, cx: &mut Context<Self>) {
        let (Some(split), Some(tab)) = (self.measure.split_at(y), self.active_tab()) else { return };
        tab.update(cx, |t, cx| {
            t.split = split;
            cx.notify();
        });
    }

    /// Sidebar edge drag: the sidebar starts at the window's left edge, so its width is the pointer's x.
    pub fn drag_sidebar(&mut self, x: Pixels, cx: &mut Context<Self>) {
        self.sidebar_width = x.clamp(px(size::SIDEBAR_MIN_W), px(size::SIDEBAR_MAX_W));
        cx.notify();
    }

    /// History dialog pick: restores the entry's terms, states, flags and context on the active tab.
    pub fn restore_history(&mut self, index: usize, cx: &mut Context<Self>) {
        let spec = cx.try_global::<HistoryStore>().and_then(|h| h.entries().get(index)).map(|e| e.to_spec());
        if let (Some(spec), Some(tab)) = (spec, self.active_tab()) {
            let doc = tab.read(cx).doc.clone();
            let _ = doc.update(cx, |d, cx| d.apply_spec(spec, cx));
        }
        self.close_dialog(cx);
    }

    pub fn set_fixed_sidebar(&mut self, sidebar: FixedSidebar, cx: &mut Context<Self>) {
        self.fixed_sidebar = Some(sidebar);
        cx.notify();
    }

    fn open_files(&self, cx: &App) -> Vec<OpenFile> {
        self.tabs
            .iter()
            .map(|tab| {
                let t = tab.read(cx);
                let d = t.doc.read(cx);
                OpenFile {
                    tab: tab.entity_id(),
                    title: d.title(),
                    path: d.source.path().map(Path::to_path_buf),
                    bytes: d.text().map(|text| text.bytes().len() as u64),
                    failed: matches!(d.load, LoadState::Failed { .. }),
                }
            })
            .collect()
    }

    fn sidebar_groups(&self, files: &[OpenFile]) -> Vec<SidebarGroup> {
        match &self.fixed_sidebar {
            Some(fixed) => fixed
                .groups
                .iter()
                .map(|g| SidebarGroup {
                    label: g.label.clone(),
                    entries: g.entries.iter().map(|e| SidebarEntry { tab: files.iter().find(|f| f.title == e.label).map(|f| f.tab), ..e.clone() }).collect(),
                })
                .collect(),
            None => sidebar_model::group_open_files(files),
        }
    }

    fn active_footer(&self, cx: &App) -> Option<SidebarFooter> {
        let tab = self.active_tab()?;
        let d = tab.read(cx).doc.read(cx);
        let mut lines: Vec<SharedString> = vec![d.source.path().and_then(Path::parent).map(|p| p.display().to_string()).unwrap_or_else(|| "In memory".into()).into()];
        if let Some(text) = d.text() {
            let n = text.line_count();
            lines.push(format!("{} {} · {}", group_digits(n), noun(n, "line", "lines"), crate::format::format_size(text.bytes().len() as u64)).into());
        }
        Some(SidebarFooter { title: d.title(), lines })
    }

    pub(crate) fn sidebar(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        if !self.sidebar_visible {
            return None;
        }
        let files = self.open_files(cx);
        let groups = self.sidebar_groups(&files);
        let failed: HashSet<EntityId> = files.iter().filter(|f| f.failed).map(|f| f.tab).collect();
        let active = self.active_tab().map(|t| t.entity_id());
        let (rows, actions) = sidebar_model::rows_with_failures(&groups, active, &self.collapsed, &failed);
        let file_count: u64 = groups.iter().map(|g| g.entries.len() as u64).sum();
        let (count, footer, recent) = match &self.fixed_sidebar {
            Some(fixed) => (fixed.count.clone(), Some(fixed.footer.clone()), fixed.recent.clone()),
            None => (format!("{file_count} {}", noun(file_count, "file", "files")).into(), self.active_footer(cx), recent_items(cx)),
        };
        let actions = Rc::new(actions);
        Some(
            Sidebar::new()
                .width(self.sidebar_width)
                .count(count)
                .rows(rows)
                .recent(recent)
                .footer(footer)
                .on_row(act_ix(this, move |ws, i, window, cx| {
                    if let Some(action) = actions.get(i).cloned() {
                        ws.sidebar_action(action, window, cx);
                    }
                }))
                .on_recent(act_ix(this, |ws, i, window, cx| ws.open_recent(i, window, cx)))
                .into_any_element(),
        )
    }

    fn sidebar_action(&mut self, action: RowAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            RowAction::ToggleGroup(label) => {
                if !self.collapsed.remove(&label) {
                    self.collapsed.insert(label);
                }
                cx.notify();
            }
            RowAction::Activate(id) => {
                if let Some(index) = self.tabs.iter().position(|t| t.entity_id() == id) {
                    self.activate(index, window, cx);
                }
            }
            RowAction::Open(source, init) => {
                self.open_source(source, init, window, cx);
            }
            RowAction::Nothing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_files_skips_hidden_binary_and_nested() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.log"), "text").unwrap();
        std::fs::write(dir.path().join("a.log"), "text").unwrap();
        std::fs::write(dir.path().join(".hidden"), "text").unwrap();
        std::fs::write(dir.path().join("image.png"), [137, 80, 78, 71, 0, 0]).unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        let names: Vec<String> = folder_files(dir.path()).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a.log", "b.log"]);
    }
}
```

Replace `crates/rogg-app/src/workspace/mod.rs` with:

```rust
//! The main window view: title bar, tabs, sidebar, dialogs, toasts and the app's actions.

pub mod chrome;

use crate::actions::{self, ActivateTab, CloseTab, Dismiss, NextTab, OpenFile, OpenFolder, OpenRecent, PrevTab, ShowShortcuts, ToggleSidebar};
use crate::args::Args;
use crate::document::Source;
use crate::history::HistoryStore;
use crate::recent::RecentStore;
use crate::sidebar_model::FixedSidebar;
use crate::tab::{Tab, TabEvent, TabInit};
use crate::tab_view::{self, TabBody};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, DragMoveEvent, Entity, EntityId, ExternalPaths, FocusHandle, Pixels, ScrollHandle, SharedString, Stateful, Subscription,
    Window, div, px,
};
use rogg_ui::dialogs::{self, ModalLayer};
use rogg_ui::empty;
use rogg_ui::layout::{BandDrag, LayoutMeasure, MainWindowLayout, MainWindowParts, SidebarDrag, SingleParts};
use rogg_ui::theme::size;
use rogg_ui::titlebar::{TabItem, TitleBar};
use rogg_ui::toast::ToastStack;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

/// How long a toast stays up.
const TOAST_TIME: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub enum Dialog {
    Shortcuts,
    AllTabs,
    History,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    pub sidebar_visible: bool,
    pub sidebar_width: Pixels,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) tabs: Vec<Entity<Tab>>,
    pub(crate) active: usize,
    tab_subscriptions: HashMap<EntityId, Vec<Subscription>>,
    pub(crate) measure: Rc<LayoutMeasure>,
    pub toasts: Vec<(u64, SharedString)>,
    next_toast: u64,
    traffic_lights: bool,
    pub(crate) tab_scroll: ScrollHandle,
    pub(crate) collapsed: HashSet<SharedString>,
    pub(crate) fixed_sidebar: Option<FixedSidebar>,
}

/// A `Fn(&mut Window, &mut App)` callback that runs `f` on the workspace.
pub(crate) fn act(this: &Entity<Workspace>, f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| f(ws, window, cx))
}

/// Like `act`, for callbacks carrying an index.
pub(crate) fn act_ix(
    this: &Entity<Workspace>,
    f: impl Fn(&mut Workspace, usize, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| f(ws, i, window, cx))
}

impl Workspace {
    pub fn new(args: Args, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut ws = Self {
            focus,
            sidebar_visible: true,
            sidebar_width: px(size::SIDEBAR_W),
            dialog: None,
            tabs: Vec::new(),
            active: 0,
            tab_subscriptions: HashMap::new(),
            measure: Rc::new(LayoutMeasure::default()),
            toasts: Vec::new(),
            next_toast: 0,
            traffic_lights: false,
            tab_scroll: ScrollHandle::new(),
            collapsed: HashSet::new(),
            fixed_sidebar: None,
        };
        if cx.try_global::<HistoryStore>().is_some_and(|h| h.recovered) {
            ws.toast("Filter history was unreadable and has been reset; the old file was kept as history.json.bak", cx);
        }
        if cx.try_global::<RecentStore>().is_some_and(|r| r.recovered) {
            ws.toast("Recent files were unreadable and have been reset; the old file was kept as recent.json.bak", cx);
        }
        ws.open_paths(args.paths, window, cx);
        ws
    }

    pub fn tabs(&self) -> &[Entity<Tab>] {
        &self.tabs
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> Option<Entity<Tab>> {
        self.tabs.get(self.active).cloned()
    }

    pub fn open_source(&mut self, source: Source, init: TabInit, window: &mut Window, cx: &mut Context<Self>) -> Entity<Tab> {
        let tab = cx.new(|cx| Tab::new(source, init, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&tab, window, |ws, _, ev: &TabEvent, window, cx| ws.on_tab_event(ev, window, cx)),
            cx.observe(&tab, |_, _, cx| cx.notify()),
        ];
        self.tab_subscriptions.insert(tab.entity_id(), subscriptions);
        self.tabs.push(tab.clone());
        self.activate(self.tabs.len() - 1, window, cx);
        tab
    }

    pub fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else { return };
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        let title = tab.read(cx).title(cx);
        window.set_window_title(&title);
        cx.notify();
    }

    pub fn toast(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push((id, message.into()));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TIME).await;
            this.update(cx, |ws, cx| ws.dismiss_toast(id, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    pub fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|(t, _)| *t != id);
        cx.notify();
    }

    fn on_tab_event(&mut self, event: &TabEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TabEvent::Toast(message) => self.toast(message.clone(), cx),
            TabEvent::ShowHistory => self.open_dialog(Dialog::History, cx),
        }
    }

    /// Draw the traffic lights ourselves (headless captures have no native ones).
    pub fn set_traffic_lights(&mut self, on: bool, cx: &mut Context<Self>) {
        self.traffic_lights = on;
        cx.notify();
    }

    pub fn open_dialog(&mut self, dialog: Dialog, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    /// Escape: closes a dialog, else cancels the expression editor.
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        } else if let Some(tab) = self.active_tab() {
            tab.update(cx, |t, cx| {
                if t.editing.is_some() {
                    t.cancel_edit(window, cx);
                }
            });
        }
    }

    fn titlebar(&self, this: &Entity<Self>, cx: &App) -> TitleBar {
        let items = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let t = tab.read(cx);
                let d = t.doc.read(cx);
                let tooltip = t.labels.tooltip.clone().or_else(|| d.source.path().map(|p| SharedString::from(p.display().to_string()))).unwrap_or_else(|| d.title());
                TabItem { label: d.title(), tooltip, active: i == self.active }
            })
            .collect();
        TitleBar::new(items)
            .sidebar_width(self.sidebar_width)
            .traffic_lights(self.traffic_lights)
            .tab_scroll(self.tab_scroll.clone())
            .on_select_tab(act_ix(this, |ws, i, window, cx| ws.activate(i, window, cx)))
            .on_close_tab(act_ix(this, |ws, i, window, cx| ws.close_tab(i, window, cx)))
            .on_toggle_sidebar(act(this, |ws, _, cx| ws.toggle_sidebar(cx)))
            .on_all_tabs(act(this, |ws, _, cx| ws.open_dialog(Dialog::AllTabs, cx)))
            .on_shortcuts(act(this, |ws, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .on_settings(act(this, |ws, _, cx| ws.toast("Settings arrive in a later version", cx)))
    }

    fn dialog_element(&self, dialog: &Dialog, this: &Entity<Self>, cx: &App) -> AnyElement {
        let close = act(this, |ws, _, cx| ws.close_dialog(cx));
        match dialog {
            Dialog::Shortcuts => dialogs::shortcuts(&actions::shortcut_rows())
                .description("Keyboard shortcuts for the full log, the filter line and the filtered log.")
                .on_close(close)
                .into_any_element(),
            Dialog::AllTabs => {
                let tabs: Vec<(SharedString, bool)> = self.tabs.iter().enumerate().map(|(i, t)| (t.read(cx).title(cx), i == self.active)).collect();
                let pick = act_ix(this, |ws, i, window, cx| {
                    ws.activate(i, window, cx);
                    ws.close_dialog(cx);
                });
                dialogs::all_tabs(&tabs, Rc::new(pick)).description("All open tabs, including those outside the visible header.").on_close(close).into_any_element()
            }
            Dialog::History => {
                let entries: Vec<(SharedString, SharedString)> = cx
                    .try_global::<HistoryStore>()
                    .map(|h| h.entries().iter().map(|e| (SharedString::from(e.expression()), SharedString::from(e.to_spec().summary()))).collect())
                    .unwrap_or_default();
                dialogs::history_picker(&entries, Rc::new(act_ix(this, |ws, i, _, cx| ws.restore_history(i, cx))))
                    .description("Restore an expression together with its term states and matching options.")
                    .on_close(close)
                    .into_any_element()
            }
        }
    }

    /// The modal layer and the toasts, over everything else. The container has no handlers, so it does not catch
    /// the mouse.
    fn overlay(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        let modal = self.dialog.as_ref().map(|d| ModalLayer::new(self.dialog_element(d, this, cx)).on_dismiss(act(this, |ws, _, cx| ws.close_dialog(cx))));
        if modal.is_none() && self.toasts.is_empty() {
            return None;
        }
        let dismiss = this.clone();
        Some(
            div()
                .absolute()
                .inset_0()
                .children(modal)
                .when(!self.toasts.is_empty(), |d| {
                    d.child(ToastStack::new(self.toasts.clone()).on_dismiss(move |id, _, cx| dismiss.update(cx, |ws, cx| ws.dismiss_toast(id, cx))))
                })
                .into_any_element(),
        )
    }

    fn welcome(&self, this: &Entity<Self>, cx: &App) -> AnyElement {
        let open_file: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFile), cx));
        let open_folder: rogg_ui::Callback = Rc::new(|window: &mut Window, cx: &mut App| window.dispatch_action(Box::new(OpenFolder), cx));
        let recent = match &self.fixed_sidebar {
            Some(fixed) => fixed.recent.clone(),
            None => chrome::recent_items(cx),
        };
        empty::welcome(recent, open_file, open_folder, Rc::new(act_ix(this, |ws, i, window, cx| ws.open_recent(i, window, cx)))).into_any_element()
    }

    fn root(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("root")
            .size_full()
            .key_context(actions::WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|ws, _: &Dismiss, window, cx| ws.dismiss(window, cx)))
            .on_action(cx.listener(|ws, _: &ShowShortcuts, _, cx| ws.open_dialog(Dialog::Shortcuts, cx)))
            .on_action(cx.listener(|ws, _: &OpenFile, window, cx| ws.prompt_open(false, window, cx)))
            .on_action(cx.listener(|ws, _: &OpenFolder, window, cx| ws.prompt_open(true, window, cx)))
            .on_action(cx.listener(|ws, action: &OpenRecent, window, cx| ws.open_recent(action.index, window, cx)))
            .on_action(cx.listener(|ws, _: &CloseTab, window, cx| {
                let index = ws.active;
                ws.close_tab(index, window, cx);
            }))
            .on_action(cx.listener(|ws, _: &NextTab, window, cx| ws.cycle_tab(true, window, cx)))
            .on_action(cx.listener(|ws, _: &PrevTab, window, cx| ws.cycle_tab(false, window, cx)))
            .on_action(cx.listener(|ws, action: &ActivateTab, window, cx| ws.activate(action.index, window, cx)))
            .on_action(cx.listener(|ws, _: &ToggleSidebar, _, cx| ws.toggle_sidebar(cx)))
            .on_drop(cx.listener(|ws, paths: &ExternalPaths, window, cx| ws.open_paths(paths.paths().to_vec(), window, cx)))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<BandDrag>, _, cx| ws.drag_band(e.event.position.y, cx)))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<SidebarDrag>, _, cx| ws.drag_sidebar(e.event.position.x, cx)))
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep keys flowing to the window's actions when the focused element went away (an applied editor, a
        // closed dialog's input).
        if window.focused(cx).is_none() {
            cx.defer_in(window, |ws, window, cx| ws.focus.focus(window, cx));
        }
        let this = cx.entity();
        let titlebar = self.titlebar(&this, cx).into_any_element();
        let overlay = self.overlay(&this, cx);
        let sidebar = self.sidebar(&this, cx);
        let content = match self.active_tab() {
            None => MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: None, body: self.welcome(&this, cx), status: None, overlay })
                .sidebar_resizable(true)
                .into_any_element(),
            Some(tab) => {
                let parts = tab_view::parts(&tab, window, cx);
                match parts.body {
                    TabBody::Panes { top_header, top_body, band, bottom_header, bottom_body, split } => MainWindowLayout::new(MainWindowParts {
                        titlebar,
                        sidebar,
                        info_bar: parts.info_bar,
                        top_header,
                        top_body,
                        band,
                        bottom_header,
                        bottom_body,
                        status: parts.status,
                        overlay,
                        split,
                    })
                    .measure(self.measure.clone())
                    .sidebar_resizable(true)
                    .into_any_element(),
                    TabBody::Placeholder(body) => {
                        MainWindowLayout::single(SingleParts { titlebar, sidebar, info_bar: Some(parts.info_bar), body, status: Some(parts.status), overlay })
                            .sidebar_resizable(true)
                            .into_any_element()
                    }
                }
            }
        };
        self.root(cx).child(content)
    }
}
```

In `crates/rogg-app/src/lib.rs`: add `pub mod format;`, `pub mod recent;` and `pub mod sidebar_model;`; install the store and build the menus from it:

```rust
pub fn init_with(cx: &mut App, paths: AppPaths) {
    gpui_kit::init(cx);
    RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
    install_globals(cx, paths);
    let recent = cx.global::<recent::RecentStore>().entries().to_vec();
    cx.set_menus(menus::app_menus(&recent));
}

pub fn install_globals(cx: &mut App, paths: AppPaths) {
    cx.set_global(document::EngineConfig::default());
    cx.set_global(history::HistoryStore::load(paths.history.clone()));
    cx.set_global(recent::RecentStore::load(paths.recent.clone()));
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass (Tasks 6–11 tests too), no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): tab management, sidebar of open files, recent files, folders, drops and split handles

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 13: Keyboard — the full keymap, focus cycling, Go to line, theme and density

**Files:**
- Create: `crates/rogg-app/src/workspace/keys.rs`, `crates/rogg-app/tests/keyboard.rs`
- Modify: `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/lib.rs`

**Interfaces:**
- Consumes: Task 6 actions/keymap; Task 11 `Tab` methods; Task 12 `Workspace`; `rogg_ui::dialogs::go_to_line`.
- Produces (used by Tasks 14, 15):
  - `rogg_app::workspace::keys::{ThemeChoice::{System, Light, Dark}, appearance_for(ThemeChoice, WindowAppearance) -> Appearance}`.
  - `Dialog::GoToLine(Entity<InputState>)`.
  - `Workspace::{open_go_to_line(window, cx), set_theme(ThemeChoice, window, cx), set_density(Density, window, cx)}`.
  - `Tab::{focus_find(window, cx), cycle_focus(forward: bool, window, cx)}`.
  - Behaviour (spec §5.9): ⌘F focuses the find field of the last-used pane; ⌘L (and Enter on the focused filter line) opens the expression editor, Enter applies, Escape cancels; ⌥⌘R/⌥⌘C toggle regex/case; ⌘B pins the selected line; ⌘G opens Go to line (Enter selects and centres the line in the full log; an unknown line shows a toast and keeps the dialog); Return reveals the selected line in the full log; Tab/⇧Tab cycle focus full log → filter line → filtered log; ⇧⌘F toggles follow; ⌘\\ toggles the sidebar; View → Wrap toggles the last-used pane; View → Theme (System follows macOS appearance live) and Density reinstall the theme; Window → Minimize/Zoom. `init_with` starts with the system appearance.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/keyboard.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, Entity, TestAppContext, point, px};
use rogg_app::actions::{DensityComfortable, ThemeDark, ThemeLight};
use rogg_app::args::Args;
use rogg_app::document::Pane;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density};

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn open_one(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &LINES);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    (dir, w, ws, tab)
}

fn type_text(cx: &mut TestAppContext, w: AnyWindowHandle, text: &str) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn focused(cx: &mut TestAppContext, w: AnyWindowHandle, tab: &Entity<Tab>, i: usize) -> bool {
    cx.update_window(w, |_, window, cx| tab.read(cx).focus[i].is_focused(window)).unwrap()
}

fn click_at(cx: &mut TestAppContext, w: AnyWindowHandle, id: &'static str, x: f32, y: f32) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at(id, point(px(x), px(y)), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn cmd_l_edits_and_enter_applies(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    common::press(cx, w, "cmd-l");
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()));
    type_text(cx, w, "error");
    common::press(cx, w, "enter");
    assert!(tab.read_with(cx, |t, _| t.editing.is_none()));
    assert_eq!(tab.read_with(cx, |t, cx| t.doc.read(cx).filter.spec.expression()), "error");
}

#[gpui_kit::test]
fn escape_cancels_editing_and_clears_the_error(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "alt-cmd-r");
    assert!(doc.read_with(cx, |d, _| d.filter.spec.regex));
    common::press(cx, w, "cmd-l");
    type_text(cx, w, " || (");
    common::press(cx, w, "enter");
    assert!(common::exists(cx, w, "filter-error"));
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()));
    common::press(cx, w, "escape");
    assert!(tab.read_with(cx, |t, _| t.editing.is_none()));
    assert!(doc.read_with(cx, |d, _| d.filter.error.is_none()));
    assert!(!common::exists(cx, w, "filter-error"));
    assert!(common::exists(cx, w, "token-0") && !common::exists(cx, w, "token-1"), "the applied chips are back");
}

#[gpui_kit::test]
fn cmd_b_pins_and_return_reveals(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    click_at(cx, w, "log-top", 300., 4. + 23. + 10.); // line 2
    common::press(cx, w, "cmd-b");
    assert!(doc.read_with(cx, |d, _| d.filter.pins.contains(&1)));
    doc.update(cx, |d, cx| d.set_expression("done", cx)).unwrap();
    common::press(cx, w, "shift-cmd-f");
    assert!(!tab.read_with(cx, |t, _| t.follow));
    // filtered rows: line 2 (pinned), gap, line 8
    click_at(cx, w, "log-bottom", 300., 4. + 23. + 17. + 10.);
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(2), "follow is off");
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(8), "Return reveals in the full log");
}

#[gpui_kit::test]
fn tab_cycles_focus_between_the_three_regions(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    // a filter, so the filtered pane shows a log view (not the no-matches state) that can take focus
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 0));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 2));
    common::press(cx, w, "shift-tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "enter");
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()), "Enter on the filter line edits");
}

#[gpui_kit::test]
fn go_to_line_selects_and_centres(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open_one(cx);
    common::press(cx, w, "cmd-g");
    assert!(common::exists(cx, w, "dialog"));
    type_text(cx, w, "5");
    common::press(cx, w, "enter");
    assert!(!common::exists(cx, w, "dialog"));
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(5));
    common::press(cx, w, "cmd-g");
    type_text(cx, w, "99");
    common::press(cx, w, "enter");
    assert!(common::exists(cx, w, "dialog"), "an unknown line keeps the dialog");
    assert!(ws.read_with(cx, |ws, _| !ws.toasts.is_empty()));
}

#[gpui_kit::test]
fn flag_find_and_sidebar_shortcuts(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "alt-cmd-c");
    assert!(doc.read_with(cx, |d, _| d.filter.spec.case_sensitive));
    common::press(cx, w, "cmd-f");
    let find = tab.read_with(cx, |t, _| t.find[Pane::Full.index()].clone());
    assert!(cx.update_window(w, |_, window, cx| find.read(cx).focus_handle().is_focused(window)).unwrap());
    common::press(cx, w, "escape");
    common::press(cx, w, "cmd-\\");
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible));
}

#[gpui_kit::test]
fn theme_and_density_actions_reinstall_the_theme(cx: &mut TestAppContext) {
    let (_dir, w, _, _) = open_one(cx);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ThemeDark), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().appearance), Appearance::Dark);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ThemeLight), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().appearance), Appearance::Light);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(DensityComfortable), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().density), Density::Comfortable);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test keyboard`
Expected: most tests fail — the keys are bound (Task 6) but nothing handles `EditFilters`, `ToggleRegex`, `TogglePin`, `GoToLine`, `FocusNext`, `ThemeDark`, …

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/workspace/keys.rs`:

```rust
//! The window's keyboard actions (spec §5.9): focus cycling, Go to line, theme and density.

use super::{Dialog, Workspace};
use crate::actions::*;
use crate::document::Document;
use crate::tab::Tab;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Stateful, Window, WindowAppearance};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, RoggTheme};

/// View → Theme. System follows the macOS appearance live (spec §6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

pub fn appearance_for(choice: ThemeChoice, window: WindowAppearance) -> Appearance {
    match choice {
        ThemeChoice::Dark => Appearance::Dark,
        ThemeChoice::Light => Appearance::Light,
        ThemeChoice::System => match window {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
            _ => Appearance::Light,
        },
    }
}

/// Registers the keyboard actions on the workspace root.
pub(super) fn bind(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    root.on_action(cx.listener(|ws, _: &Find, window, cx| ws.with_tab(cx, |t, cx| t.focus_find(window, cx))))
        .on_action(cx.listener(|ws, _: &EditFilters, window, cx| ws.with_tab(cx, |t, cx| t.start_edit(window, cx))))
        .on_action(cx.listener(|ws, _: &ToggleRegex, _, cx| ws.with_doc(cx, |d, cx| d.toggle_regex(cx))))
        .on_action(cx.listener(|ws, _: &ToggleCase, _, cx| ws.with_doc(cx, |d, cx| d.toggle_case(cx))))
        .on_action(cx.listener(|ws, _: &TogglePin, _, cx| ws.with_tab(cx, |t, cx| t.pin_selected(cx))))
        .on_action(cx.listener(|ws, _: &RevealInFullLog, _, cx| ws.with_tab(cx, |t, cx| t.reveal_selected(cx))))
        .on_action(cx.listener(|ws, _: &ToggleFollow, _, cx| ws.with_tab(cx, |t, cx| t.toggle_follow(cx))))
        .on_action(cx.listener(|ws, _: &ToggleWrap, _, cx| {
            ws.with_tab(cx, |t, cx| {
                let pane = t.last_pane;
                t.toggle_wrap(pane, cx);
            })
        }))
        .on_action(cx.listener(|ws, _: &FocusNext, window, cx| ws.with_tab(cx, |t, cx| t.cycle_focus(true, window, cx))))
        .on_action(cx.listener(|ws, _: &FocusPrev, window, cx| ws.with_tab(cx, |t, cx| t.cycle_focus(false, window, cx))))
        .on_action(cx.listener(|ws, _: &GoToLine, window, cx| ws.open_go_to_line(window, cx)))
        .on_action(cx.listener(|ws, _: &ThemeSystem, window, cx| ws.set_theme(ThemeChoice::System, window, cx)))
        .on_action(cx.listener(|ws, _: &ThemeLight, window, cx| ws.set_theme(ThemeChoice::Light, window, cx)))
        .on_action(cx.listener(|ws, _: &ThemeDark, window, cx| ws.set_theme(ThemeChoice::Dark, window, cx)))
        .on_action(cx.listener(|ws, _: &DensityCompact, window, cx| ws.set_density(Density::Compact, window, cx)))
        .on_action(cx.listener(|ws, _: &DensityComfortable, window, cx| ws.set_density(Density::Comfortable, window, cx)))
        .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
        .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
}

impl Workspace {
    fn with_tab(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Tab, &mut Context<Tab>)) {
        if let Some(tab) = self.active_tab() {
            tab.update(cx, f);
        }
    }

    fn with_doc(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Document, &mut Context<Document>)) {
        if let Some(tab) = self.active_tab() {
            let doc = tab.read(cx).doc.clone();
            doc.update(cx, f);
        }
    }

    /// ⌘G: a dialog with a line-number field; Enter selects and centres the line in the full log.
    pub fn open_go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab().is_none() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Line number, e.g. 120492"));
        input.update(cx, |i, cx| i.focus(window, cx));
        self.goto_subscription = Some(cx.subscribe_in(&input, window, |ws, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().to_string();
                ws.go_to_line(&text, window, cx);
            }
        }));
        self.open_dialog(Dialog::GoToLine(input), cx);
    }

    fn go_to_line(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        match text.trim().replace(',', "").parse::<u64>() {
            Ok(number) => {
                if tab.update(cx, |t, cx| t.go_to_line(number, cx)) {
                    self.close_dialog(cx);
                    tab.read(cx).focus[0].clone().focus(window, cx);
                } else {
                    self.toast(format!("Line {number} is not in this log"), cx);
                }
            }
            Err(_) => self.toast("Type a line number", cx),
        }
    }

    pub fn set_theme(&mut self, choice: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = choice;
        self.apply_theme(window, cx);
    }

    pub fn set_density(&mut self, density: Density, window: &mut Window, cx: &mut Context<Self>) {
        self.density = density;
        self.apply_theme(window, cx);
    }

    /// Installs the theme for the current choice and density (only if it changes).
    pub(crate) fn apply_theme(&self, window: &Window, cx: &mut App) {
        let appearance = appearance_for(self.theme, window.appearance());
        let current = cx.rogg();
        if current.appearance != appearance || current.density != self.density {
            RoggTheme::new(appearance, self.density).install(cx);
        }
    }
}
```

In `crates/rogg-app/src/workspace/mod.rs`:
- add `pub mod keys;` after `pub mod chrome;`, and imports `use gpui_kit::component::input::InputState;`, `use keys::ThemeChoice;`, `use rogg_ui::theme::{ActiveRoggTheme, Density};`;
- add the variant `GoToLine(Entity<InputState>),` to `Dialog`;
- add fields to `Workspace`:

```rust
    pub(crate) theme: ThemeChoice,
    pub(crate) density: Density,
    goto_subscription: Option<Subscription>,
    _appearance_subscription: Subscription,
```

  and initialise them in `new` (inside the struct literal): `theme: ThemeChoice::System, density: cx.rogg().density, goto_subscription: None, _appearance_subscription: cx.observe_window_appearance(window, |ws, window, cx| { if ws.theme == ThemeChoice::System { ws.apply_theme(window, cx); } }),`;
- in `close_dialog`, also drop the Go to line subscription: add `self.goto_subscription = None;` before `cx.notify();`;
- add the arm to `dialog_element`:

```rust
            Dialog::GoToLine(input) => dialogs::go_to_line(input.clone()).description("Selects the line in the full log.").on_close(close).into_any_element(),
```

- in `root`, pass the finished chain through `keys::bind`: change the function to end with `keys::bind(div().id("root") … .on_drag_move(… SidebarDrag …), cx)` — i.e. wrap the whole existing builder expression in `keys::bind(…, cx)`.

In `crates/rogg-app/src/tab.rs`, add to `impl Tab`:

```rust
    /// ⌘F: focuses the find field of the pane used last.
    pub fn focus_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.find[self.last_pane.index()].clone();
        input.update(cx, |i, cx| i.focus(window, cx));
    }

    /// Tab / ⇧Tab: full log → filter line → filtered log (spec §5.9).
    pub fn cycle_focus(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.focus.iter().position(|f| f.contains_focused(window, cx));
        let next = match current {
            None => 0,
            Some(i) if forward => (i + 1) % 3,
            Some(i) => (i + 2) % 3,
        };
        self.focus[next].focus(window, cx);
        match next {
            0 => self.last_pane = Pane::Full,
            2 => self.last_pane = Pane::Filtered,
            _ => {}
        }
        cx.notify();
    }
```

In `crates/rogg-app/src/lib.rs`, start with the system appearance in `init_with`:

```rust
pub fn init_with(cx: &mut App, paths: AppPaths) {
    gpui_kit::init(cx);
    let appearance = workspace::keys::appearance_for(workspace::keys::ThemeChoice::System, cx.window_appearance());
    RoggTheme::new(appearance, Density::Compact).install(cx);
    install_globals(cx, paths);
    let recent = cx.global::<recent::RecentStore>().entries().to_vec();
    cx.set_menus(menus::app_menus(&recent));
}
```

and drop the now-unused `Appearance` import if the compiler reports it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo clippy -p rogg-app --all-targets`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): keyboard actions, focus cycling, go to line, theme and density

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 14: `rogg --demo` — the reference state in the real app, sample data shared with the gallery

**Files:**
- Create: `crates/rogg-app/src/demo.rs`, `crates/rogg-app/tests/demo.rs`
- Modify: `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-gallery/Cargo.toml`, `crates/rogg-gallery/src/replica/data.rs`, `crates/rogg-gallery/src/fixtures.rs`

**Interfaces:**
- Consumes: Task 11 `TabInit`/`TabLabels`, Task 12 `FixedSidebar`/`SidebarEntry`/`SidebarGroup`/`set_fixed_sidebar`.
- Produces (used by Task 15):
  - `rogg_app::demo::{FIRST_LINE_NUMBER = 120480, BUNDLE, FILES, TABS, SAMPLES, RECENT, FOOTER_TITLE, FOOTER_LINES, STATUS_NOTE, PROFILE, PINNED_SAMPLE = 7, SELECTED_SAMPLE = 12, line_text(usize) -> String, sample_bytes() -> Arc<Vec<u8>>, reference_spec() -> FilterSpec, reference_history() -> Vec<FilterSpec>, source(&str) -> Source, init(&str) -> TabInit, sidebar() -> FixedSidebar, open(&mut Workspace, &mut Window, &mut Context<Workspace>)}`.
  - `rogg --demo` opens the reference bundle: sidebar `incident-2026-10-05.zip` with its 10 members and sizes, the reference Recent entries and footer; tabs api.log, worker.log, gateway.log (api.log active) holding the 40 sample lines numbered from 120480; filters `error` IN, `timeout` IN, `healthcheck` NOT, `retry` OFF; line 120487 pinned; line 120492 selected in both panes (the footer shows no selection until the user selects, like the reference); profile label `Services`, note `UTF-8 · LF · sample excerpt`, header count `40 sample lines`, breadcrumb `incident-2026-10-05.zip › api.log`. Clicking another member opens it with the same sample.
  - The gallery's mockup replica now imports `SAMPLES`, `FIRST_LINE_NUMBER`, `line_text`, `BUNDLE` and `FILES` from `rogg_app::demo` (spec §3.2: nothing duplicated between the gallery and the app).

- [ ] **Step 1: Write the failing test**

Create `crates/rogg-app/tests/demo.rs`:

```rust
mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;
use rogg_app::demo;
use rogg_app::document::Pane;

#[gpui_kit::test]
fn demo_reproduces_the_reference_state(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args { demo: true, paths: vec![] });
    let titles: Vec<String> = ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect());
    assert_eq!(titles, demo::TABS);
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 0);
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.read_with(cx, |d, _| {
        assert_eq!(d.number_base(), demo::FIRST_LINE_NUMBER);
        let stats = d.filter.stats.clone().unwrap();
        assert_eq!((stats.matched, stats.total_lines, stats.pinned, stats.pinned_unmatched, stats.context_lines), (6, 40, 1, 1, 0));
        assert_eq!(d.filter.view.shown_lines().as_ref(), &vec![7, 12, 15, 20, 23, 28, 34]);
    });
    let selected = demo::FIRST_LINE_NUMBER + demo::SELECTED_SAMPLE;
    tab.read_with(cx, |t, cx| {
        assert_eq!(t.state(Pane::Full).read(cx).selected(), Some(selected));
        assert_eq!(t.state(Pane::Filtered).read(cx).selected(), Some(selected));
        assert_eq!(t.selection(), None, "the footer shows no selection until the user selects");
    });
    assert!(common::exists(cx, w, "tree-row-10"), "bundle folder + 10 members");
    assert!(common::exists(cx, w, "recent-1"));
    common::click(cx, w, "tree-row-4");
    assert_eq!(ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).title(cx).to_string()), "auth.log");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 4);
}

#[test]
fn sample_lines_follow_the_reference_formula() {
    assert_eq!(demo::line_text(0), "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29");
    assert_eq!(demo::sample_bytes().iter().filter(|&&b| b == b'\n').count(), 39);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test demo`
Expected: compile error — `rogg_app::demo` not found.

- [ ] **Step 3: Implement**

Create `crates/rogg-app/src/demo.rs`. Move into it, verbatim, the `SAMPLES` array (all 40 rows) and `line_text` from `crates/rogg-gallery/src/replica/data.rs`, and `BUNDLE` and `FILES` from `crates/rogg-gallery/src/fixtures.rs`; add the rest as shown:

```rust
//! `rogg --demo` (spec §9 M2): the reference mockup's bundle, tabs, sample lines, filters, pin and selection, for
//! the visual review against the reference. The gallery's mockup replica uses the same data.

use crate::document::{DocumentInit, Source};
use crate::sidebar_model::{FixedSidebar, SidebarEntry, SidebarGroup};
use crate::tab::{TabInit, TabLabels};
use crate::workspace::Workspace;
use gpui_kit::{Context, SharedString, Window};
use rogg_core::filter::{FilterSpec, Term, TermState};
use rogg_ui::sidebar::{RecentItem, SidebarFooter};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

/// Display number of the first sample line.
pub const FIRST_LINE_NUMBER: u64 = 120480;

// pub const BUNDLE: &str = …;            moved verbatim from crates/rogg-gallery/src/fixtures.rs
// pub const FILES: [(&str, &str); 10] = …; moved verbatim from crates/rogg-gallery/src/fixtures.rs
// pub const SAMPLES: [(&str, &str, &str); 40] = …; moved verbatim from crates/rogg-gallery/src/replica/data.rs
// pub fn line_text(i: usize) -> String { … }      moved verbatim from crates/rogg-gallery/src/replica/data.rs

/// The tabs open in the reference.
pub const TABS: [&str; 3] = ["api.log", "worker.log", "gateway.log"];
/// The reference's Recent section: (label, is folder).
pub const RECENT: [(&str, bool); 2] = [("staging / logs", true), ("deployment-10-04.log", false)];
pub const FOOTER_TITLE: &str = "Support bundle";
pub const FOOTER_LINES: [&str; 2] = ["10 members · 8.4 GB expanded", "Members open individually"];
pub const STATUS_NOTE: &str = "UTF-8 · LF · sample excerpt";
pub const PROFILE: &str = "Services";
/// Sample index pinned in the reference (line 120487).
pub const PINNED_SAMPLE: u64 = 7;
/// Sample index selected in the reference (line 120492).
pub const SELECTED_SAMPLE: u64 = 12;

/// The 40 sample lines joined with `\n` (no trailing newline), shared by every demo tab.
pub fn sample_bytes() -> Arc<Vec<u8>> {
    static BYTES: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    BYTES.get_or_init(|| Arc::new((0..SAMPLES.len()).map(line_text).collect::<Vec<_>>().join("\n").into_bytes())).clone()
}

/// `error` IN, `timeout` IN, `healthcheck` NOT, `retry` OFF.
pub fn reference_spec() -> FilterSpec {
    FilterSpec {
        terms: vec![Term::include("error"), Term::include("timeout"), Term::new("healthcheck", TermState::Exclude), Term::new("retry", TermState::Off)],
        ..Default::default()
    }
}

/// The reference's history dialog entries, newest first (the gallery seeds a capture's history with them).
pub fn reference_history() -> Vec<FilterSpec> {
    vec![reference_spec(), FilterSpec::from_expression("request_id=req_8f29 || connection reset")]
}

pub fn source(name: &str) -> Source {
    Source::Memory { name: SharedString::from(name.to_string()), bytes: sample_bytes() }
}

pub fn init(name: &str) -> TabInit {
    TabInit {
        doc: DocumentInit { number_base: FIRST_LINE_NUMBER, spec: reference_spec(), pins: BTreeSet::from([PINNED_SAMPLE]) },
        selection: Some(FIRST_LINE_NUMBER + SELECTED_SAMPLE),
        labels: TabLabels {
            crumbs: Some(vec![BUNDLE.into(), SharedString::from(name.to_string())]),
            profile: Some(PROFILE.into()),
            note: Some(STATUS_NOTE.into()),
            full_count: Some("40 sample lines".into()),
            tooltip: Some(format!("{name} · {BUNDLE}").into()),
        },
    }
}

pub fn sidebar() -> FixedSidebar {
    FixedSidebar {
        groups: vec![SidebarGroup {
            label: BUNDLE.into(),
            entries: FILES.iter().map(|(name, size)| SidebarEntry { label: (*name).into(), size: (*size).into(), tab: None, open: Some((source(name), init(name))) }).collect(),
        }],
        count: "10 files".into(),
        footer: SidebarFooter { title: FOOTER_TITLE.into(), lines: FOOTER_LINES.iter().map(|l| SharedString::from(*l)).collect() },
        recent: RECENT.iter().map(|(label, is_folder)| RecentItem { label: (*label).into(), is_folder: *is_folder, missing: false }).collect(),
    }
}

/// Opens the demo in `ws`: the bundle sidebar and the three reference tabs, api.log active.
pub fn open(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.set_fixed_sidebar(sidebar(), cx);
    for name in TABS {
        ws.open_source(source(name), init(name), window, cx);
    }
    ws.activate(0, window, cx);
}
```

(Replace the four `// …moved verbatim…` comment lines with the moved items; keep `line_text`'s body and the `SAMPLES` rows exactly as they are in the gallery.)

Add `pub mod demo;` to `crates/rogg-app/src/lib.rs`. In `Workspace::new` (`crates/rogg-app/src/workspace/mod.rs`), right after `ws.open_paths(args.paths, window, cx);`, add:

```rust
        if args.demo {
            crate::demo::open(&mut ws, window, cx);
        }
```

Make the gallery use the shared data:
- `crates/rogg-gallery/Cargo.toml`: add `rogg-app = { path = "../rogg-app" }` to `[dependencies]`;
- `crates/rogg-gallery/src/replica/data.rs`: delete `FIRST_LINE_NUMBER`, `SAMPLES` and `line_text` and add `pub use rogg_app::demo::{FIRST_LINE_NUMBER, SAMPLES, line_text};` (keep `STAMP_LEN`, `Term`, `Filtered`, the filter helpers and the tests — they now exercise the shared data);
- `crates/rogg-gallery/src/fixtures.rs`: delete `BUNDLE` and `FILES` and add `pub use rogg_app::demo::{BUNDLE, FILES};`; make `recent()` and `sidebar_footer()` build from `rogg_app::demo::{RECENT, FOOTER_TITLE, FOOTER_LINES}`:

```rust
pub fn recent() -> Vec<RecentItem> {
    rogg_app::demo::RECENT.iter().map(|(label, is_folder)| RecentItem { label: (*label).into(), is_folder: *is_folder, missing: false }).collect()
}

pub fn sidebar_footer() -> SidebarFooter {
    SidebarFooter { title: rogg_app::demo::FOOTER_TITLE.into(), lines: rogg_app::demo::FOOTER_LINES.iter().map(|l| SharedString::from(*l)).collect() }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app && cargo test -p rogg-gallery && cargo clippy -p rogg-app -p rogg-gallery --all-targets && just visual-check`
Expected: all pass; the replica's golden images are unchanged (same data, same rendering).

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-app crates/rogg-gallery Cargo.lock
git commit -m "feat(app): rogg --demo with the reference bundle, sharing sample data with the gallery

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: Visual harness for the real app, and the M2 gate report

**Files:**
- Modify: `crates/rogg-gallery/src/gallery.rs`, `crates/rogg-gallery/src/pages.rs`, `crates/rogg-gallery/src/capture.rs`, `crates/rogg-gallery/src/main.rs`, `crates/rogg-gallery/tests/capture.rs`, `tools/visual-diff/config.mjs`, `tools/visual-diff/gallery.mjs`, `justfile`
- Create: `docs/superpowers/reviews/2026-10-07-m2-visual.md`

**Interfaces:**
- Consumes: Task 14 `rogg_app::demo`; Task 6 `install_globals`, `AppPaths`; Task 12/13 `Workspace::{set_traffic_lights, open_dialog}`, `Dialog`.
- Produces:
  - Gallery page `Page::App` (slug `app`, title `Real app (demo)`) hosting a real `rogg_app::workspace::Workspace` opened with `--demo` semantics and drawn traffic lights; `rogg-gallery --capture app [--dialog all-tabs|shortcuts|history]` captures it at the reference size with geometry (rows of the active tab's panes as `row-top-N`, `row-bottom-N`, `gap-bottom-N`). The capture runs with app state in a temp dir whose history is seeded with `demo::reference_history()`; `--dialog profile|appearance` on the app page is an error (those dialogs are M4).
  - `ROGG_VISUAL_PAGE=app` makes `tools/visual-diff` compare the reference with the app page (output in `target/visual-app/`); `just visual-app` runs it and opens the report. The replica pipeline (`just visual`, `just visual-check`) is unchanged.
  - `docs/superpowers/reviews/2026-10-07-m2-visual.md`: the measured numbers for the M2 joint review.

- [ ] **Step 1: Write the failing capture test**

Append to `crates/rogg-gallery/tests/capture.rs`:

```rust
#[test]
fn captures_the_real_app_page_with_rows_and_history() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("app.png");
    let json = dir.join("app.json");
    capture(&["--capture", "app", "--dialog", "history", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (2880, 1800));
    let g: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    for key in ["titlebar", "tab-shell-2", "token-3", "row-top-0", "row-bottom-1", "gap-bottom-0", "statusbar", "dialog", "tree-row-10", "history-pick-1"] {
        assert!(g["elements"].get(key).is_some(), "missing geometry key {key}");
    }
}

#[test]
fn app_page_rejects_dialogs_it_does_not_have() {
    let status = Command::new(env!("CARGO_BIN_EXE_rogg-gallery"))
        .args(["--capture", "app", "--dialog", "profile", "--out", "/tmp/never.png"])
        .status()
        .expect("spawn gallery");
    assert!(!status.success());
}
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-gallery --test capture`
Expected: both new tests fail (`unknown page \`app\``).

- [ ] **Step 2: Add the app page to the gallery**

In `crates/rogg-gallery/src/gallery.rs`:
- add `App` to `Page` (after `Replica`), to `Page::ALL` (size 11), `slug` → `"app"`, `title` → `"Real app (demo)"`;
- add the field `pub app: Option<Entity<rogg_app::workspace::Workspace>>,` to `GalleryView`, set in `new` with `app: (page == Page::App).then(|| Self::new_app(window, cx)),`, and add:

```rust
    /// The real app window in demo mode, with drawn traffic lights (headless windows have no native ones).
    pub fn new_app(window: &mut Window, cx: &mut Context<Self>) -> Entity<rogg_app::workspace::Workspace> {
        cx.new(|cx| {
            let mut ws = rogg_app::workspace::Workspace::new(rogg_app::args::Args { demo: true, paths: vec![] }, window, cx);
            ws.set_traffic_lights(true, cx);
            ws
        })
    }

    /// The two log panels whose rows go into capture geometry: the replica's, or the app's active tab.
    pub fn log_states(&self, cx: &gpui_kit::App) -> [(&'static str, Entity<LogViewState>); 2] {
        if self.page == Page::App
            && let Some(tab) = self.app.as_ref().and_then(|a| a.read(cx).active_tab())
        {
            let t = tab.read(cx);
            return [("top", t.full.clone()), ("bottom", t.filtered.clone())];
        }
        let r = self.replica.read(cx);
        [("top", r.top.clone()), ("bottom", r.bottom.clone())]
    }
```

- in `for_capture`, open the dialog on the app page instead of the replica when `page == Page::App`:

```rust
    pub fn for_capture(page: Page, dialog: Option<crate::replica::ReplicaDialog>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use crate::replica::ReplicaDialog;
        use rogg_app::workspace::Dialog;
        let this = Self::new(page, false, window, cx);
        match (page, dialog, this.app.clone()) {
            (Page::App, Some(d), Some(app)) => {
                let dialog = match d {
                    ReplicaDialog::AllTabs => Dialog::AllTabs,
                    ReplicaDialog::Shortcuts => Dialog::Shortcuts,
                    ReplicaDialog::History => Dialog::History,
                    ReplicaDialog::Profile | ReplicaDialog::Appearance => unreachable!("rejected by CaptureOptions::parse"),
                };
                app.update(cx, |ws, cx| ws.open_dialog(dialog, cx));
            }
            (_, Some(d), _) => this.replica.update(cx, |r, cx| r.open_dialog(d, cx)),
            _ => {}
        }
        this
    }
```

In `crates/rogg-gallery/src/pages.rs`, render the page (create the workspace on first visit in the interactive gallery):

```rust
        Page::App => {
            let app = match view.app.clone() {
                Some(app) => app,
                None => {
                    let app = GalleryView::new_app(window, cx);
                    view.app = Some(app.clone());
                    app
                }
            };
            if view.chrome {
                div().w(px(1440.)).h(px(900.)).flex().border_1().border_color(p.border).rounded(px(10.)).overflow_hidden().child(app).into_any_element()
            } else {
                div().size_full().child(app).into_any_element()
            }
        }
```

(remove the `let _ = window;` line at the top of `render` since `window` is now used).

- [ ] **Step 3: Capture support**

In `crates/rogg-gallery/src/capture.rs`:
- in `CaptureOptions::parse`, after parsing `dialog`, reject the M4 dialogs on the app page:

```rust
        if page == Page::App && matches!(dialog, Some(ReplicaDialog::Profile | ReplicaDialog::Appearance)) {
            return Err(format!("dialog `{}` is not available on the app page", dialog.unwrap().slug()));
        }
```

- in `collect_geometry`, replace the `for (prefix, state) in [("top", replica.read(cx).top.clone()), …]` loop header (and the `let replica = …` line) with `for (prefix, state) in view.read(cx).log_states(cx) {`;
- in `run`, before `cx.update(|cx| { gpui_kit::init(cx); … })`, prepare app state for the app page, and install it after the theme:

```rust
    let app_state = if opts.page == Page::App { Some(seed_app_state()?) } else { None };
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(appearance, density).install(cx);
        if let Some(dir) = &app_state {
            rogg_app::install_globals(cx, rogg_app::paths::AppPaths::under(dir.path()));
        }
    });
```

  (this replaces the existing `cx.update(|cx| { gpui_kit::init(cx); RoggTheme::new(appearance, density).install(cx); });` block; keep `app_state` alive until the end of `run`), and add:

```rust
/// A temp dir for the app page's state, with the reference's history entries, so the history dialog shows them.
fn seed_app_state() -> anyhow::Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    let mut history = rogg_core::history::History::default();
    for spec in rogg_app::demo::reference_history().iter().rev() {
        history.record(rogg_core::history::HistoryEntry::from_spec(spec));
    }
    history.save(&dir.path().join("history.json"))?;
    Ok(dir)
}
```

- `crates/rogg-gallery/Cargo.toml`: add `rogg-core = { path = "../rogg-core" }` and `tempfile.workspace = true` to `[dependencies]`.

In `crates/rogg-gallery/src/main.rs`, after `RoggTheme::new(…).install(cx);`, install app globals for the interactive app page with state under the temp dir (never the user's real history):

```rust
            let state = std::env::temp_dir().join(format!("rogg-gallery-{}", std::process::id()));
            rogg_app::install_globals(cx, rogg_app::paths::AppPaths::under(&state));
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-gallery && cargo clippy -p rogg-gallery --all-targets`
Expected: all pass, including the two new capture tests and `every_page_renders_in_every_theme` (now with the app page); no warnings.

- [ ] **Step 4: The visual-diff page switch**

In `tools/visual-diff/config.mjs`, replace the `OUT` and `DIALOGS` exports with:

```js
// `ROGG_VISUAL_PAGE=app` compares the reference with the real app (rogg --demo) instead of the gallery replica.
export const PAGE = process.env.ROGG_VISUAL_PAGE ?? 'replica';
export const OUT = path.join(ROOT, PAGE === 'replica' ? 'target/visual' : `target/visual-${PAGE}`);

// Reference <dialog> id → gallery --dialog slug, and the pages that have it (profile and settings are M4 in the app).
export const DIALOGS = [
  { html: 'tabs-dialog', gallery: 'all-tabs', pages: ['replica', 'app'] },
  { html: 'profile-dialog', gallery: 'profile', pages: ['replica'] },
  { html: 'settings-dialog', gallery: 'appearance', pages: ['replica'] },
  { html: 'shortcuts-dialog', gallery: 'shortcuts', pages: ['replica', 'app'] },
  { html: 'history-dialog', gallery: 'history', pages: ['replica', 'app'] },
].filter((d) => d.pages.includes(PAGE));
```

In `tools/visual-diff/gallery.mjs`, import `PAGE` from `./config.mjs` and pass it instead of the literal: `['--capture', PAGE, '--theme', …]`.

Append to `justfile`:

```just
visual-app:
    cd tools/visual-diff && ROGG_VISUAL_PAGE=app npm run all
    open target/visual-app/report.html
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cd tools/visual-diff && ROGG_VISUAL_PAGE=app npm run all`
Expected: reference and app captures for 4 variants × (main + 3 dialogs) and a report at `target/visual-app/report.html`. If the sandbox refuses Playwright, run `cargo run -p rogg-gallery -- --capture app --out target/visual-app/manual.png` instead, report that the comparison could not run, and leave Step 5's table for the controller to fill.

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; just visual-check`
Expected: the replica's golden check still passes.

- [ ] **Step 5: Record the gate report**

Create `docs/superpowers/reviews/2026-10-07-m2-visual.md` from the run's `target/visual-app/results.json`:

```markdown
# M2 visual review — real app vs reference

Command: `just visual-app` (gallery `--capture app`, i.e. `rogg --demo` in a headless window, 1440×900 @2x)

| Capture | Differing pixels | Largest geometry deltas (selector: px) |
|---|---|---|
| dark-compact | <percent> | <top 3> |
| dark-comfortable | <percent> | <top 3> |
| light-compact | <percent> | <top 3> |
| light-comfortable | <percent> | <top 3> |
| dark-compact--all-tabs | <percent> | <top 3> |
| dark-compact--shortcuts | <percent> | <top 3> |
| dark-compact--history | <percent> | <top 3> |

## Expected differences
- Traffic lights: drawn in the capture; the real window shows the native ones at (13, 14).
- Shortcuts dialog: lists the real keymap (the reference shows six proposed shortcuts).
- Filtered pane header: has `Open as tab` (spec §5.3; also in the approved replica).
- Fonts: glyph-shape differences are expected (spec §7.2).

## For the joint review
<anything else the report shows, largest first, each marked "fix" or "accept?">
```

Fill every `<…>` with the measured values (the largest deviations first). The M2 gate itself is the user's approval of this report and the live app (`just demo`) — the plan's work ends at producing them.

- [ ] **Step 6: Run the whole workspace suite and commit**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: all pass, no warnings.

```bash
git add crates/rogg-gallery tools/visual-diff justfile docs/superpowers/reviews/2026-10-07-m2-visual.md Cargo.lock
git commit -m "test(visual): compare the real app with the reference for the M2 gate

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Deferred from M2 (by milestone)

- **M3:** recursive folder trees, archives, `.gz`, stdin, live follow with pause (`Space`), stick-to-bottom scrolling, truncate/rotate handling and re-mapping (SIGBUS on external truncation until then), cache eviction, sweeping stale `*.partial` transcode files, progressive scrolling of a file that is still indexing (M1 indexes ~1 GB in ~150 ms, so the loading state is brief).
- **M4:** highlighting profiles (the info-bar Profile button shows a toast; rows use the built-in decoration of `decorate.rs`), Settings (title-bar button shows a toast; theme/density choices are not persisted yet), session restore and `.rogg` files, Export (`⌘E`), Open filtered as tab (`⇧⌘T`), statistics popover, per-panel find flags (regex/case) in a find popover.
- **Not planned for M2 (rulings):** per-button keyboard focus rings and revealing a tab's close button on keyboard focus (Tab cycles the three regions of spec §5.9; buttons are mouse targets), the context menu's "selection" is the word under the pointer (line-level selection has no text range), copy-with-numbers uses `number<TAB>line`, hover changes keep calling `window.refresh()` (M0 carry-over; measured cheap), `just visual` bootstrapping a fresh clone (`npm ci`, `npx playwright install chromium`) stays a manual step documented in the M0 note.
