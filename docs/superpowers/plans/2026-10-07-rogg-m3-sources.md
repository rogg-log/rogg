# rogg M3 — Sources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open everything a support bundle is made of — folders (recursive), `.gz` files, zip / tar / tar.gz / tgz archives and archives nested in archives — through a cache with LRU eviction, and follow logs that grow: live follow with pause, stdin and FIFOs, and truncation / rotation handling; plus the M2 → M3 carry-over (off-thread view rebuilds and the other deferred fixes).

**Architecture:** `rogg-core` gains the source layer: crash-safe mappings (a SIGBUS guard, so a file truncated under its mapping reads zeros instead of killing the app), a `Cache` (origin-keyed entries, atomic writes, leases, LRU eviction, sweeps of dead writers' files), archive sniffing / listing / extraction and streaming gunzip, a `SourceRef` (file, or member of a possibly nested archive) with `open_source` that materialises any source into a mapped `LogText` (a `.gz` grows while it decompresses), a `SourceTree` for the sidebar and a bounded recursive folder scan, `LogText::refresh` (append-only growth that indexes only the new bytes), tail filter scans whose merge is property-tested against a full scan, a `Follower` thread (FSEvents through `notify` plus a polling fallback) and stdin / FIFO capture into the cache. `rogg-app` wires them in: documents open any source on a background thread with stage labels, follow growth incrementally (tail scans, tail searches, off-thread view rebuilds), reload with a toast on truncation or replacement, and tabs get a live-follow control (`Pause` / `Paused +1,204`, `Space`) with stick-to-bottom scrolling; the sidebar shows folders and archives as a lazily listed tree. `rogg-ui` gains a log-view tail mode, tree-row notes and gallery states for all of it.

**Tech Stack:** Rust 1.99.0 (edition 2024), gpui-pre 0.3.8 through gpui-kit =0.7.1, new pins `libc =0.2.190`, `flate2 =1.1.10`, `zip =8.6.0` (no default features, `deflate-flate2`), `tar =0.4.46`, `notify =8.2.0`; Node + Playwright visual harness (M0).

**Spec:** `docs/superpowers/specs/2026-10-06-rogg-design.md` (§3.3, §3.4, §4.1–4.5, §4.13, §5.3, §5.6, §5.7, §5.9, §6.3, §7.1, §8, §9 M3). Carry-over folded in: `docs/superpowers/notes/2026-10-07-m2-carryover-to-m3.md` (all items except the horizontal-scrollbar hit zone, which waits for the user's R3 decision), the M1 carry-over items marked M3 (SIGBUS on truncation, stale `*.partial` sweep) and the M3 line of the M2 plan's deferred list.

## Global Constraints

- macOS only (Apple Silicon); no cross-platform abstractions (spec §2).
- GUI: `gpui-pre =0.3.8` through `gpui-kit =0.7.1` only — import gpui types from `gpui_kit`, never add a `gpui` dependency. Crates using `gpui_kit::actions!` / `#[derive(gpui_kit::Action)]` keep `extern crate gpui_kit as gpui;` at their root.
- Toolchain: Rust 1.99.0 pinned by `rust-toolchain.toml`. Prefix every cargo/just command with `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH";`. Build and run the app with `-p rogg-app`.
- Dependencies: only these new third-party crates, pinned exactly in the root `Cargo.toml` `[workspace.dependencies]` and used with `.workspace = true`: `libc = "=0.2.190"`, `flate2 = "=1.1.10"`, `zip = { version = "=8.6.0", default-features = false, features = ["deflate-flate2"] }`, `tar = "=0.4.46"`, `notify = "=8.2.0"` (spec §3.4). Nothing else is added.
- Boundaries (spec §3.2): `rogg-core` has no gpui dependency and owns everything that touches files, archives, the cache, watching and stdin; `rogg-ui` renders plain data and never touches files or the engine; `rogg-app` is the only crate that depends on both.
- Concurrency (spec §3.3): extraction, decompression, archive listing, folder scanning, indexing, filtering and searching of inputs larger than `EngineConfig::sync_bytes`, file watching and view rebuilds above `EngineConfig::view_sync_work` run off the UI thread. The UI polls job channels at most once per frame (`POLL_INTERVAL` = 16 ms). A superseded or closed job is cancelled and its results never reach the UI. Never join or block on a worker from the UI thread.
- M1 engine contracts still hold: a `FilterJob` channel that closes without `Finished`/`Cancelled` means the worker died; never drop a `FilterJob` on a rayon thread (retire it on a short-lived std thread); slice raw lines to 64 KB before decoding or highlighting; `FilteredView::build`'s `excluded` closure is `|n| filter.is_excluded(text.line(n))`.
- Never panic on input data (spec §4.13): corrupt or truncated archives, unsupported zip methods, damaged gzip streams, non-UTF-8 names, files that shrink, vanish or are replaced, and FIFOs without a writer all become typed `CoreError`s or UI states. A damaged archive member fails only its own tab or tree node.
- Cache (spec §4.2): every cache file is written to a writer-unique `<dst>.<pid>.<n>.partial` and renamed on success (`Cache::write_entry`, `cache::partial_path`); every cache entry a document reads is leased (`CacheLease`) and never evicted while leased.
- Live follow (spec §4.5): only appended bytes are indexed and scanned; a shrink or a replaced file reloads with the toast `File was truncated or replaced — reloaded.`; tests that wait on watched files poll with a deadline (`common::wait_until`, or a `recv_timeout` loop of at least 10 s) and never rely on a fixed sleep alone.
- Visuals: only `rogg_ui::theme` tokens; no gpui-kit default colors. The replica and app goldens are **not** re-approved in M3 — the R3 horizontal-scrollbar decision is pending with the user — so `just visual-check` may fail only on the 9 replica captures it failed on at the end of M2. New gallery states go on the component pages (Sidebar, Chrome, Empty states, Controls), which are not golden-checked.
- Display numbers: a file's first line is number 1; engine line indices are 0-based; convert only at the `Tab`/rows boundary.
- Copy: loading labels `Extracting <name>`, `Decompressing <name>`, `Converting UTF-16 to UTF-8`, `Indexing`; footer progress `Decompressing N%` / `Filtering N%`; live control `Pause` (following) / `Paused` with badge `+1,204` (approved M0 gallery state); sidebar footer `N members · X expanded` + `Members open individually` for an archive, `N files · X` for a folder; reload toast `File was truncated or replaced — reloaded.`
- Commit messages are conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

The five failure modes most likely to bite a user that the feature tests would not otherwise exercise. Each names the owning task, which carries a test for it.

1. **A log truncated in place while it is open** (`logrotate copytruncate`, `> app.log`) — no crash (no SIGBUS) while rows are painted or a scan runs; the tab reloads with the toast and shows the new content. (Task 1: `truncation_under_a_mapping_reads_zeros_and_slots_are_released`; Task 11: `truncation_reloads_with_a_toast_and_drops_pins`.)
2. **A damaged or unsupported member in an otherwise good archive** — only that member's tab shows the error state with its message; the archive still lists and its other members open. (Task 4: `a_damaged_member_fails_alone`; Task 14: `a_damaged_member_fails_only_its_own_tab`.)
3. **Lines appended while a long initial scan is still running** — every appended line is scanned exactly once: no lost or duplicated matches, exact per-term hit counts, and an unfinished last line that is completed later is matched on its full text. (Task 7: `tail_scan_merge_equals_a_full_scan` property; Task 11: `appends_during_a_running_scan_are_scanned_once`.)
4. **Opening a huge or looping folder tree** (a home folder, a symlink to an ancestor) — the scan stays bounded (depth 4, at most 10,000 files, symlinked folders not followed), runs off the UI thread and says when it stopped early. (Task 5: `scan_is_bounded_and_does_not_follow_symlinked_folders`; Task 14: `huge_folders_list_the_first_files_with_a_toast`.)
5. **The cache over its limit while big entries are open** — entries in use are never evicted, entries of closed tabs are, and partial files of a still-running writer survive the startup sweep. (Task 2: `evicts_oldest_unleased_first`, `sweeps_files_of_dead_processes_only`; Task 10: `open_members_are_leased_until_their_tab_closes`.)

---

## File Structure

```
Cargo.toml                         MOD  pins: libc, flate2, zip, tar, notify
justfile                           MOD  bundle / pipe recipes
crates/rogg-core/
  Cargo.toml                       MOD  libc, flate2, zip, tar, notify
  src/mapping.rs                   NEW  SafeMap + SIGBUS guard (truncation never crashes)
  src/cache.rs                     NEW  Cache, CacheLease, origin_key, partial_path, sweeps, LRU eviction
  src/archive.rs                   NEW  sniff, ArchiveKind, list, extract, gunzip
  src/source.rs                    NEW  SourceRef, open_source, Inflating (progressive gunzip), list_archive
  src/tree.rs                      NEW  SourceTree for the sidebar
  src/folder.rs                    NEW  bounded recursive folder scan
  src/follow.rs                    NEW  Follower (notify + polling): Grew / Truncated / Replaced
  src/stream.rs                    NEW  stdin / FIFO capture into the cache
  src/text.rs                      MOD  SafeMap data, file handle, refresh(), stable_lines(), open_handle()
  src/index.rs                     MOD  ends_with_newline()
  src/engine.rs                    MOD  run_filter_lines, FilterJob::spawn_from, TailBase
  src/search.rs                    MOD  SearchScope::From
  src/encoding.rs                  MOD  partial names from cache::partial_path
  src/recent.rs                    MOD  archive entries remember their member
  src/error.rs                     MOD  CoreError::Archive
  tests/mapping.rs, tests/archive.rs, tests/sources.rs, tests/open_source.rs, tests/growth.rs,
  tests/follow.rs, tests/stream.rs NEW; tests/filter_props.rs MOD (tail merge property)
crates/rogg-genlog/                MOD  bundle writers (zip, tar, tar.gz, gz), damage helper, --bundle CLI
crates/rogg-ui/src/
  logview/state.rs                 MOD  tail mode, extend_rows, at_bottom, TailLeft
  logview/element.rs               MOD  user scrolls leave tail mode
  sidebar.rs                       MOD  folder rows: note + failed
  layout.rs, filter.rs             MOD  (Task 16) drag grab offsets, drag cursor kept during drags
crates/rogg-gallery/src/           MOD  sidebar / chrome / empty-state specimens for the new states
crates/rogg-app/src/
  cache.rs                         NEW  CacheStore global, startup maintenance, evict_soon
  sources.rs                       NEW  workspace source tree state + scan / list jobs
  document/mod.rs                  MOD  Source::{File, Member, Stream, Memory}, open pipeline, follow, reload, events
  document/load.rs                 MOD  OpenJob runs open_source
  document/filter.rs               MOD  tail scans, pins revision, off-thread view rebuilds
  document/search.rs               MOD  tail searches, failed label, run counter
  tab.rs, tab_view.rs              MOD  live follow control, crumbs / tooltips from the source, rows per revision
  sidebar_model.rs                 MOD  tree rows, footer summaries
  workspace/{mod,chrome,keys}.rs   MOD  open routing (folders, archives, FIFOs, stdin), tree actions, Space
  actions.rs, menus.rs, args.rs    MOD  ToggleLive, Hide items, `-` for stdin
  main.rs                          MOD  stdin with data opens a tab
  tests/common/mod.rs              MOD  append / bundle helpers
  tests/sources.rs, tests/live.rs, tests/live_tab.rs, tests/tree.rs, tests/streams.rs, tests/bundle_e2e.rs  NEW
docs/superpowers/notes/2026-10-07-m3-manual-checks.md   NEW  hand checks for the M3 review
```

---
### Task 1: Dependency pins and crash-safe mappings

**Files:**
- Modify: `Cargo.toml` (workspace dependencies), `crates/rogg-core/Cargo.toml`
- Create: `crates/rogg-core/src/mapping.rs`
- Modify: `crates/rogg-core/src/text.rs`, `crates/rogg-core/src/lib.rs`
- Test: `crates/rogg-core/tests/mapping.rs`

**Interfaces:**
- Consumes: `memmap2::Mmap` (M1), `LogText` (M1).
- Produces:
  - `rogg_core::mapping::{SafeMap, MAX_GUARDED: usize = 1024, guarded_count() -> usize}`; `SafeMap::map(&File) -> std::io::Result<SafeMap>` (fails on an empty file), `SafeMap::bytes(&self) -> &[u8]`, `SafeMap::is_guarded(&self) -> bool`.
  - `LogText` maps files through `SafeMap`: reading a mapped file that was truncated returns zero bytes past the new end instead of raising SIGBUS.
  - Workspace pins `libc`, `flate2`, `zip`, `tar`, `notify` (used by later tasks).

- [ ] **Step 1: Add the pins**

In the root `Cargo.toml`, append to `[workspace.dependencies]`:

```toml
libc = "=0.2.190"
flate2 = "=1.1.10"
zip = { version = "=8.6.0", default-features = false, features = ["deflate-flate2"] }
tar = "=0.4.46"
notify = "=8.2.0"
```

In `crates/rogg-core/Cargo.toml`, append to `[dependencies]`:

```toml
libc.workspace = true
flate2.workspace = true
zip.workspace = true
tar.workspace = true
notify.workspace = true
```

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo build -p rogg-core`
Expected: builds (the new crates download and compile; nothing uses them yet).

- [ ] **Step 2: Write the failing test**

Create `crates/rogg-core/tests/mapping.rs` (one test in its own binary, so the global slot count is not disturbed by other tests):

```rust
use rogg_core::CancelToken;
use rogg_core::mapping::{SafeMap, guarded_count};
use rogg_core::text::LogText;
use std::fs::{File, OpenOptions};

#[test]
fn truncation_under_a_mapping_reads_zeros_and_slots_are_released() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shrinking.log");
    std::fs::write(&path, vec![b'x'; 1 << 20]).unwrap();
    let before = guarded_count();
    let map = SafeMap::map(&File::open(&path).unwrap()).unwrap();
    assert!(map.is_guarded());
    assert_eq!(guarded_count(), before + 1);
    // `logrotate copytruncate` / `> app.log`: the file shrinks under the mapping.
    OpenOptions::new().write(true).open(&path).unwrap().set_len(100).unwrap();
    let xs = map.bytes().iter().filter(|&&b| b == b'x').count();
    assert_eq!(xs, 100, "bytes past the new end read as zeros instead of raising SIGBUS");
    assert_eq!(map.bytes()[(1 << 20) - 1], 0);
    drop(map);
    assert_eq!(guarded_count(), before, "dropping a mapping frees its slot");

    // The same through LogText: reading every line of a truncated file does not crash.
    let lines: Vec<u8> = (0..50_000).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
    std::fs::write(&path, &lines).unwrap();
    let text = LogText::open(&path, dir.path(), &CancelToken::new(), |_| {}).unwrap();
    OpenOptions::new().write(true).open(&path).unwrap().set_len(7).unwrap();
    assert_eq!(text.line(0), b"line 0");
    let zero_lines = (0..text.line_count()).filter(|&i| text.line(i).iter().all(|&b| b == 0)).count();
    assert!(zero_lines > 40_000, "lines past the truncation read as zeros ({zero_lines})");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test mapping`
Expected: FAIL to compile — `unresolved import rogg_core::mapping`.

- [ ] **Step 4: Implement `mapping.rs`**

Create `crates/rogg-core/src/mapping.rs`:

```rust
//! Read-only file mappings that survive the file shrinking underneath them (spec §4.5).
//!
//! Reading a mapped page that lies wholly past the end of a truncated file raises SIGBUS. Every `SafeMap` registers
//! its address range in a fixed, lock-free table, and a process-wide SIGBUS handler checks the faulting address
//! against it: inside a registered mapping, the handler maps private zero pages over the rest of that mapping and
//! returns, so the read sees zeros instead of killing the process. Live follow then notices the shrink and reloads
//! the file. A fault anywhere else goes back to the handler that was installed before ours.

use memmap2::Mmap;
use std::fs::File;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Mappings protected at once. A mapping made while every slot is taken still works, unprotected.
pub const MAX_GUARDED: usize = 1024;

struct Slot {
    used: AtomicBool,
    start: AtomicUsize,
    len: AtomicUsize,
}

static SLOTS: [Slot; MAX_GUARDED] =
    [const { Slot { used: AtomicBool::new(false), start: AtomicUsize::new(0), len: AtomicUsize::new(0) } }; MAX_GUARDED];
static PAGE: AtomicUsize = AtomicUsize::new(0);
/// The SIGBUS action the process had before `install` (faults that are not ours go back to it).
static PREVIOUS: OnceLock<libc::sigaction> = OnceLock::new();
static INSTALLED: OnceLock<()> = OnceLock::new();

fn install() {
    INSTALLED.get_or_init(|| {
        // SAFETY: plain libc calls with valid arguments; the handler only reads atomics and calls mmap/sigaction.
        unsafe {
            PAGE.store((libc::sysconf(libc::_SC_PAGESIZE) as usize).max(4096), Ordering::Relaxed);
            let mut previous: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGBUS, std::ptr::null(), &mut previous);
            let _ = PREVIOUS.set(previous);
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_sigbus as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_SIGINFO;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGBUS, &action, std::ptr::null_mut());
        }
    });
}

/// End address of the registered mapping containing `addr`.
fn guarded_end(addr: usize) -> Option<usize> {
    SLOTS.iter().find_map(|s| {
        // `len` is published after `start` and cleared before it, so a non-zero `len` means `start` is valid.
        let len = s.len.load(Ordering::Acquire);
        let start = s.start.load(Ordering::Acquire);
        (len > 0 && addr >= start && addr - start < len).then_some(start + len)
    })
}

extern "C" fn on_sigbus(_signal: libc::c_int, info: *mut libc::siginfo_t, _context: *mut libc::c_void) {
    // SAFETY: the kernel passes a valid `siginfo_t` to an SA_SIGINFO handler.
    let addr = unsafe { (*info).si_addr } as usize;
    if let Some(end) = guarded_end(addr) {
        let page = PAGE.load(Ordering::Relaxed);
        let from = addr & !(page - 1);
        // SAFETY: `from..end` lies inside a live mapping registered by a `SafeMap`, which keeps the range mapped until
        // it unregisters; replacing those pages with private zero pages leaves every address in it readable.
        let mapped = unsafe {
            libc::mmap(from as *mut libc::c_void, end - from, libc::PROT_READ, libc::MAP_FIXED | libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0)
        };
        if mapped != libc::MAP_FAILED {
            return;
        }
    }
    // Not ours (or the remap failed): reinstate the previous action; returning re-runs the faulting access under it.
    if let Some(previous) = PREVIOUS.get() {
        // SAFETY: `previous` is the action the process had before `install`.
        unsafe { libc::sigaction(libc::SIGBUS, previous, std::ptr::null_mut()) };
    }
}

fn register(start: usize, len: usize) -> Option<usize> {
    let slot = SLOTS.iter().position(|s| s.used.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok())?;
    SLOTS[slot].start.store(start, Ordering::Release);
    SLOTS[slot].len.store(len, Ordering::Release);
    Some(slot)
}

fn unregister(slot: usize) {
    SLOTS[slot].len.store(0, Ordering::Release);
    SLOTS[slot].start.store(0, Ordering::Release);
    SLOTS[slot].used.store(false, Ordering::Release);
}

/// Mappings currently protected (tests).
pub fn guarded_count() -> usize {
    SLOTS.iter().filter(|s| s.len.load(Ordering::Acquire) > 0).count()
}

/// A read-only mapping of a whole file, protected against the file shrinking (see module docs).
pub struct SafeMap {
    map: Mmap,
    slot: Option<usize>,
}

impl SafeMap {
    /// Maps all of `file` read-only. Fails for an empty file (there is nothing to map).
    pub fn map(file: &File) -> std::io::Result<SafeMap> {
        install();
        // SAFETY: a read-only mapping that rogg never writes through. Another process writing the file only changes
        // what is displayed; a truncation is caught by the SIGBUS guard above.
        let map = unsafe { Mmap::map(file) }?;
        let slot = register(map.as_ptr() as usize, map.len());
        Ok(SafeMap { map, slot })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    /// False when every guard slot was taken at mapping time (the mapping then works unprotected).
    pub fn is_guarded(&self) -> bool {
        self.slot.is_some()
    }
}

impl Drop for SafeMap {
    fn drop(&mut self) {
        // Unregister before `map` unmaps the range.
        if let Some(slot) = self.slot.take() {
            unregister(slot);
        }
    }
}
```

In `crates/rogg-core/src/lib.rs`, add `pub mod mapping;` (keep the module list alphabetical).

- [ ] **Step 5: Map `LogText` files through `SafeMap`**

In `crates/rogg-core/src/text.rs`:

1. Replace the module doc's second paragraph ("Files are memory-mapped. The mapping assumes …") with:

```rust
//! Files are memory-mapped through `mapping::SafeMap`, so a file truncated while open reads zeros past its new end
//! instead of crashing; live follow (`LogText::refresh`, spec §4.5) notices the shrink and reloads.
```

2. Replace `use memmap2::Mmap;` with `use crate::mapping::SafeMap;`, and the `Data` enum and its `bytes`:

```rust
enum Data {
    Mapped(SafeMap),
    Owned(Vec<u8>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Mapped(m) => m.bytes(),
            Data::Owned(v) => v,
        }
    }
}
```

3. Replace `map_file` with:

```rust
fn map_file(path: &Path) -> Result<Data, CoreError> {
    let file = File::open(path).map_err(CoreError::io(path))?;
    let len = file.metadata().map_err(CoreError::io(path))?.len();
    if len == 0 {
        return Ok(Data::Owned(Vec::new()));
    }
    SafeMap::map(&file).map(Data::Mapped).map_err(CoreError::io(path))
}
```

4. In the `LogText` struct doc, delete the sentence "Opened files are memory-mapped and the mapping assumes the file is not truncated while mapped; truncating it externally is undefined behavior until live follow (M3) re-maps on change."

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core`
Expected: PASS, including `truncation_under_a_mapping_reads_zeros_and_slots_are_released` and every M1 test.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/rogg-core
git commit -m "feat(core): crash-safe file mappings and M3 dependency pins

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The cache — origin keys, atomic entries, leases, sweeps and LRU eviction

**Files:**
- Create: `crates/rogg-core/src/cache.rs`
- Modify: `crates/rogg-core/src/text.rs` (UTF-16 conversions keyed through the cache), `crates/rogg-core/src/encoding.rs` (partial names), `crates/rogg-core/src/lib.rs`
- Test: unit tests in `cache.rs`; existing `encoding.rs` and `tests/text.rs` tests keep passing

**Interfaces:**
- Consumes: `CoreError::{Io, io}`, `libc::kill` (Task 1 pin).
- Produces (`rogg_core::cache`):
  - `DEFAULT_LIMIT_BYTES: u64` (20 GB), `STREAMS_DIR: &str = "streams"`
  - `fnv1a(&[&[u8]]) -> u64`; `origin_key(path: &Path, members: &[&str], purpose: &str) -> std::io::Result<u64>`
  - `partial_path(dst: &Path) -> PathBuf` (`<dst>.<pid>.<n>.partial`)
  - `Cache::new(dir: impl Into<PathBuf>, limit: u64) -> Arc<Cache>`; `dir() -> &Path`, `limit() -> u64`, `entry(key: u64, ext: &str) -> PathBuf` (`<dir>/<key:016x>.<ext>`), `streams_dir() -> PathBuf`, `lease(self: &Arc<Self>, &Path) -> CacheLease`, `is_leased(&Path) -> bool`, `write_entry(&self, dst: &Path, write: impl FnOnce(&mut File) -> Result<(), CoreError>) -> Result<(), CoreError>`, `sweep_partials() -> usize`, `sweep_streams() -> usize`, `evict() -> Evicted`, `evict_to(limit: u64) -> Evicted`
  - `CacheLease::path(&self) -> &Path` (dropping it releases the entry)
  - `Evicted { removed: usize, freed: u64, remaining: u64 }`

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/cache.rs` with only its tests:

```rust
//! The cache directory (spec §4.2).

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn set_mtime(path: &Path, t: SystemTime) {
        File::options().append(true).open(path).unwrap().set_modified(t).unwrap();
    }

    fn dead_pid() -> i32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id() as i32;
        child.wait().unwrap();
        pid
    }

    #[test]
    fn keys_follow_origin_members_and_purpose() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.log");
        std::fs::write(&a, "one").unwrap();
        let base = origin_key(&a, &[], "utf8").unwrap();
        assert_eq!(base, origin_key(&a, &[], "utf8").unwrap(), "stable");
        assert_ne!(base, origin_key(&a, &["x.log"], "utf8").unwrap(), "member");
        assert_ne!(base, origin_key(&a, &[], "member").unwrap(), "purpose");
        assert_ne!(origin_key(&a, &["ab", "c"], "m").unwrap(), origin_key(&a, &["a", "bc"], "m").unwrap(), "chain parts stay apart");
        std::fs::write(&a, "one two").unwrap();
        assert_ne!(base, origin_key(&a, &[], "utf8").unwrap(), "a changed origin gets a new key");
        assert!(origin_key(&dir.path().join("missing"), &[], "utf8").is_err());
    }

    #[test]
    fn write_entry_renames_on_success_and_leaves_nothing_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("cache"), DEFAULT_LIMIT_BYTES);
        let ok = cache.entry(1, "member");
        cache.write_entry(&ok, |f| f.write_all(b"abc").map_err(CoreError::io(&ok))).unwrap();
        assert_eq!(std::fs::read(&ok).unwrap(), b"abc");
        let bad = cache.entry(2, "member");
        let err = cache.write_entry(&bad, |f| {
            f.write_all(b"half").unwrap();
            Err(CoreError::Cancelled)
        });
        assert!(matches!(err, Err(CoreError::Cancelled)));
        assert!(!bad.exists());
        let names: Vec<String> = std::fs::read_dir(cache.dir()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".partial")), "{names:?}");
        assert_eq!(cache.entry(0xab, "log").file_name().unwrap(), "00000000000000ab.log");
    }

    #[test]
    fn evicts_oldest_unleased_first() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), 250);
        let t0 = SystemTime::now() - Duration::from_secs(3600);
        let paths: Vec<PathBuf> = ["a", "b", "c"].iter().map(|n| dir.path().join(format!("{n}.member"))).collect();
        for (i, p) in paths.iter().enumerate() {
            std::fs::write(p, [0u8; 100]).unwrap();
            set_mtime(p, t0 + Duration::from_secs(10 * i as u64));
        }
        std::fs::write(dir.path().join("x.member.1.0.partial"), [0u8; 100]).unwrap();
        // 400 bytes in total (the partial counts but is never removed): b and c go, oldest first; a is leased.
        let lease = cache.lease(&paths[0]);
        assert!(cache.is_leased(&paths[0]));
        let out = cache.evict();
        assert_eq!(out, Evicted { removed: 2, freed: 200, remaining: 200 });
        assert!(paths[0].exists() && !paths[1].exists() && !paths[2].exists());
        assert!(dir.path().join("x.member.1.0.partial").exists());
        drop(lease);
        assert!(!cache.is_leased(&paths[0]));
        assert_eq!(cache.evict_to(0).removed, 1, "released entries are evictable");
        assert!(!paths[0].exists());
    }

    #[test]
    fn leases_count_and_refresh_the_lru_time() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
        let p = dir.path().join("e.log");
        std::fs::write(&p, "x").unwrap();
        set_mtime(&p, SystemTime::now() - Duration::from_secs(3600));
        let one = cache.lease(&p);
        let two = cache.lease(&p);
        let age = SystemTime::now().duration_since(std::fs::metadata(&p).unwrap().modified().unwrap()).unwrap();
        assert!(age < Duration::from_secs(60), "a lease marks the entry as recently used");
        drop(one);
        assert!(cache.is_leased(&p), "still leased by the second holder");
        drop(two);
        assert!(!cache.is_leased(&p));
        assert_eq!(cache.lease(&dir.path().join("not-yet.log")).path(), dir.path().join("not-yet.log"), "an entry may be leased before it exists");
    }

    #[test]
    fn sweeps_files_of_dead_processes_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
        let (dead, me) = (dead_pid(), std::process::id());
        std::fs::write(dir.path().join(format!("k.utf8.{dead}.0.partial")), "x").unwrap();
        std::fs::write(dir.path().join(format!("k.utf8.{me}.1.partial")), "x").unwrap();
        std::fs::write(dir.path().join("k.utf8"), "x").unwrap();
        assert_eq!(cache.sweep_partials(), 1);
        assert!(dir.path().join(format!("k.utf8.{me}.1.partial")).exists(), "a running writer's partial survives");
        assert!(dir.path().join("k.utf8").exists());
        std::fs::create_dir_all(cache.streams_dir()).unwrap();
        std::fs::write(cache.streams_dir().join(format!("stream-{dead}-0.log")), "x").unwrap();
        std::fs::write(cache.streams_dir().join(format!("stream-{me}-0.log")), "x").unwrap();
        assert_eq!(cache.sweep_streams(), 1);
        assert!(cache.streams_dir().join(format!("stream-{me}-0.log")).exists());
    }

    #[test]
    fn partial_paths_are_unique_and_name_their_writer() {
        let dst = Path::new("/c/k.utf8");
        let (a, b) = (partial_path(dst), partial_path(dst));
        assert_ne!(a, b);
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(partial_owner(&name), Some(std::process::id() as i32));
        assert_eq!(stream_owner("stream-42-7.log"), Some(42));
        assert_eq!(partial_owner("k.utf8"), None);
    }
}
```

In `crates/rogg-core/src/lib.rs`, add `pub mod cache;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --lib cache`
Expected: FAIL to compile — `origin_key`, `Cache`, … not found.

- [ ] **Step 3: Implement the cache**

Replace the module doc line of `crates/rogg-core/src/cache.rs` and add the implementation above the tests:

```rust
//! The cache directory (spec §4.2): UTF-8 conversions, extracted archive members and decompressed `.gz` files, keyed
//! by their origin, written atomically, and evicted least-recently-used above a size limit. Entries in use are leased
//! and never evicted. Captured stdin and FIFO data lives in `streams/`.

use crate::error::CoreError;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default size limit (spec §4.2: 20 GB).
pub const DEFAULT_LIMIT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
/// Captured stdin and FIFO data, one `stream-<pid>-<n>.log` per stream.
pub const STREAMS_DIR: &str = "streams";

/// FNV-1a over `parts`, with a separator after each part so `["ab", "c"]` and `["a", "bc"]` differ.
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for &b in *part {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The key of a cache entry derived from the file at `path`: its canonical path, the member chain inside it (empty for
/// the file itself), its size and modification time, and `purpose` (what the entry holds). A changed origin never
/// reuses a stale entry, and different products of one origin never collide.
pub fn origin_key(path: &Path, members: &[&str], purpose: &str) -> std::io::Result<u64> {
    let meta = std::fs::metadata(path)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let mut parts: Vec<&[u8]> = vec![canonical.as_os_str().as_encoded_bytes()];
    parts.extend(members.iter().map(|m| m.as_bytes()));
    let (len, mtime) = (meta.len().to_le_bytes(), mtime.to_le_bytes());
    parts.extend([&len[..], &mtime[..], purpose.as_bytes()]);
    Ok(fnv1a(&parts))
}

/// A writer-unique temporary name next to `dst`: `<dst>.<pid>.<n>.partial`. Concurrent writers of one entry never
/// share or delete each other's output, and a sweep can tell whether the writer is still running.
pub fn partial_path(dst: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut name = dst.as_os_str().to_owned();
    name.push(format!(".{}.{}.partial", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    PathBuf::from(name)
}

/// The writer of a `<name>.<pid>.<n>.partial` file.
fn partial_owner(name: &str) -> Option<i32> {
    let mut parts = name.strip_suffix(".partial")?.rsplitn(3, '.');
    parts.next()?.parse::<u64>().ok()?;
    let pid = parts.next()?.parse().ok()?;
    parts.next().map(|_| pid)
}

/// The writer of a `stream-<pid>-<n>.log` capture.
fn stream_owner(name: &str) -> Option<i32> {
    name.strip_prefix("stream-")?.split('-').next()?.parse().ok()
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 delivers nothing; it only checks that the process exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    alive || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Marks a file as recently used (the LRU order is the modification time).
fn touch(path: &Path) {
    if let Ok(file) = File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Evicted {
    pub removed: usize,
    pub freed: u64,
    /// Bytes left in the cache directory (leased entries and partial files included).
    pub remaining: u64,
}

pub struct Cache {
    dir: PathBuf,
    limit: u64,
    leases: Mutex<HashMap<PathBuf, usize>>,
}

/// Keeps a cache entry from being evicted while a document reads it.
pub struct CacheLease {
    cache: Arc<Cache>,
    path: PathBuf,
}

impl CacheLease {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CacheLease {
    fn drop(&mut self) {
        let mut leases = self.cache.leases.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = leases.get_mut(&self.path) {
            *n -= 1;
            if *n == 0 {
                leases.remove(&self.path);
            }
        }
    }
}

impl Cache {
    pub fn new(dir: impl Into<PathBuf>, limit: u64) -> Arc<Cache> {
        Arc::new(Cache { dir: dir.into(), limit, leases: Mutex::new(HashMap::new()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// `<dir>/<key as 16 hex digits>.<ext>`.
    pub fn entry(&self, key: u64, ext: &str) -> PathBuf {
        self.dir.join(format!("{key:016x}.{ext}"))
    }

    pub fn streams_dir(&self) -> PathBuf {
        self.dir.join(STREAMS_DIR)
    }

    /// Marks `path` in use until the lease drops (eviction skips it) and refreshes its LRU time. An entry may be
    /// leased before it is written, so eviction cannot remove it between its creation and its first use.
    pub fn lease(self: &Arc<Self>, path: &Path) -> CacheLease {
        touch(path);
        *self.leases.lock().unwrap_or_else(|e| e.into_inner()).entry(path.to_path_buf()).or_insert(0) += 1;
        CacheLease { cache: self.clone(), path: path.to_path_buf() }
    }

    pub fn is_leased(&self, path: &Path) -> bool {
        self.leases.lock().unwrap_or_else(|e| e.into_inner()).contains_key(path)
    }

    /// Writes an entry atomically: `write` fills a writer-unique partial file, which is renamed over `dst` on success
    /// and removed on error or cancellation, so `dst` is either complete or absent.
    pub fn write_entry(&self, dst: &Path, write: impl FnOnce(&mut File) -> Result<(), CoreError>) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.dir).map_err(CoreError::io(&self.dir))?;
        let partial = partial_path(dst);
        let result = File::create(&partial).map_err(CoreError::io(&partial)).and_then(|mut file| {
            write(&mut file)?;
            file.flush().map_err(CoreError::io(&partial))
        });
        match result {
            Ok(()) => std::fs::rename(&partial, dst).map_err(CoreError::io(dst)),
            Err(e) => {
                let _ = std::fs::remove_file(&partial);
                Err(e)
            }
        }
    }

    /// Removes partial files whose writer is no longer running (a crash or kill mid-write). Run at startup.
    pub fn sweep_partials(&self) -> usize {
        sweep(&self.dir, partial_owner)
    }

    /// Removes stdin / FIFO captures whose process is no longer running (stream tabs are never restored).
    pub fn sweep_streams(&self) -> usize {
        sweep(&self.streams_dir(), stream_owner)
    }

    /// Deletes least-recently-used entries until the directory holds at most the limit. Leased entries, partial files
    /// and the streams folder are never removed.
    pub fn evict(&self) -> Evicted {
        self.evict_to(self.limit)
    }

    pub fn evict_to(&self, limit: u64) -> Evicted {
        let Ok(read) = std::fs::read_dir(&self.dir) else { return Evicted::default() };
        let mut candidates: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let mut out = Evicted::default();
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            out.remaining += meta.len();
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".partial") || self.is_leased(&path) {
                continue;
            }
            candidates.push((meta.modified().unwrap_or(UNIX_EPOCH), meta.len(), path));
        }
        candidates.sort();
        for (_, len, path) in candidates {
            if out.remaining <= limit {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                out.removed += 1;
                out.freed += len;
                out.remaining -= len;
            }
        }
        out
    }
}

fn sweep(dir: &Path, owner: fn(&str) -> Option<i32>) -> usize {
    let Ok(read) = std::fs::read_dir(dir) else { return 0 };
    read.flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            owner(&name).is_some_and(|pid| !process_alive(pid)) && std::fs::remove_file(e.path()).is_ok()
        })
        .count()
}
```

- [ ] **Step 4: Route UTF-16 conversions and partial names through the cache**

In `crates/rogg-core/src/encoding.rs`, delete `unique_partial_path` (and its now-unused `AtomicU64`/`Ordering` imports) and use the cache's helper: in `transcode_to_utf8` replace `let partial = unique_partial_path(dst);` with `let partial = crate::cache::partial_path(dst);`. Update the doc comment of `transcode_to_utf8` to say "a writer-unique `<dst>.<pid>.<n>.partial` (`cache::partial_path`)".

In `crates/rogg-core/src/text.rs`, delete `fnv1a` and replace `transcoded_path` with:

```rust
/// Where the UTF-8 conversion of the UTF-16 file at `path` lives in the cache (spec §4.2 key).
fn transcoded_path(cache_dir: &Path, path: &Path) -> Result<PathBuf, CoreError> {
    let key = crate::cache::origin_key(path, &[], "utf8").map_err(CoreError::io(path))?;
    Ok(cache_dir.join(format!("{key:016x}.utf8")))
}
```

and in `LogText::open` replace `let dst = transcoded_path(cache_dir, path, &meta);` with `let dst = transcoded_path(cache_dir, path)?;` (remove the then-unused `meta` binding and its `metadata()` call).

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core`
Expected: PASS — the six cache tests, the encoding tests (partials still unique and cleaned up) and `tests/text.rs` (a second open of a UTF-16 file still reuses its conversion).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): cache with origin keys, atomic entries, leases, sweeps and LRU eviction

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: genlog bundles and archive fixtures

**Files:**
- Modify: `crates/rogg-genlog/Cargo.toml`, `crates/rogg-genlog/src/lib.rs`, `crates/rogg-genlog/src/main.rs`, `justfile`
- Create: `crates/rogg-genlog/src/bundle.rs`
- Test: `crates/rogg-genlog/tests/bundle.rs`

**Interfaces:**
- Consumes: `rogg_genlog::{GenOptions, GenStats, generate_file}` (M1).
- Produces (`rogg_genlog::bundle`, used as test fixtures by `rogg-core` and `rogg-app`):
  - `enum BundleKind { Zip, Tar, TarGz }` with `parse(&str) -> Option<BundleKind>` (`zip`, `tar`, `tgz`, `tar.gz`)
  - `write_archive(path: &Path, kind: BundleKind, members: &[(&str, &[u8])]) -> io::Result<()>` (zip members deflated)
  - `write_stored_zip(path: &Path, members: &[(&str, &[u8])]) -> io::Result<()>` (no compression, for damage tests)
  - `damage_zip_member(path: &Path, index: usize) -> io::Result<()>` (flips a byte inside member `index`'s data)
  - `write_gz(path: &Path, bytes: &[u8]) -> io::Result<()>`; `gz_bytes(bytes: &[u8]) -> Vec<u8>`
  - `generate_bundle(out: &Path, kind: BundleKind, files: usize, opts: &GenOptions) -> io::Result<GenStats>` (logs `service-00.log`…, seeds `opts.seed + i`, streamed through temp files)
  - CLI: `genlog --bundle zip|tar|tgz [--files N] --size S --out FILE`; `just bundle`

- [ ] **Step 1: Add the dependencies**

In `crates/rogg-genlog/Cargo.toml`, add:

```toml
[dependencies]
zip.workspace = true
tar.workspace = true
flate2.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

In `crates/rogg-genlog/src/lib.rs`, change the module doc's last sentence "No third-party dependencies." to "Archives are written with `zip`, `tar` and `flate2`." and add `pub mod bundle;` after the doc.

- [ ] **Step 2: Write the failing tests**

Create `crates/rogg-genlog/tests/bundle.rs`:

```rust
use flate2::read::MultiGzDecoder;
use rogg_genlog::GenOptions;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, generate_bundle, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::io::Read;

const MEMBERS: [(&str, &[u8]); 2] = [("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")];

fn tar_members(reader: impl Read) -> Vec<(String, Vec<u8>)> {
    let mut archive = tar::Archive::new(reader);
    archive
        .entries()
        .unwrap()
        .map(|e| {
            let mut e = e.unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.path().unwrap().to_string_lossy().into_owned(), bytes)
        })
        .collect()
}

fn zip_members(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut e = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.name().to_string(), bytes)
        })
        .collect()
}

fn expected() -> Vec<(String, Vec<u8>)> {
    MEMBERS.iter().map(|(n, b)| (n.to_string(), b.to_vec())).collect()
}

#[test]
fn archives_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &MEMBERS).unwrap();
    assert_eq!(zip_members(&zip), expected());
    let tar = dir.path().join("b.tar");
    write_archive(&tar, BundleKind::Tar, &MEMBERS).unwrap();
    assert_eq!(tar_members(std::fs::File::open(&tar).unwrap()), expected());
    let tgz = dir.path().join("b.tgz");
    write_archive(&tgz, BundleKind::TarGz, &MEMBERS).unwrap();
    assert_eq!(tar_members(MultiGzDecoder::new(std::fs::File::open(&tgz).unwrap())), expected());
    let gz = dir.path().join("a.log.gz");
    write_gz(&gz, b"x\ny\n").unwrap();
    let mut out = String::new();
    MultiGzDecoder::new(std::fs::File::open(&gz).unwrap()).read_to_string(&mut out).unwrap();
    assert_eq!(out, "x\ny\n");
    assert_eq!(std::fs::read(&gz).unwrap(), gz_bytes(b"x\ny\n"));
    assert_eq!(BundleKind::parse("tar.gz"), Some(BundleKind::TarGz));
    assert_eq!(BundleKind::parse("rar"), None);
}

#[test]
fn a_damaged_stored_member_fails_its_checksum_only() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &MEMBERS).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
    let mut sink = Vec::new();
    assert!(archive.by_index(0).unwrap().read_to_end(&mut sink).is_err(), "checksum mismatch");
    sink.clear();
    archive.by_index(1).unwrap().read_to_end(&mut sink).unwrap();
    assert_eq!(sink, b"read me\n");
}

#[test]
fn bundles_hold_one_log_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bundle.zip");
    let opts = GenOptions { size_bytes: 20_000, ..Default::default() };
    let stats = generate_bundle(&out, BundleKind::Zip, 3, &opts).unwrap();
    let members = zip_members(&out);
    assert_eq!(members.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["service-00.log", "service-01.log", "service-02.log"]);
    assert_eq!(members.iter().map(|(_, b)| b.len() as u64).sum::<u64>(), stats.bytes);
    assert_ne!(members[0].1, members[1].1, "each file has its own seed");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).filter(|n| n != "bundle.zip").collect();
    assert!(leftovers.is_empty(), "temp files removed: {leftovers:?}");
}
```

(Integration tests can use the crate's own `[dependencies]`, so `zip`, `tar` and `flate2` need no extra dev-dependencies.)

- [ ] **Step 3: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-genlog --test bundle`
Expected: FAIL to compile — `rogg_genlog::bundle` not found.

- [ ] **Step 4: Implement `bundle.rs`**

Create `crates/rogg-genlog/src/bundle.rs`:

```rust
//! Archives of synthetic logs (spec §3.1 bundle generator) and the archive fixtures of the rogg tests (spec §8: zip,
//! tar, tar.gz, tgz, gz, nested, corrupt).

use crate::{GenOptions, GenStats, generate_file};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleKind {
    Zip,
    Tar,
    TarGz,
}

impl BundleKind {
    pub fn parse(s: &str) -> Option<BundleKind> {
        match s {
            "zip" => Some(BundleKind::Zip),
            "tar" => Some(BundleKind::Tar),
            "tgz" | "tar.gz" => Some(BundleKind::TarGz),
            _ => None,
        }
    }
}

fn zip_options(method: zip::CompressionMethod) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default().compression_method(method).large_file(true)
}

fn tar_append<W: Write>(builder: &mut tar::Builder<W>, name: &str, len: u64, data: impl Read) -> io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(len);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, name, data)
}

/// Writes `members` (path, bytes) into a zip (deflated), a tar or a tar.gz at `path`.
pub fn write_archive(path: &Path, kind: BundleKind, members: &[(&str, &[u8])]) -> io::Result<()> {
    let file = File::create(path)?;
    match kind {
        BundleKind::Zip => write_zip(file, members, zip::CompressionMethod::Deflated),
        BundleKind::Tar => {
            let mut builder = tar::Builder::new(file);
            for (name, bytes) in members {
                tar_append(&mut builder, name, bytes.len() as u64, *bytes)?;
            }
            builder.into_inner()?.flush()
        }
        BundleKind::TarGz => {
            let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::fast()));
            for (name, bytes) in members {
                tar_append(&mut builder, name, bytes.len() as u64, *bytes)?;
            }
            builder.into_inner()?.finish()?.flush()
        }
    }
}

fn write_zip(file: File, members: &[(&str, &[u8])], method: zip::CompressionMethod) -> io::Result<()> {
    let mut zip = zip::ZipWriter::new(file);
    for (name, bytes) in members {
        zip.start_file(*name, zip_options(method)).map_err(io::Error::other)?;
        zip.write_all(bytes)?;
    }
    zip.finish().map_err(io::Error::other)?.flush()
}

/// A zip whose members are stored uncompressed, so `damage_zip_member` can corrupt their data in place.
pub fn write_stored_zip(path: &Path, members: &[(&str, &[u8])]) -> io::Result<()> {
    write_zip(File::create(path)?, members, zip::CompressionMethod::Stored)
}

/// Flips one byte in the middle of member `index`'s data (its CRC no longer matches; the directory stays intact).
pub fn damage_zip_member(path: &Path, index: usize) -> io::Result<()> {
    let (header, size) = {
        let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
        let entry = zip.by_index_raw(index).map_err(io::Error::other)?;
        (entry.header_start(), entry.compressed_size())
    };
    let mut file = File::options().read(true).write(true).open(path)?;
    // Local file header: 30 fixed bytes, then the name and the extra field, then the member's data.
    let mut fixed = [0u8; 30];
    file.seek(SeekFrom::Start(header))?;
    file.read_exact(&mut fixed)?;
    let name_len = u16::from_le_bytes([fixed[26], fixed[27]]) as u64;
    let extra_len = u16::from_le_bytes([fixed[28], fixed[29]]) as u64;
    let at = header + 30 + name_len + extra_len + size / 2;
    let mut byte = [0u8];
    file.seek(SeekFrom::Start(at))?;
    file.read_exact(&mut byte)?;
    file.seek(SeekFrom::Start(at))?;
    file.write_all(&[byte[0] ^ 0xff])
}

pub fn gz_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}

pub fn write_gz(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, gz_bytes(bytes))
}

/// `files` synthetic logs `service-NN.log` of `opts.size_bytes` each (seeds `opts.seed + i`) archived into `out`.
/// Each log is generated into a temporary file next to `out` and streamed into the archive, so bundles of 1 GB logs
/// never sit in memory.
pub fn generate_bundle(out: &Path, kind: BundleKind, files: usize, opts: &GenOptions) -> io::Result<GenStats> {
    let dir = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let names: Vec<String> = (0..files).map(|i| format!("service-{i:02}.log")).collect();
    let temps: Vec<PathBuf> = names.iter().map(|n| dir.join(format!(".genlog-{}-{n}", std::process::id()))).collect();
    let mut stats = GenStats::default();
    let result = (|| -> io::Result<()> {
        for (i, temp) in temps.iter().enumerate() {
            let s = generate_file(temp, &GenOptions { seed: opts.seed + i as u64, ..opts.clone() })?;
            stats.lines += s.lines;
            stats.bytes += s.bytes;
        }
        let file = File::create(out)?;
        match kind {
            BundleKind::Zip => {
                let mut zip = zip::ZipWriter::new(file);
                for (name, temp) in names.iter().zip(&temps) {
                    zip.start_file(name.as_str(), zip_options(zip::CompressionMethod::Deflated)).map_err(io::Error::other)?;
                    io::copy(&mut File::open(temp)?, &mut zip)?;
                }
                zip.finish().map_err(io::Error::other)?.flush()
            }
            BundleKind::Tar => tar_files(file, &names, &temps)?.flush(),
            BundleKind::TarGz => tar_files(GzEncoder::new(file, Compression::fast()), &names, &temps)?.finish()?.flush(),
        }
    })();
    for temp in &temps {
        let _ = std::fs::remove_file(temp);
    }
    result.map(|()| stats)
}

/// Streams each file of `temps` into a tar under the matching name; returns the finished tar's sink.
fn tar_files<W: Write>(sink: W, names: &[String], temps: &[PathBuf]) -> io::Result<W> {
    let mut builder = tar::Builder::new(sink);
    for (name, temp) in names.iter().zip(temps) {
        let source = File::open(temp)?;
        let len = source.metadata()?.len();
        tar_append(&mut builder, name, len, source)?;
    }
    builder.into_inner()
}
```

- [ ] **Step 5: Add the CLI flags and the just recipe**

In `crates/rogg-genlog/src/main.rs`:
- add `bundle: Option<BundleKind>` and `files: usize` to `Args`;
- usage becomes `"usage: genlog --size 1G --out FILE [--seed N] [--crlf] [--utf16le] [--long-every N] [--bundle zip|tar|tgz [--files N]]"`;
- add `"--bundle"` and `"--files"` to `known_flags` and to the flags that require a value;
- parse them:

```rust
    let bundle = match value("--bundle") {
        Some(kind) => Some(BundleKind::parse(&kind).ok_or_else(|| format!("bad --bundle: {kind}\n{usage}"))?),
        None => None,
    };
    let files = match value("--files") {
        Some(n) => n.parse::<usize>().map_err(|_| format!("bad --files: unparseable\n{usage}"))?,
        None => 10,
    };
```

- in `main`, generate a bundle when asked:

```rust
    let result = match parsed.bundle {
        Some(kind) => rogg_genlog::bundle::generate_bundle(&parsed.out, kind, parsed.files, &opts),
        None => generate_file(&parsed.out, &opts),
    };
    match result {
        Ok(stats) => println!("wrote {} lines, {} bytes to {}", stats.lines, stats.bytes, parsed.out.display()),
        Err(e) => {
            eprintln!("genlog: {e}");
            std::process::exit(1);
        }
    }
```

(import `rogg_genlog::bundle::BundleKind`). In `justfile`, after the `genlog` recipe, add:

```make
bundle kind="zip" files="10" size="64M" out="target/genlog/bundle.zip":
    mkdir -p target/genlog
    cargo run --release -p rogg-genlog -- --bundle {{kind}} --files {{files}} --size {{size}} --out {{out}}
```

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-genlog && cargo run -q -p rogg-genlog -- --bundle tgz --files 2 --size 10K --out /tmp/rogg-bundle-check.tgz`
Expected: tests PASS; the CLI prints `wrote … to /tmp/rogg-bundle-check.tgz`.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-genlog justfile Cargo.lock
git commit -m "feat(genlog): bundle generator and archive fixtures

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Archives and `.gz` — sniffing, listing, extracting, gunzip

**Files:**
- Create: `crates/rogg-core/src/archive.rs`
- Modify: `crates/rogg-core/src/error.rs` (`CoreError::Archive`), `crates/rogg-core/src/lib.rs`
- Test: `crates/rogg-core/tests/archive.rs`

**Interfaces:**
- Consumes: `CancelToken`, `CoreError`, `Encoding::detect` (M1); `rogg_genlog::bundle` fixtures (Task 3, dev-dependency).
- Produces (`rogg_core::archive`):
  - `SNIFF_BYTES: usize = 8192`
  - `enum ArchiveKind { Zip, Tar, TarGz }` (serde); `enum Content { Archive(ArchiveKind), Gzip, Text, Binary }`
  - `sniff(&Path) -> io::Result<Content>`; `sniff_head(&[u8]) -> Content` (no gzip look-ahead); `kind_from_name(&str) -> Option<ArchiveKind>`
  - `ArchiveEntry { path: String, size: u64 }`
  - `list(&Path, ArchiveKind, &CancelToken, progress: impl FnMut(u64, u64)) -> Result<Vec<ArchiveEntry>, CoreError>` (regular files only, archive order)
  - `extract(&Path, ArchiveKind, member: &str, out: &mut impl Write, &CancelToken, progress: impl FnMut(u64, u64)) -> Result<u64, CoreError>`
  - `gunzip(src: &Path, out: &mut impl Write, &CancelToken, progress: impl FnMut(u64, u64)) -> Result<u64, CoreError>` (progress = compressed bytes read / compressed size)
  - `CoreError::Archive { path: PathBuf, message: String }` (Display: `<path>: <message>`)

- [ ] **Step 1: Add the error variant**

In `crates/rogg-core/src/error.rs`, add to `CoreError` after `Io`:

```rust
    /// A damaged, truncated or unsupported archive or compressed file (spec §4.13: it fails only its own node).
    #[error("{}: {message}", path.display())]
    Archive { path: PathBuf, message: String },
```

- [ ] **Step 2: Write the failing tests**

Create `crates/rogg-core/tests/archive.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::archive::{ArchiveKind, Content, extract, gunzip, kind_from_name, list, sniff};
use rogg_core::error::CoreError;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::path::{Path, PathBuf};

const MEMBERS: [(&str, &[u8]); 3] = [("logs/api.log", b"api one\napi two\n"), ("logs/worker.log", b"worker\n"), ("README", b"read me\n")];

fn bundle(dir: &Path, name: &str, kind: BundleKind) -> PathBuf {
    let path = dir.join(name);
    write_archive(&path, kind, &MEMBERS).unwrap();
    path
}

const KINDS: [(BundleKind, ArchiveKind, &str); 3] =
    [(BundleKind::Zip, ArchiveKind::Zip, "b.zip"), (BundleKind::Tar, ArchiveKind::Tar, "b.tar"), (BundleKind::TarGz, ArchiveKind::TarGz, "b.tgz")];

#[test]
fn sniffs_archives_compressed_text_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        assert_eq!(sniff(&bundle(dir.path(), name, bundle_kind)).unwrap(), Content::Archive(kind), "{name}");
    }
    let gz = dir.path().join("a.log.gz");
    write_gz(&gz, b"plain text\n").unwrap();
    assert_eq!(sniff(&gz).unwrap(), Content::Gzip);
    let cases: [(&str, &[u8], Content); 4] = [
        ("t.log", b"hello\n", Content::Text),
        ("u16.log", &[0xFF, 0xFE, b'h', 0, b'i', 0], Content::Text),
        ("bin.dat", &[0x7F, b'E', b'L', b'F', 0, 1], Content::Binary),
        ("empty.log", b"", Content::Text),
    ];
    for (name, bytes, expected) in cases {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        assert_eq!(sniff(&p).unwrap(), expected, "{name}");
    }
    assert!(sniff(&dir.path().join("missing")).is_err());
}

#[test]
fn lists_regular_files_of_every_kind() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        let path = bundle(dir.path(), name, bundle_kind);
        let mut last = (0, 0);
        let entries = list(&path, kind, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
        let got: Vec<(&str, u64)> = entries.iter().map(|e| (e.path.as_str(), e.size)).collect();
        assert_eq!(got, [("logs/api.log", 16), ("logs/worker.log", 7), ("README", 8)], "{name}");
        assert!(last.0 > 0 && last.0 == last.1, "{name}: progress reaches its total ({last:?})");
    }
}

#[test]
fn extracts_members_and_reports_missing_ones() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        let path = bundle(dir.path(), name, bundle_kind);
        let mut out = Vec::new();
        let n = extract(&path, kind, "logs/worker.log", &mut out, &CancelToken::new(), |_, _| {}).unwrap();
        assert_eq!((n, out.as_slice()), (7, &b"worker\n"[..]), "{name}");
        let err = extract(&path, kind, "nope.log", &mut Vec::new(), &CancelToken::new(), |_, _| {}).unwrap_err();
        assert!(matches!(&err, CoreError::Archive { message, .. } if message.contains("nope.log")), "{name}: {err}");
    }
}

#[test]
fn a_damaged_member_fails_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.zip");
    write_stored_zip(&path, &MEMBERS).unwrap();
    damage_zip_member(&path, 0).unwrap();
    assert_eq!(list(&path, ArchiveKind::Zip, &CancelToken::new(), |_, _| {}).unwrap().len(), 3, "the directory is intact");
    let err = extract(&path, ArchiveKind::Zip, "logs/api.log", &mut Vec::new(), &CancelToken::new(), |_, _| {}).unwrap_err();
    assert!(matches!(&err, CoreError::Archive { message, .. } if message.contains("logs/api.log")), "{err}");
    let mut out = Vec::new();
    extract(&path, ArchiveKind::Zip, "README", &mut out, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(out, b"read me\n");
}

#[test]
fn truncated_and_garbage_archives_are_errors_not_panics() {
    let dir = tempfile::tempdir().unwrap();
    let tgz = bundle(dir.path(), "t.tgz", BundleKind::TarGz);
    let bytes = std::fs::read(&tgz).unwrap();
    std::fs::write(&tgz, &bytes[..bytes.len() / 2]).unwrap();
    assert!(matches!(list(&tgz, ArchiveKind::TarGz, &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })));
    let junk = dir.path().join("junk.zip");
    std::fs::write(&junk, b"PK\x03\x04 not really a zip").unwrap();
    assert!(matches!(list(&junk, ArchiveKind::Zip, &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })));
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let zip = bundle(dir.path(), "c.zip", BundleKind::Zip);
    assert!(matches!(list(&zip, ArchiveKind::Zip, &cancelled, |_, _| {}), Err(CoreError::Cancelled)));
}

#[test]
fn gunzips_multi_member_files_with_progress_and_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("multi.log.gz");
    let mut bytes = gz_bytes(b"first\n");
    bytes.extend(gz_bytes(b"second\n"));
    std::fs::write(&gz, &bytes).unwrap();
    let mut out = Vec::new();
    let mut last = (0, 0);
    let n = gunzip(&gz, &mut out, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
    assert_eq!((n, out.as_slice()), (13, &b"first\nsecond\n"[..]));
    assert_eq!(last, (bytes.len() as u64, bytes.len() as u64));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(gunzip(&gz, &mut Vec::new(), &cancel, |_, _| {}), Err(CoreError::Cancelled)));
    std::fs::write(&gz, &bytes[..bytes.len() - 5]).unwrap();
    assert!(matches!(gunzip(&gz, &mut Vec::new(), &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })), "a truncated stream");
}

#[test]
fn archive_kinds_from_member_names() {
    assert_eq!(kind_from_name("inner.ZIP"), Some(ArchiveKind::Zip));
    assert_eq!(kind_from_name("x.tar"), Some(ArchiveKind::Tar));
    assert_eq!(kind_from_name("x.tar.gz"), Some(ArchiveKind::TarGz));
    assert_eq!(kind_from_name("x.tgz"), Some(ArchiveKind::TarGz));
    assert_eq!(kind_from_name("app.log.1.gz"), None, "a compressed log is not an archive");
    assert_eq!(kind_from_name("app.log"), None);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test archive`
Expected: FAIL to compile — `rogg_core::archive` not found.

- [ ] **Step 4: Implement `archive.rs`**

Create `crates/rogg-core/src/archive.rs` and add `pub mod archive;` to `lib.rs`:

```rust
//! Archives and compressed files (spec §4.1): what a file holds, cheap listing of zip and tar archives, extracting
//! one member, and streaming `.gz` decompression. Damage is an `Archive` error for the archive or member it concerns,
//! never a panic (spec §4.13).

use crate::cancel::CancelToken;
use crate::encoding::Encoding;
use crate::error::CoreError;
use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::rc::Rc;

/// Bytes sniffed to tell text from binary (spec §4.1: the first 8 KB).
pub const SNIFF_BYTES: usize = 8192;
const COPY_CHUNK: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
}

/// What a file holds, judged from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    Archive(ArchiveKind),
    /// A single gzip-compressed file (`app.log.1.gz`).
    Gzip,
    Text,
    Binary,
}

/// A regular file inside an archive: its `/`-separated path and uncompressed size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

fn is_tar_block(block: &[u8]) -> bool {
    block.len() >= 512 && &block[257..262] == b"ustar"
}

/// Reads until `buf` is full or the input ends.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// Classifies a file: zip, tar, gzip (a tar.gz when the decompressed start is a tar header), text (no NUL in the
/// first 8 KB, or a UTF-16 BOM) or binary.
pub fn sniff(path: &Path) -> io::Result<Content> {
    let mut head = vec![0u8; SNIFF_BYTES];
    let n = read_full(&mut File::open(path)?, &mut head)?;
    head.truncate(n);
    if head.starts_with(&[0x1f, 0x8b]) {
        let mut inner = [0u8; 512];
        let got = read_full(&mut MultiGzDecoder::new(File::open(path)?), &mut inner).unwrap_or(0);
        return Ok(if is_tar_block(&inner[..got]) { Content::Archive(ArchiveKind::TarGz) } else { Content::Gzip });
    }
    Ok(sniff_head(&head))
}

/// `sniff` for bytes already read; a gzip start is reported as `Gzip` (no look inside).
pub fn sniff_head(head: &[u8]) -> Content {
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Content::Archive(ArchiveKind::Zip)
    } else if head.starts_with(&[0x1f, 0x8b]) {
        Content::Gzip
    } else if is_tar_block(head) {
        Content::Archive(ArchiveKind::Tar)
    } else if Encoding::detect(head).is_utf16() || !head[..head.len().min(SNIFF_BYTES)].contains(&0) {
        Content::Text
    } else {
        Content::Binary
    }
}

/// The archive kind a member's name suggests (members are only sniffed once extracted).
pub fn kind_from_name(name: &str) -> Option<ArchiveKind> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if lower.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else {
        None
    }
}

fn archive_error(path: &Path, message: impl std::fmt::Display) -> CoreError {
    CoreError::Archive { path: path.to_path_buf(), message: message.to_string() }
}

/// A member path as listed: without a leading `./` or `/`.
fn clean(name: &str) -> &str {
    name.trim_start_matches("./").trim_start_matches('/')
}

/// Counts the bytes read through it (tar progress is measured on the compressed file).
struct Counting<R> {
    inner: R,
    read: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.set(self.read.get() + n as u64);
        Ok(n)
    }
}

fn tar_reader(path: &Path, kind: ArchiveKind, read: Rc<Cell<u64>>) -> Result<(Box<dyn Read>, u64), CoreError> {
    let file = File::open(path).map_err(CoreError::io(path))?;
    let total = file.metadata().map_err(CoreError::io(path))?.len();
    let counted = Counting { inner: file, read };
    let reader: Box<dyn Read> = if kind == ArchiveKind::TarGz { Box::new(MultiGzDecoder::new(counted)) } else { Box::new(counted) };
    Ok((reader, total))
}

/// Lists the regular files of an archive in archive order. `progress(done, total)` counts entries (zip) or archive
/// bytes read (tar).
pub fn list(path: &Path, kind: ArchiveKind, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<Vec<ArchiveEntry>, CoreError> {
    let mut out = Vec::new();
    match kind {
        ArchiveKind::Zip => {
            let file = File::open(path).map_err(CoreError::io(path))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| archive_error(path, e))?;
            let total = zip.len() as u64;
            for i in 0..zip.len() {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let entry = zip.by_index_raw(i).map_err(|e| archive_error(path, e))?;
                if !entry.is_dir() {
                    out.push(ArchiveEntry { path: clean(entry.name()).to_string(), size: entry.size() });
                }
                progress(i as u64 + 1, total);
            }
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            let read = Rc::new(Cell::new(0));
            let (reader, total) = tar_reader(path, kind, read.clone())?;
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries().map_err(|e| archive_error(path, e))? {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let entry = entry.map_err(|e| archive_error(path, e))?;
                if entry.header().entry_type().is_file() {
                    let name = entry.path().map_err(|e| archive_error(path, e))?.to_string_lossy().into_owned();
                    out.push(ArchiveEntry { path: clean(&name).to_string(), size: entry.size() });
                }
            }
            progress(total, total);
        }
    }
    Ok(out)
}

enum CopyError {
    Read(io::Error),
    Write(io::Error),
    Cancelled,
}

/// Copies `from` to `out` in 1 MiB chunks, checking `cancel` between chunks.
fn copy(from: &mut impl Read, out: &mut impl Write, total: u64, cancel: &CancelToken, progress: &mut impl FnMut(u64, u64)) -> Result<u64, CopyError> {
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut written = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(CopyError::Cancelled);
        }
        let n = match from.read(&mut buf) {
            Ok(0) => return Ok(written),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(CopyError::Read(e)),
        };
        out.write_all(&buf[..n]).map_err(CopyError::Write)?;
        written += n as u64;
        progress(written, total.max(written));
    }
}

/// Copies archive member `member` to `out`; `progress(written, member size)`. A missing member, an unsupported
/// compression method or a damaged member is an `Archive` error naming the member.
pub fn extract(
    path: &Path,
    kind: ArchiveKind,
    member: &str,
    out: &mut impl Write,
    cancel: &CancelToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, CoreError> {
    let failed = |e: CopyError| match e {
        CopyError::Read(e) => archive_error(path, format!("{member}: {e}")),
        CopyError::Write(e) => CoreError::Io { path: path.to_path_buf(), source: e },
        CopyError::Cancelled => CoreError::Cancelled,
    };
    match kind {
        ArchiveKind::Zip => {
            let file = File::open(path).map_err(CoreError::io(path))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| archive_error(path, e))?;
            let index = (0..zip.len())
                .find(|&i| zip.name_for_index(i).is_some_and(|n| clean(n) == member))
                .ok_or_else(|| archive_error(path, format!("no member `{member}`")))?;
            let mut entry = zip.by_index(index).map_err(|e| archive_error(path, format!("{member}: {e}")))?;
            let size = entry.size();
            copy(&mut entry, out, size, cancel, &mut progress).map_err(failed)
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            let (reader, _) = tar_reader(path, kind, Rc::new(Cell::new(0)))?;
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries().map_err(|e| archive_error(path, e))? {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let mut entry = entry.map_err(|e| archive_error(path, e))?;
                let name = entry.path().map_err(|e| archive_error(path, e))?.to_string_lossy().into_owned();
                if entry.header().entry_type().is_file() && clean(&name) == member {
                    let size = entry.size();
                    return copy(&mut entry, out, size, cancel, &mut progress).map_err(failed);
                }
            }
            Err(archive_error(path, format!("no member `{member}`")))
        }
    }
}

/// Decompresses the gzip file `src` (every member of a multi-member file) into `out`, writing as it goes so a reader
/// of `out` sees the data at once; `progress(compressed bytes read, compressed size)`.
pub fn gunzip(src: &Path, out: &mut impl Write, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<u64, CoreError> {
    let file = File::open(src).map_err(CoreError::io(src))?;
    let total = file.metadata().map_err(CoreError::io(src))?.len();
    let read = Rc::new(Cell::new(0));
    let mut decoder = MultiGzDecoder::new(Counting { inner: file, read: read.clone() });
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut written = 0u64;
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
    progress(total, total);
    Ok(written)
}
```

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test archive && cargo test -p rogg-core`
Expected: PASS (7 archive tests; nothing else changes).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): sniff, list and extract zip/tar/tar.gz archives and gunzip .gz files

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Source references, the sidebar source tree and the folder scan

**Files:**
- Create: `crates/rogg-core/src/source.rs` (the `SourceRef` part; Task 6 adds opening), `crates/rogg-core/src/tree.rs`, `crates/rogg-core/src/folder.rs`
- Modify: `crates/rogg-core/src/lib.rs`
- Test: `crates/rogg-core/tests/sources.rs`

**Interfaces:**
- Consumes: `archive::{ArchiveKind, ArchiveEntry, Content, sniff, kind_from_name}` (Task 4).
- Produces:
  - `rogg_core::source::SourceRef` (serde, `Hash`): `File(PathBuf)` | `Member { archive: Box<SourceRef>, path: String }`; `SourceRef::member(archive, path)`, `root_path() -> &Path`, `member_chain() -> Vec<&str>`, `name() -> String`, `crumbs() -> Vec<String>`, `tooltip() -> String`
  - `rogg_core::tree`: `NodeId = usize`; `NodeKind { Folder, Archive(ArchiveKind), Member, File }`; `Listing { Pending, Listing, Listed, Failed(String) }`; `Node { name, kind, size: Option<u64>, source: Option<SourceRef>, parent: Option<NodeId>, children: Vec<NodeId>, listing }`; `TreeSummary { files: u64, bytes: u64 }`; `SourceTree::{new, roots, node, len, add_root(path, kind, size) -> NodeId, root_for(path) -> Option<NodeId>, set_listing(id, Listing), insert_folder(root, &FolderScan), insert_listing(archive, &[ArchiveEntry]), visible(expanded: impl Fn(NodeId) -> bool) -> Vec<(NodeId, usize)>, find(&SourceRef) -> Option<NodeId>, first_openable(under) -> Option<NodeId>, root_of(id) -> NodeId, summary(id) -> TreeSummary, is_openable(id) -> bool, is_container(id) -> bool}`
  - `rogg_core::folder`: `DEFAULT_SCAN_DEPTH = 4`, `MAX_SCAN_FILES = 10_000`, `ScannedFile { rel: PathBuf, size: u64, content: Content }`, `FolderScan { files: Vec<ScannedFile>, truncated: bool }`, `scan_folder(dir, depth, &CancelToken) -> Result<FolderScan, CoreError>`, `scan_folder_with(dir, depth, max_files, &CancelToken)`

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/tests/sources.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::archive::{ArchiveEntry, ArchiveKind, Content};
use rogg_core::folder::{DEFAULT_SCAN_DEPTH, scan_folder, scan_folder_with};
use rogg_core::source::SourceRef;
use rogg_core::tree::{Listing, NodeKind, SourceTree, TreeSummary};
use rogg_genlog::bundle::{BundleKind, write_archive, write_gz};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn names(tree: &SourceTree, rows: &[(usize, usize)]) -> Vec<String> {
    rows.iter().map(|&(id, depth)| format!("{}{}", "  ".repeat(depth), tree.node(id).name)).collect()
}

fn entry(path: &str, size: u64) -> ArchiveEntry {
    ArchiveEntry { path: path.into(), size }
}

#[test]
fn source_refs_name_their_chain() {
    let zip = SourceRef::File(PathBuf::from("/b/incident.zip"));
    let inner = SourceRef::member(zip.clone(), "nested/inner.tgz");
    let log = SourceRef::member(inner.clone(), "app/x.log");
    assert_eq!(log.root_path(), Path::new("/b/incident.zip"));
    assert_eq!(log.member_chain(), ["nested/inner.tgz", "app/x.log"]);
    assert_eq!(log.name(), "x.log");
    assert_eq!(log.crumbs(), ["incident.zip", "nested", "inner.tgz", "app", "x.log"]);
    assert_eq!(log.tooltip(), "x.log · incident.zip");
    let file = SourceRef::File(PathBuf::from("/var/log/api.log"));
    assert_eq!(file.crumbs(), ["log", "api.log"]);
    assert_eq!(file.tooltip(), "/var/log/api.log");
    assert!(file.member_chain().is_empty());
    let json = serde_json::to_string(&log).unwrap();
    assert_eq!(serde_json::from_str::<SourceRef>(&json).unwrap(), log);
}

#[test]
fn listings_become_folders_members_and_nested_archives() {
    let mut tree = SourceTree::new();
    let zip = tree.add_root(Path::new("/b/incident.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(900));
    assert_eq!(tree.node(zip).listing, Listing::Pending);
    tree.insert_listing(zip, &[entry("logs/worker.log", 20), entry("logs/api.log", 30), entry("inner.tgz", 50), entry("README", 5)]);
    assert_eq!(tree.node(zip).listing, Listing::Listed);
    let expanded: HashSet<usize> = tree.visible(|_| true).into_iter().map(|(id, _)| id).collect();
    let all = tree.visible(|id| expanded.contains(&id));
    assert_eq!(names(&tree, &all), ["incident.zip", "  logs", "    api.log", "    worker.log", "  inner.tgz", "  README"]);
    let collapsed = tree.visible(|id| id == zip);
    assert_eq!(names(&tree, &collapsed), ["incident.zip", "  logs", "  inner.tgz", "  README"], "only expanded containers open");
    let inner = all[4].0;
    assert_eq!(tree.node(inner).kind, NodeKind::Archive(ArchiveKind::TarGz));
    assert_eq!(tree.node(inner).listing, Listing::Pending);
    assert!(tree.is_container(inner) && !tree.is_openable(inner));
    let api = SourceRef::member(SourceRef::File(PathBuf::from("/b/incident.zip")), "logs/api.log");
    assert_eq!(tree.find(&api), Some(all[2].0));
    assert_eq!(tree.first_openable(zip), Some(all[5].0), "a file at the top comes before files in folders");
    assert_eq!(tree.first_openable(all[1].0), Some(all[2].0), "inside a folder: its first file");
    assert_eq!(tree.root_of(all[2].0), zip);
    assert_eq!(tree.summary(zip), TreeSummary { files: 3, bytes: 55 }, "nested archives are not files until listed");
    tree.insert_listing(inner, &[entry("x.log", 7)]);
    assert_eq!(tree.summary(zip), TreeSummary { files: 4, bytes: 62 });
    let x = tree.first_openable(inner).unwrap();
    assert_eq!(tree.node(x).source, Some(SourceRef::member(SourceRef::member(SourceRef::File(PathBuf::from("/b/incident.zip")), "inner.tgz"), "x.log")));
    tree.set_listing(inner, Listing::Failed("damaged".into()));
    assert_eq!(tree.node(inner).listing, Listing::Failed("damaged".into()));
    assert_eq!(tree.root_for(Path::new("/b/incident.zip")), Some(zip));
    assert_eq!(tree.root_for(Path::new("/b/other.zip")), None);
}

#[test]
fn folder_scans_list_text_gz_and_archives_but_not_binaries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("top.log"), "t\n").unwrap();
    std::fs::write(root.join("a/mid.log"), "m\n").unwrap();
    std::fs::write(root.join("a/b/deep.log"), "d\n").unwrap();
    std::fs::write(root.join(".hidden.log"), "h\n").unwrap();
    std::fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
    write_gz(&root.join("old.log.1.gz"), b"o\n").unwrap();
    write_archive(&root.join("a/more.zip"), BundleKind::Zip, &[("x.log", b"x\n")]).unwrap();
    let scan = scan_folder(&root, DEFAULT_SCAN_DEPTH, &CancelToken::new()).unwrap();
    let got: Vec<(String, Content)> = scan.files.iter().map(|f| (f.rel.display().to_string(), f.content)).collect();
    assert_eq!(
        got,
        [
            ("a/b/deep.log".to_string(), Content::Text),
            ("a/mid.log".to_string(), Content::Text),
            ("a/more.zip".to_string(), Content::Archive(ArchiveKind::Zip)),
            ("old.log.1.gz".to_string(), Content::Gzip),
            ("top.log".to_string(), Content::Text),
        ]
    );
    assert!(!scan.truncated);

    let mut tree = SourceTree::new();
    let id = tree.add_root(&root, NodeKind::Folder, None);
    tree.insert_folder(id, &scan);
    let rows = tree.visible(|_| true);
    assert_eq!(names(&tree, &rows), ["bundle", "  a", "    b", "      deep.log", "    mid.log", "    more.zip", "  old.log.1.gz", "  top.log"]);
    let deep = rows[3].0;
    assert_eq!(tree.node(deep).source, Some(SourceRef::File(root.join("a/b/deep.log"))));
    assert_eq!(tree.node(rows[5].0).kind, NodeKind::Archive(ArchiveKind::Zip));
    assert_eq!(tree.node(rows[6].0).kind, NodeKind::File, "a .gz log opens like a file");
    assert_eq!(tree.summary(id).files, 4);
}

#[test]
fn scan_is_bounded_and_does_not_follow_symlinked_folders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("r");
    let mut deep = root.clone();
    for level in 1..=6 {
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join(format!("level{level}.log")), "x\n").unwrap();
        deep = deep.join("sub");
    }
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    std::os::unix::fs::symlink(root.join("level1.log"), root.join("alias.log")).unwrap();
    let scan = scan_folder(&root, DEFAULT_SCAN_DEPTH, &CancelToken::new()).unwrap();
    let got: Vec<String> = scan.files.iter().map(|f| f.rel.display().to_string()).collect();
    assert_eq!(got, ["alias.log", "level1.log", "sub/level2.log", "sub/sub/level3.log", "sub/sub/sub/level4.log"], "depth 4, no loop");
    let few = scan_folder_with(&root, DEFAULT_SCAN_DEPTH, 2, &CancelToken::new()).unwrap();
    assert_eq!(few.files.len(), 2);
    assert!(few.truncated);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(scan_folder(&root, DEFAULT_SCAN_DEPTH, &cancel).is_err());
    assert!(scan_folder(&dir.path().join("missing"), DEFAULT_SCAN_DEPTH, &CancelToken::new()).is_err());
}
```

(`serde_json` is a `rogg-core` dependency, so the integration test can use it.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test sources`
Expected: FAIL to compile — `rogg_core::source`, `tree`, `folder` not found.

- [ ] **Step 3: Implement `SourceRef`**

Create `crates/rogg-core/src/source.rs` (Task 6 adds opening below it) and add `pub mod source;` to `lib.rs`:

```rust
//! What a tab shows and where it comes from (spec §4.1): a file, or a member of an archive (possibly nested).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceRef {
    File(PathBuf),
    /// `path` (`/`-separated) inside the archive `archive`, which is itself a file or a member.
    Member { archive: Box<SourceRef>, path: String },
}

impl SourceRef {
    pub fn member(archive: SourceRef, path: impl Into<String>) -> SourceRef {
        SourceRef::Member { archive: Box::new(archive), path: path.into() }
    }

    /// The file on disk everything comes from (the outermost archive of a member).
    pub fn root_path(&self) -> &Path {
        match self {
            SourceRef::File(path) => path,
            SourceRef::Member { archive, .. } => archive.root_path(),
        }
    }

    /// Member paths from the outermost archive inwards; empty for a file.
    pub fn member_chain(&self) -> Vec<&str> {
        match self {
            SourceRef::File(_) => Vec::new(),
            SourceRef::Member { archive, path } => {
                let mut chain = archive.member_chain();
                chain.push(path.as_str());
                chain
            }
        }
    }

    /// The file's or member's own name.
    pub fn name(&self) -> String {
        match self {
            SourceRef::File(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string()),
            SourceRef::Member { path, .. } => path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path.as_str()).to_string(),
        }
    }

    /// The info-bar breadcrumb (spec §5.3): a file's folder and name; for a member, the outer archive's name, each
    /// nested archive and folder, and the member's name (`incident.zip › logs › api.log`).
    pub fn crumbs(&self) -> Vec<String> {
        match self {
            SourceRef::File(path) => {
                let mut crumbs: Vec<String> = path.parent().and_then(Path::file_name).map(|d| d.to_string_lossy().into_owned()).into_iter().collect();
                crumbs.push(self.name());
                crumbs
            }
            SourceRef::Member { archive, path } => {
                let mut crumbs = match archive.as_ref() {
                    SourceRef::File(_) => vec![archive.name()],
                    member => member.crumbs(),
                };
                crumbs.extend(path.split('/').filter(|s| !s.is_empty()).map(str::to_string));
                crumbs
            }
        }
    }

    /// Tab tooltip: a file's full path; a member as `api.log · incident.zip` (the reference's tab title).
    pub fn tooltip(&self) -> String {
        match self {
            SourceRef::File(path) => path.display().to_string(),
            SourceRef::Member { .. } => {
                let root = self.root_path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                format!("{} · {root}", self.name())
            }
        }
    }
}
```

- [ ] **Step 4: Implement the folder scan**

Create `crates/rogg-core/src/folder.rs` and add `pub mod folder;` to `lib.rs`:

```rust
//! Opening a folder (spec §4.1): a recursive scan, up to a depth, of the regular files that are text, gzip or known
//! archives. Binaries (a NUL in the first 8 KB) and hidden entries are skipped, symlinked folders are not followed
//! (no cycles), and the scan stops after `MAX_SCAN_FILES` files.

use crate::archive::{Content, sniff};
use crate::cancel::CancelToken;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

/// Spec §4.1 default (settings arrive in M4): files directly in the folder are depth 1.
pub const DEFAULT_SCAN_DEPTH: usize = 4;
pub const MAX_SCAN_FILES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
    /// Path relative to the scanned folder.
    pub rel: PathBuf,
    pub size: u64,
    /// `Text`, `Gzip` or `Archive(..)` — binaries are never listed.
    pub content: Content,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderScan {
    /// Sorted by path within each folder (files and folders interleaved by name).
    pub files: Vec<ScannedFile>,
    /// The scan stopped at the file limit.
    pub truncated: bool,
}

pub fn scan_folder(dir: &Path, depth: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    scan_folder_with(dir, depth, MAX_SCAN_FILES, cancel)
}

/// `scan_folder` with an explicit file limit (tests).
pub fn scan_folder_with(dir: &Path, depth: usize, max_files: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    // The folder itself must be readable; unreadable subfolders are skipped.
    std::fs::read_dir(dir).map_err(CoreError::io(dir))?;
    let mut scan = FolderScan::default();
    walk(dir, Path::new(""), 1, depth.max(1), max_files, cancel, &mut scan)?;
    Ok(scan)
}

fn walk(dir: &Path, rel: &Path, depth: usize, max_depth: usize, max_files: usize, cancel: &CancelToken, scan: &mut FolderScan) -> Result<(), CoreError> {
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    let Ok(read) = std::fs::read_dir(dir) else { return Ok(()) };
    let mut entries: Vec<std::fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if scan.truncated {
            return Ok(());
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            if depth < max_depth {
                walk(&path, &rel.join(&name), depth + 1, max_depth, max_files, cancel, scan)?;
            }
            continue;
        }
        // Regular files and symlinks to regular files; a symlink to a folder is not followed.
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(content) = sniff(&path) else { continue };
        if content == Content::Binary {
            continue;
        }
        if scan.files.len() >= max_files {
            scan.truncated = true;
            return Ok(());
        }
        scan.files.push(ScannedFile { rel: rel.join(&name), size: meta.len(), content });
    }
    Ok(())
}
```

- [ ] **Step 5: Implement the source tree**

Create `crates/rogg-core/src/tree.rs` and add `pub mod tree;` to `lib.rs`:

```rust
//! The sidebar's tree of opened folders and archives (spec §4.1 `SourceTree`): folder, archive, member and file
//! nodes with sizes. Archives are listed lazily (on expand); nodes are never removed, so ids stay valid.

use crate::archive::{ArchiveEntry, ArchiveKind, Content, kind_from_name};
use crate::folder::FolderScan;
use crate::source::SourceRef;
use std::collections::HashMap;
use std::path::Path;

pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Archive(ArchiveKind),
    /// A file inside an archive.
    Member,
    /// A file on disk (plain or `.gz`).
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
    /// Not listed yet (archives until first expanded, a folder until its scan finishes).
    Pending,
    Listing,
    Listed,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
    /// Size on disk (files, archives) or uncompressed size (members); None for folders.
    pub size: Option<u64>,
    /// What opening (files, members) or listing (archives) this node uses; roots also carry their path. None for
    /// folders inside a root.
    pub source: Option<SourceRef>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub listing: Listing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeSummary {
    /// Openable files and members.
    pub files: u64,
    /// Their sizes (uncompressed for members).
    pub bytes: u64,
}

#[derive(Default)]
pub struct SourceTree {
    nodes: Vec<Node>,
    roots: Vec<NodeId>,
    by_source: HashMap<SourceRef, NodeId>,
}

impl SourceTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn is_container(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].kind, NodeKind::Folder | NodeKind::Archive(_))
    }

    pub fn is_openable(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].kind, NodeKind::Member | NodeKind::File)
    }

    fn push(&mut self, node: Node) -> NodeId {
        let id = self.nodes.len();
        if let Some(parent) = node.parent {
            self.nodes[parent].children.push(id);
        }
        if let Some(source) = &node.source {
            self.by_source.entry(source.clone()).or_insert(id);
        }
        self.nodes.push(node);
        id
    }

    /// Adds an opened folder or archive at the top level, `Pending` until it is scanned or listed.
    pub fn add_root(&mut self, path: &Path, kind: NodeKind, size: Option<u64>) -> NodeId {
        let source = SourceRef::File(path.to_path_buf());
        let id = self.push(Node { name: source.name(), kind, size, source: Some(source), parent: None, children: Vec::new(), listing: Listing::Pending });
        self.roots.push(id);
        id
    }

    /// The root opened from `path`, if any.
    pub fn root_for(&self, path: &Path) -> Option<NodeId> {
        self.roots.iter().copied().find(|&r| matches!(&self.nodes[r].source, Some(SourceRef::File(p)) if p == path))
    }

    pub fn set_listing(&mut self, id: NodeId, listing: Listing) {
        self.nodes[id].listing = listing;
    }

    /// The child folder `name` of `parent`, created when missing.
    fn folder_child(&mut self, parent: NodeId, name: &str) -> NodeId {
        let existing = self.nodes[parent].children.iter().copied().find(|&c| self.nodes[c].kind == NodeKind::Folder && self.nodes[c].name == name);
        existing.unwrap_or_else(|| {
            self.push(Node { name: name.to_string(), kind: NodeKind::Folder, size: None, source: None, parent: Some(parent), children: Vec::new(), listing: Listing::Listed })
        })
    }

    /// Sorts `id`'s subtree: folders first, then by name (case-insensitive).
    fn sort(&mut self, id: NodeId) {
        let mut children = std::mem::take(&mut self.nodes[id].children);
        children.sort_by_key(|&c| (self.nodes[c].kind != NodeKind::Folder, self.nodes[c].name.to_lowercase()));
        for &c in &children {
            self.sort(c);
        }
        self.nodes[id].children = children;
    }

    /// Fills a folder root from its scan: subfolders, files (plain and `.gz`) and archives (`Pending`).
    pub fn insert_folder(&mut self, root: NodeId, scan: &FolderScan) {
        let Some(SourceRef::File(base)) = self.nodes[root].source.clone() else { return };
        for file in &scan.files {
            let parts: Vec<String> = file.rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            let Some((leaf, dirs)) = parts.split_last() else { continue };
            let mut parent = root;
            for dir in dirs {
                parent = self.folder_child(parent, dir);
            }
            let (kind, listing) = match file.content {
                Content::Archive(kind) => (NodeKind::Archive(kind), Listing::Pending),
                _ => (NodeKind::File, Listing::Listed),
            };
            let source = SourceRef::File(base.join(&file.rel));
            self.push(Node { name: leaf.clone(), kind, size: Some(file.size), source: Some(source), parent: Some(parent), children: Vec::new(), listing });
        }
        self.sort(root);
        self.nodes[root].listing = Listing::Listed;
    }

    /// Fills an archive from its listing: folders from the member paths, members, and nested archives (`Pending`).
    pub fn insert_listing(&mut self, archive: NodeId, entries: &[ArchiveEntry]) {
        let Some(archive_source) = self.nodes[archive].source.clone() else { return };
        for entry in entries {
            let parts: Vec<&str> = entry.path.split('/').filter(|p| !p.is_empty()).collect();
            let Some((leaf, dirs)) = parts.split_last() else { continue };
            let mut parent = archive;
            for dir in dirs {
                parent = self.folder_child(parent, dir);
            }
            let (kind, listing) = match kind_from_name(leaf) {
                Some(kind) => (NodeKind::Archive(kind), Listing::Pending),
                None => (NodeKind::Member, Listing::Listed),
            };
            let source = SourceRef::member(archive_source.clone(), entry.path.clone());
            self.push(Node { name: leaf.to_string(), kind, size: Some(entry.size), source: Some(source), parent: Some(parent), children: Vec::new(), listing });
        }
        self.sort(archive);
        self.nodes[archive].listing = Listing::Listed;
    }

    /// Rows in display order with their depth: every root, and the children of each container `expanded` accepts.
    pub fn visible(&self, expanded: impl Fn(NodeId) -> bool) -> Vec<(NodeId, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = self.roots.iter().rev().map(|&r| (r, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            out.push((id, depth));
            if self.is_container(id) && expanded(id) {
                stack.extend(self.nodes[id].children.iter().rev().map(|&c| (c, depth + 1)));
            }
        }
        out
    }

    /// The openable node showing `source`.
    pub fn find(&self, source: &SourceRef) -> Option<NodeId> {
        self.by_source.get(source).copied().filter(|&id| self.is_openable(id))
    }

    /// The file or member to open first under `id`: its first direct file, else the first one found in its folders and
    /// archives (in display order).
    pub fn first_openable(&self, id: NodeId) -> Option<NodeId> {
        let children = &self.nodes[id].children;
        children.iter().copied().find(|&c| self.is_openable(c)).or_else(|| children.iter().find_map(|&c| self.first_openable(c)))
    }

    pub fn root_of(&self, mut id: NodeId) -> NodeId {
        while let Some(parent) = self.nodes[id].parent {
            id = parent;
        }
        id
    }

    /// Openable files under `id` and their total size (spec §5.3 footer `10 members · 8.4 GB expanded`).
    pub fn summary(&self, id: NodeId) -> TreeSummary {
        self.nodes[id].children.iter().fold(TreeSummary::default(), |acc, &c| {
            let sub = if self.is_openable(c) { TreeSummary { files: 1, bytes: self.nodes[c].size.unwrap_or(0) } } else { self.summary(c) };
            TreeSummary { files: acc.files + sub.files, bytes: acc.bytes + sub.bytes }
        })
    }
}
```

Note the `first_openable` order: a direct file wins over files in folders (opening a bundle shows its top-level log, e.g. `README` in the test, rather than the first file of its first folder).

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test sources && cargo test -p rogg-core`
Expected: PASS (4 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): source references, sidebar source tree and bounded folder scan

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Opening any source — member extraction, progressive gunzip, UTF-16 and indexing

**Files:**
- Modify: `crates/rogg-core/src/source.rs` (opening), `crates/rogg-core/src/text.rs` (open through a file handle, backing path)
- Test: `crates/rogg-core/tests/open_source.rs`

**Interfaces:**
- Consumes: `Cache`, `CacheLease`, `origin_key`, `partial_path` (Task 2); `sniff`, `list`, `extract`, `gunzip` (Task 4); `SourceRef` (Task 5); `LogText::open` (M1).
- Produces:
  - `LogText::open_handle(file: File, path: &Path, cache_dir: &Path, &CancelToken, on_progress) -> Result<LogText, CoreError>`; `LogText::backing_path() -> Option<&Path>` (the mapped file: the source, or its UTF-8 conversion); `LogText::mapped_len() -> u64` (mapped bytes, BOM included). `LogText` keeps the mapped `File` open (`Option<Arc<File>>`) for Task 7's growth.
  - `rogg_core::source::{OpenStage { Extracting, Decompressing, Transcoding, Indexing }, OpenUpdate { stage, name: String, done: u64, total: u64 }, OpenedSource { text: LogText, leases: Vec<CacheLease>, inflating: Option<Inflating>, watch: Option<PathBuf> }, open_source(&SourceRef, &Arc<Cache>, &CancelToken, progress: impl FnMut(OpenUpdate)) -> Result<OpenedSource, CoreError>, list_archive(&SourceRef, &Arc<Cache>, &CancelToken) -> Result<Vec<ArchiveEntry>, CoreError>}`
  - `Inflating` (a `.gz` still decompressing into the file the text maps): `dst() -> &Path`, `partial() -> &Path`, `progress() -> (u64, u64)` (compressed bytes read, compressed size), `fraction() -> f32`, `cancel()`, `poll(&mut self) -> InflatePoll`; `InflatePoll { Running, Done(Result<(), CoreError>) }`. Dropping it cancels the decompression.
  - Cache entries: extracted members `<key>.member` (`origin_key(root, chain, "member")`), decompressed `.gz` `<key>.log` (`origin_key(root, chain, "gunzip")`).
  - `open_source` never receives a FIFO (the app routes FIFOs to stream capture, Task 15).

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/tests/open_source.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use rogg_core::encoding::Encoding;
use rogg_core::error::CoreError;
use rogg_core::source::{InflatePoll, OpenStage, OpenedSource, SourceRef, list_archive, open_source};
use rogg_core::text::LogText;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn lines(text: &LogText) -> Vec<String> {
    (0..text.line_count()).map(|i| text.line_text(i).into_owned()).collect()
}

fn cache(dir: &Path) -> Arc<Cache> {
    Cache::new(dir.join("cache"), DEFAULT_LIMIT_BYTES)
}

/// The complete text of an opened source: waits for a progressive `.gz` to finish, then opens its entry.
fn finish(opened: OpenedSource, cache: &Cache) -> LogText {
    let Some(mut inflating) = opened.inflating else { return opened.text };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match inflating.poll() {
            InflatePoll::Done(result) => {
                result.unwrap();
                return LogText::open(inflating.dst(), cache.dir(), &CancelToken::new(), |_| {}).unwrap();
            }
            InflatePoll::Running => {
                assert!(Instant::now() < deadline, "decompression did not finish");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[test]
fn plain_files_open_in_place_and_are_watched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let opened = open_source(&SourceRef::File(path.clone()), &cache(dir.path()), &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&opened.text), ["one", "two"]);
    assert_eq!(opened.watch, Some(path.clone()));
    assert_eq!(opened.text.backing_path(), Some(path.as_path()));
    assert!(opened.leases.is_empty() && opened.inflating.is_none());
}

#[test]
fn members_are_extracted_once_and_leased() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"api one\napi two\n")]).unwrap();
    let cache = cache(dir.path());
    let member = SourceRef::member(SourceRef::File(zip), "logs/api.log");
    let mut stages = Vec::new();
    let first = open_source(&member, &cache, &CancelToken::new(), |u| stages.push((u.stage, u.name))).unwrap();
    assert!(stages.contains(&(OpenStage::Extracting, "api.log".to_string())), "{stages:?}");
    assert_eq!(lines(&first.text), ["api one", "api two"]);
    assert_eq!(first.watch, None, "cache files are not watched");
    assert_eq!(first.leases.len(), 1);
    assert!(cache.is_leased(first.leases[0].path()));
    assert_eq!(cache.evict_to(0).removed, 0, "an open member is never evicted");
    let mut again = Vec::new();
    let second = open_source(&member, &cache, &CancelToken::new(), |u| again.push(u.stage)).unwrap();
    assert!(!again.contains(&OpenStage::Extracting), "the cached member is reused");
    drop((first, second));
    assert_eq!(cache.evict_to(0).removed, 1, "closed members are evictable");
}

#[test]
fn nested_archives_and_compressed_members_open() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("inner.tgz");
    write_archive(&inner, BundleKind::TarGz, &[("app/x.log", b"nested line\n")]).unwrap();
    let inner_bytes = std::fs::read(&inner).unwrap();
    let gz = gz_bytes(b"gz line\n");
    let zip = dir.path().join("outer.zip");
    write_archive(&zip, BundleKind::Zip, &[("inner.tgz", &inner_bytes), ("old.log.1.gz", &gz)]).unwrap();
    let cache = cache(dir.path());
    let outer = SourceRef::File(zip);
    let nested = SourceRef::member(SourceRef::member(outer.clone(), "inner.tgz"), "app/x.log");
    let opened = open_source(&nested, &cache, &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&opened.text), ["nested line"]);
    let compressed = open_source(&SourceRef::member(outer.clone(), "old.log.1.gz"), &cache, &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&finish(compressed, &cache)), ["gz line"]);
    let entries = list_archive(&SourceRef::member(outer, "inner.tgz"), &cache, &CancelToken::new()).unwrap();
    assert_eq!(entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["app/x.log"]);
}

#[test]
fn large_gz_files_are_viewable_while_they_decompress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.log.gz");
    let line = format!("{}\n", "x".repeat(99));
    write_gz(&path, line.repeat(1_000_000).as_bytes()).unwrap();
    let cache = cache(dir.path());
    let mut stages = Vec::new();
    let opened = open_source(&SourceRef::File(path.clone()), &cache, &CancelToken::new(), |u| stages.push(u.stage)).unwrap();
    assert!(stages.contains(&OpenStage::Decompressing));
    assert!(opened.inflating.is_some(), "100 MB take longer to decompress than the first bytes");
    let prefix = opened.text.line_count();
    assert!(prefix < 1_000_000, "the first text is a prefix ({prefix} lines)");
    let full = finish(opened, &cache);
    assert_eq!(full.line_count(), 1_000_000);
    let mut again = Vec::new();
    let reopened = open_source(&SourceRef::File(path), &cache, &CancelToken::new(), |u| again.push(u.stage)).unwrap();
    assert!(reopened.inflating.is_none() && !again.contains(&OpenStage::Decompressing), "the decompressed entry is reused");
    assert_eq!(reopened.text.line_count(), 1_000_000);
}

#[test]
fn utf16_gz_files_are_complete_before_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let mut utf16 = vec![0xFF, 0xFE];
    for u in "héllo\r\nwörld\n".encode_utf16() {
        utf16.extend(u.to_le_bytes());
    }
    let path = dir.path().join("w.log.gz");
    write_gz(&path, &utf16).unwrap();
    let cache = cache(dir.path());
    let opened = open_source(&SourceRef::File(path), &cache, &CancelToken::new(), |_| {}).unwrap();
    assert!(opened.inflating.is_none());
    assert_eq!(opened.text.encoding(), Encoding::Utf16Le);
    assert_eq!(lines(&opened.text), ["héllo", "wörld"]);
    assert_eq!(opened.leases.len(), 2, "the decompressed file and its UTF-8 conversion");
}

#[test]
fn errors_name_what_failed() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(dir.path());
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let damaged = open_source(&SourceRef::member(SourceRef::File(zip.clone()), "logs/api.log"), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&damaged, Err(CoreError::Archive { message, .. }) if message.contains("logs/api.log")));
    assert!(open_source(&SourceRef::member(SourceRef::File(zip.clone()), "README"), &cache, &CancelToken::new(), |_| {}).is_ok());
    let as_file = open_source(&SourceRef::File(zip.clone()), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&as_file, Err(CoreError::Archive { message, .. }) if message.contains("is an archive")));
    let plain = dir.path().join("p.log");
    std::fs::write(&plain, "x\n").unwrap();
    let not_archive = open_source(&SourceRef::member(SourceRef::File(plain), "x"), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&not_archive, Err(CoreError::Archive { message, .. }) if message.contains("is not an archive")));
    let cancel = CancelToken::new();
    cancel.cancel();
    let zip2 = dir.path().join("c.zip");
    write_archive(&zip2, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    assert!(matches!(open_source(&SourceRef::member(SourceRef::File(zip2), "a.log"), &cache, &cancel, |_| {}), Err(CoreError::Cancelled)));
    assert!(matches!(open_source(&SourceRef::File(dir.path().join("missing.log")), &cache, &CancelToken::new(), |_| {}), Err(CoreError::Io { .. })));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test open_source`
Expected: FAIL to compile — `open_source`, `OpenStage`, … not found.

- [ ] **Step 3: Let `LogText` open through a handle and remember its file**

In `crates/rogg-core/src/text.rs`:

1. Add `use std::sync::Arc;` and two fields to `LogText` (after `source_path`):

```rust
    /// The file whose bytes are mapped: the source itself, or its UTF-8 conversion in the cache.
    backing: Option<PathBuf>,
    /// The mapped file, kept open so growth can re-map it (spec §4.5); None for in-memory and converted texts.
    file: Option<Arc<File>>,
```

`from_bytes` sets both to `None`.

2. Replace `map_file` with a version that maps an open file:

```rust
fn map_file(file: &File, path: &Path) -> Result<Data, CoreError> {
    let len = file.metadata().map_err(CoreError::io(path))?.len();
    if len == 0 {
        return Ok(Data::Owned(Vec::new()));
    }
    SafeMap::map(file).map(Data::Mapped).map_err(CoreError::io(path))
}
```

3. Replace `LogText::open` with `open` + `open_handle`:

```rust
    /// Opens a plain file: detects the encoding, transcodes UTF-16 into `cache_dir`, memory-maps and indexes it.
    pub fn open(path: &Path, cache_dir: &Path, cancel: &CancelToken, on_progress: impl FnMut(OpenProgress)) -> Result<LogText, CoreError> {
        let file = File::open(path).map_err(CoreError::io(path))?;
        Self::open_handle(file, path, cache_dir, cancel, on_progress)
    }

    /// `open` for a file that is already open: a `.gz` being decompressed is read through a handle taken before its
    /// partial file is renamed. `path` names the file for errors, the cache key and `source_path`.
    pub fn open_handle(
        file: File,
        path: &Path,
        cache_dir: &Path,
        cancel: &CancelToken,
        mut on_progress: impl FnMut(OpenProgress),
    ) -> Result<LogText, CoreError> {
        let mut head = [0u8; 4];
        let mut head_len = 0;
        while head_len < head.len() {
            let n = std::os::unix::fs::FileExt::read_at(&file, &mut head[head_len..], head_len as u64).map_err(CoreError::io(path))?;
            if n == 0 {
                break;
            }
            head_len += n;
        }
        let encoding = Encoding::detect(&head[..head_len]);
        let (data, start, backing, handle) = if encoding.is_utf16() {
            let dst = transcoded_path(cache_dir, path)?;
            if !dst.exists() {
                std::fs::create_dir_all(cache_dir).map_err(CoreError::io(cache_dir))?;
                transcode_to_utf8(path, &dst, encoding, cancel, |done, total| on_progress(OpenProgress::Transcoding { done, total }))?;
            }
            let converted = File::open(&dst).map_err(CoreError::io(&dst))?;
            (map_file(&converted, &dst)?, 0, dst, None)
        } else {
            (map_file(&file, path)?, encoding.bom_len(), path.to_path_buf(), Some(Arc::new(file)))
        };
        let text = &data.bytes()[start.min(data.bytes().len())..];
        let total = text.len() as u64;
        let mut index = build_index(text, DEFAULT_CHUNK_BYTES, cancel, |_, p: IndexProgress| on_progress(OpenProgress::Indexing { done: p.bytes_done, total }))?;
        index.shrink_to_fit();
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        Ok(LogText { data, start, index, encoding, endings, source_path: Some(path.to_path_buf()), backing: Some(backing), file: handle })
    }
```

Remove the now-unused `std::io::Read` import if nothing else uses it.

4. Add accessors after `source_path`:

```rust
    /// The file whose bytes are mapped (the source, or its UTF-8 conversion in the cache).
    pub fn backing_path(&self) -> Option<&Path> {
        self.backing.as_deref()
    }

    /// Bytes mapped, a UTF-8 BOM included (compare with the backing file's size to see whether it grew).
    pub fn mapped_len(&self) -> u64 {
        self.data.bytes().len() as u64
    }
```

- [ ] **Step 4: Implement opening in `source.rs`**

Append to `crates/rogg-core/src/source.rs` (and extend its imports):

```rust
use crate::archive::{ArchiveEntry, Content, extract, gunzip, list, sniff};
use crate::cache::{Cache, CacheLease, origin_key, partial_path};
use crate::cancel::CancelToken;
use crate::encoding::Encoding;
use crate::error::CoreError;
use crate::text::{LogText, OpenProgress};
use std::fs::File;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenStage {
    Extracting,
    Decompressing,
    Transcoding,
    Indexing,
}

/// Progress of `open_source`: the stage, the file it concerns (Extracting / Decompressing) and bytes done of total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenUpdate {
    pub stage: OpenStage,
    pub name: String,
    pub done: u64,
    pub total: u64,
}

pub struct OpenedSource {
    pub text: LogText,
    /// Cache entries the text reads from; keep them while the text is in use (eviction skips them).
    pub leases: Vec<CacheLease>,
    /// A `.gz` still decompressing into the file the text maps: the text can grow as it proceeds (Task 7 `refresh`).
    pub inflating: Option<Inflating>,
    /// Plain files on disk: the path live follow watches (spec §4.5).
    pub watch: Option<PathBuf>,
}

fn update(stage: OpenStage, name: &str, done: u64, total: u64) -> OpenUpdate {
    OpenUpdate { stage, name: name.to_string(), done, total }
}

/// Opens `src` for viewing (spec §4.1): extracts archive members into the cache (outer archives first), decompresses
/// `.gz` — progressively: the returned text covers what is decompressed so far and `inflating` keeps going —, converts
/// UTF-16, and indexes. Opening an archive itself is an error (its members open from the sidebar).
pub fn open_source(src: &SourceRef, cache: &Arc<Cache>, cancel: &CancelToken, mut progress: impl FnMut(OpenUpdate)) -> Result<OpenedSource, CoreError> {
    let mut leases = Vec::new();
    let raw = raw_file(src, cache, cancel, &mut progress, &mut leases)?;
    let name = src.name();
    match sniff(&raw).map_err(CoreError::io(&raw))? {
        Content::Archive(_) => Err(CoreError::Archive { path: raw, message: format!("{name} is an archive; open its members from the sidebar") }),
        Content::Gzip => open_gzip(src, &raw, &name, cache, cancel, &mut progress, leases),
        Content::Text | Content::Binary => {
            let text = open_text(File::open(&raw).map_err(CoreError::io(&raw))?, &raw, cache, cancel, &mut progress, &mut leases)?;
            let watch = matches!(src, SourceRef::File(_)).then(|| raw.clone());
            Ok(OpenedSource { text, leases, inflating: None, watch })
        }
    }
}

/// Lists the archive `src`, extracting it into the cache first when it is itself a member (nested archives).
pub fn list_archive(src: &SourceRef, cache: &Arc<Cache>, cancel: &CancelToken) -> Result<Vec<ArchiveEntry>, CoreError> {
    let mut leases = Vec::new();
    let path = raw_file(src, cache, cancel, &mut |_: OpenUpdate| {}, &mut leases)?;
    match sniff(&path).map_err(CoreError::io(&path))? {
        Content::Archive(kind) => list(&path, kind, cancel, |_, _| {}),
        _ => Err(CoreError::Archive { path, message: format!("{} is not an archive", src.name()) }),
    }
}

/// The file holding `src`'s raw bytes: the file itself, or the member extracted into the cache.
fn raw_file(
    src: &SourceRef,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    leases: &mut Vec<CacheLease>,
) -> Result<PathBuf, CoreError> {
    let SourceRef::Member { archive, path } = src else { return Ok(src.root_path().to_path_buf()) };
    let container = raw_file(archive, cache, cancel, progress, leases)?;
    let kind = match sniff(&container).map_err(CoreError::io(&container))? {
        Content::Archive(kind) => kind,
        _ => return Err(CoreError::Archive { path: container, message: format!("{} is not an archive", archive.name()) }),
    };
    let root = src.root_path();
    let dst = cache.entry(origin_key(root, &src.member_chain(), "member").map_err(CoreError::io(root))?, "member");
    leases.push(cache.lease(&dst));
    if !dst.exists() {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let name = src.name();
        cache.write_entry(&dst, |file| {
            extract(&container, kind, path, file, cancel, |done, total| progress(update(OpenStage::Extracting, &name, done, total))).map(drop)
        })?;
    }
    Ok(dst)
}

/// Opens a text file, leasing its UTF-8 conversion when one was made in the cache.
fn open_text(
    file: File,
    path: &Path,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    leases: &mut Vec<CacheLease>,
) -> Result<LogText, CoreError> {
    let text = LogText::open_handle(file, path, cache.dir(), cancel, |p| {
        progress(match p {
            OpenProgress::Transcoding { done, total } => update(OpenStage::Transcoding, "", done, total),
            OpenProgress::Indexing { done, total } => update(OpenStage::Indexing, "", done, total),
        })
    })?;
    if let Some(backing) = text.backing_path()
        && backing != path
    {
        leases.push(cache.lease(backing));
    }
    Ok(text)
}

fn open_gzip(
    src: &SourceRef,
    raw: &Path,
    name: &str,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    mut leases: Vec<CacheLease>,
) -> Result<OpenedSource, CoreError> {
    let root = src.root_path();
    let dst = cache.entry(origin_key(root, &src.member_chain(), "gunzip").map_err(CoreError::io(root))?, "log");
    leases.push(cache.lease(&dst));
    if !dst.exists() {
        std::fs::create_dir_all(cache.dir()).map_err(CoreError::io(cache.dir()))?;
        let mut inflating = Inflating::start(raw.to_path_buf(), dst.clone())?;
        let mut report = |read: u64, total: u64| progress(update(OpenStage::Decompressing, name, read, total));
        // A UTF-16 file must be complete before it can be converted; UTF-8 is shown while it decompresses.
        let head = inflating.wait_for_head(4, cancel, &mut report)?;
        if Encoding::detect(&head).is_utf16() {
            inflating.wait_done(cancel, &mut report)?;
        } else if !inflating.finished {
            let reader = inflating.reader.take().expect("the reader is taken once");
            let text = open_text(reader, inflating.partial(), cache, cancel, progress, &mut leases)?;
            return Ok(OpenedSource { text, leases, inflating: Some(inflating), watch: None });
        }
    }
    let text = open_text(File::open(&dst).map_err(CoreError::io(&dst))?, &dst, cache, cancel, progress, &mut leases)?;
    Ok(OpenedSource { text, leases, inflating: None, watch: None })
}

enum InflateMsg {
    Progress(u64, u64),
    Done(Result<(), CoreError>),
}

pub enum InflatePoll {
    Running,
    /// The decompression ended; on success the complete file is at `Inflating::dst`.
    Done(Result<(), CoreError>),
}

/// A `.gz` decompressing into the cache on a background thread (spec §4.1: viewable while it proceeds). The output
/// goes to a partial file renamed to the entry when complete; the text reads it through a handle taken up front, so
/// the rename does not affect it. Dropping this cancels the decompression (its partial file is removed).
pub struct Inflating {
    cancel: CancelToken,
    rx: Receiver<InflateMsg>,
    reader: Option<File>,
    partial: PathBuf,
    dst: PathBuf,
    progress: (u64, u64),
    finished: bool,
}

impl Inflating {
    fn start(src: PathBuf, dst: PathBuf) -> Result<Inflating, CoreError> {
        let partial = partial_path(&dst);
        let mut writer = File::create(&partial).map_err(CoreError::io(&partial))?;
        let reader = File::open(&partial).map_err(CoreError::io(&partial))?;
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let (thread_partial, thread_dst) = (partial.clone(), dst.clone());
        std::thread::Builder::new()
            .name("rogg-gunzip".into())
            .spawn(move || {
                let sender = tx.clone();
                let result = gunzip(&src, &mut writer, &token, |read, total| {
                    let _ = sender.send(InflateMsg::Progress(read, total));
                })
                .and_then(|_| std::fs::rename(&thread_partial, &thread_dst).map_err(CoreError::io(&thread_dst)));
                if result.is_err() {
                    let _ = std::fs::remove_file(&thread_partial);
                }
                let _ = tx.send(InflateMsg::Done(result));
            })
            .map_err(|source| CoreError::Io { path: dst.clone(), source })?;
        Ok(Inflating { cancel, rx, reader: Some(reader), partial, dst, progress: (0, 0), finished: false })
    }

    pub fn dst(&self) -> &Path {
        &self.dst
    }

    pub fn partial(&self) -> &Path {
        &self.partial
    }

    /// Compressed bytes read and the compressed size, as last reported.
    pub fn progress(&self) -> (u64, u64) {
        self.progress
    }

    pub fn fraction(&self) -> f32 {
        let (read, total) = self.progress;
        if total == 0 { 0. } else { read as f32 / total as f32 }
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    fn died(&self) -> CoreError {
        CoreError::Archive { path: self.dst.clone(), message: "decompression stopped unexpectedly".into() }
    }

    /// Drains progress; reports `Done` once the decompression ends.
    pub fn poll(&mut self) -> InflatePoll {
        loop {
            match self.rx.try_recv() {
                Ok(InflateMsg::Progress(read, total)) => self.progress = (read, total),
                Ok(InflateMsg::Done(result)) => {
                    self.finished = true;
                    return InflatePoll::Done(result);
                }
                Err(TryRecvError::Empty) => return InflatePoll::Running,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    return InflatePoll::Done(Err(self.died()));
                }
            }
        }
    }

    /// Blocks until at least `n` bytes are decompressed (or the stream ended) and returns those first bytes.
    fn wait_for_head(&mut self, n: usize, cancel: &CancelToken, report: &mut impl FnMut(u64, u64)) -> Result<Vec<u8>, CoreError> {
        loop {
            if cancel.is_cancelled() {
                self.cancel();
                return Err(CoreError::Cancelled);
            }
            let reader = self.reader.as_ref().expect("the reader is held until the text opens");
            let len = reader.metadata().map_err(CoreError::io(&self.partial))?.len();
            if len >= n as u64 || self.finished {
                let mut head = vec![0u8; (len as usize).min(n)];
                std::os::unix::fs::FileExt::read_exact_at(reader, &mut head, 0).map_err(CoreError::io(&self.partial))?;
                return Ok(head);
            }
            self.wait_step(report)?;
        }
    }

    /// Blocks until the decompression ends.
    fn wait_done(&mut self, cancel: &CancelToken, report: &mut impl FnMut(u64, u64)) -> Result<(), CoreError> {
        while !self.finished {
            if cancel.is_cancelled() {
                self.cancel();
                return Err(CoreError::Cancelled);
            }
            self.wait_step(report)?;
        }
        Ok(())
    }

    fn wait_step(&mut self, report: &mut impl FnMut(u64, u64)) -> Result<(), CoreError> {
        match self.rx.recv_timeout(Duration::from_millis(10)) {
            Ok(InflateMsg::Progress(read, total)) => {
                self.progress = (read, total);
                report(read, total);
                Ok(())
            }
            Ok(InflateMsg::Done(Ok(()))) => {
                self.finished = true;
                Ok(())
            }
            Ok(InflateMsg::Done(Err(e))) => Err(e),
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Err(RecvTimeoutError::Disconnected) => Err(self.died()),
        }
    }
}

impl Drop for Inflating {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
```

Two details to keep: `wait_for_head` reads the partial file through the handle (`read_exact_at`), never by path, because the thread may already have renamed it; and when the decompression finished before the head check, `open_gzip` opens the complete entry instead of returning a finished `Inflating`.

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test open_source && cargo test -p rogg-core`
Expected: PASS (6 tests; `tests/text.rs`, `tests/mapping.rs` and the rest unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): open files, archive members and .gz sources through the cache

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Growth — `LogText::refresh`, tail scans and the follower

**Files:**
- Modify: `crates/rogg-core/src/text.rs`, `crates/rogg-core/src/index.rs`, `crates/rogg-core/src/engine.rs`, `crates/rogg-core/src/search.rs`, `crates/rogg-core/src/lib.rs`
- Create: `crates/rogg-core/src/follow.rs`
- Test: `crates/rogg-core/tests/growth.rs`, `crates/rogg-core/tests/follow.rs`, `crates/rogg-core/tests/filter_props.rs` (one new property)

**Interfaces:**
- Consumes: `LogText` with its file handle (Task 6), `SafeMap` (Task 1), `build_index_into` (M1), `CompiledFilter::scan` (M1), `notify` (Task 1 pin).
- Produces:
  - `LineIndex::ends_with_newline() -> bool`
  - `text::Refresh { Unchanged, Grew(LogText), Truncated }`; `LogText::refresh(&self, &CancelToken) -> Result<Refresh, CoreError>` (only appended bytes are indexed; a shorter file or a changed start is `Truncated`; texts without a file handle — in memory, UTF-16 conversions — are always `Unchanged`); `LogText::stable_lines() -> u64` (lines an append cannot change: all of them, or all but an unfinished last line); `LogText::file_identity() -> Option<(u64, u64)>` (device, inode)
  - `engine::run_filter_lines(text, filter, lines: Range<u64>, chunk_lines, cancel) -> Result<FilterResult, CoreError>`; `FilterJob::spawn_from(text, filter, from: u64, chunk_lines) -> FilterJob` (`spawn` = `spawn_from(.., 0, ..)`); `engine::TailBase { from: u64, matches: Vec<u64>, term_hits: Vec<u64> }` with `TailBase::new(prev: &LogText, filter: &CompiledFilter, matches: &[u64], term_hits: &[u64]) -> TailBase` and `merge(self, tail_matches: Vec<u64>, tail_hits: &[u64]) -> (Vec<u64>, Vec<u64>)`
  - `search::SearchScope::From(u64)` (lines from `n` to the end)
  - `rogg_core::follow::{POLL_FALLBACK = 500 ms, DEBOUNCE = 30 ms, WatchMode { Path(PathBuf), Poll }, FollowEvent { Grew(Arc<LogText>), Truncated, Replaced, Failed(String) }, Follower}`; `Follower::spawn(Arc<LogText>, WatchMode, poll: Duration) -> Follower`, `events() -> &Receiver<FollowEvent>`, `stop()`. A follower ends after `Truncated`, `Replaced` or `Failed`; dropping it stops its thread within one poll interval.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/tests/growth.rs`:

```rust
use proptest::prelude::*;
use rogg_core::CancelToken;
use rogg_core::text::{LogText, Refresh};
use std::io::Write;
use std::path::Path;

fn open(path: &Path) -> LogText {
    LogText::open(path, &path.with_extension("cache"), &CancelToken::new(), |_| {}).unwrap()
}

fn append(path: &Path, bytes: &[u8]) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(bytes).unwrap();
}

fn lines(t: &LogText) -> Vec<String> {
    (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()
}

fn grew(t: &LogText) -> LogText {
    match t.refresh(&CancelToken::new()).unwrap() {
        Refresh::Grew(next) => next,
        Refresh::Unchanged => panic!("expected growth, got Unchanged"),
        Refresh::Truncated => panic!("expected growth, got Truncated"),
    }
}

#[test]
fn refresh_reports_growth_truncation_and_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let text = open(&path);
    assert!(matches!(text.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    append(&path, b"three\n");
    let next = grew(&text);
    assert_eq!(lines(&next), ["one", "two", "three"]);
    assert_eq!(next.index(), open(&path).index(), "the extended index equals a fresh one");
    assert_eq!(lines(&text), ["one", "two"], "the old snapshot is unchanged");
    assert!(matches!(next.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(4).unwrap();
    assert!(matches!(next.refresh(&CancelToken::new()).unwrap(), Refresh::Truncated));
    // Emptied and written again to the same length (`copytruncate` + new lines): the start changed.
    let rewritten = dir.path().join("b.log");
    std::fs::write(&rewritten, "aaaa
").unwrap();
    let before = open(&rewritten);
    std::fs::write(&rewritten, "bbbb
cc
").unwrap();
    assert!(matches!(before.refresh(&CancelToken::new()).unwrap(), Refresh::Truncated), "a rewrite is not growth");
    assert_eq!(text.file_identity(), next.file_identity());
    assert!(text.file_identity().is_some());
}

#[test]
fn an_unfinished_last_line_is_completed_by_an_append() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\nerr").unwrap();
    let text = open(&path);
    assert_eq!((text.line_count(), text.stable_lines()), (2, 1));
    append(&path, b"or here\nnext");
    let next = grew(&text);
    assert_eq!(lines(&next), ["a", "error here", "next"]);
    assert_eq!(next.stable_lines(), 2);
    append(&path, b"\n");
    assert_eq!(grew(&next).stable_lines(), 3);
}

#[test]
fn empty_files_grow_and_fixed_texts_never_do() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.log");
    std::fs::write(&path, "").unwrap();
    let empty = open(&path);
    assert_eq!((empty.line_count(), empty.stable_lines()), (0, 0));
    append(&path, b"first\n");
    assert_eq!(lines(&grew(&empty)), ["first"]);
    let memory = LogText::from_bytes(b"a\n".to_vec());
    assert!(matches!(memory.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    assert!(memory.file_identity().is_none());
    let utf16 = dir.path().join("w.log");
    std::fs::write(&utf16, [0xFF, 0xFE, b'h', 0, b'\n', 0]).unwrap();
    let converted = open(&utf16);
    append(&utf16, &[b'i', 0, b'\n', 0]);
    assert!(matches!(converted.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged), "UTF-16 conversions do not follow growth");
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'x'), Just(b'\n'), Just(b'\r')], 0..300)
}

proptest! {
    /// Spec §8: incremental append == full reindex, through the file and the mapping.
    #[test]
    fn appends_equal_a_fresh_open(data in bytes(), mut cuts in proptest::collection::vec(0usize..300, 0..5)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("g.log");
        cuts.push(data.len());
        cuts.sort_unstable();
        let first = cuts[0].min(data.len());
        std::fs::write(&path, &data[..first]).unwrap();
        let mut text = open(&path);
        let mut at = first;
        for cut in cuts {
            let cut = cut.min(data.len());
            if cut > at {
                append(&path, &data[at..cut]);
                at = cut;
                text = match text.refresh(&CancelToken::new()).unwrap() {
                    Refresh::Grew(next) => next,
                    _ => panic!("an append must grow the text"),
                };
            }
        }
        let fresh = LogText::from_bytes(data.clone());
        prop_assert_eq!(lines(&text), lines(&fresh));
        prop_assert_eq!(text.index().longest_line(), fresh.index().longest_line());
    }
}
```

Create `crates/rogg-core/tests/follow.rs`:

```rust
use rogg_core::CancelToken;
use rogg_core::follow::{FollowEvent, Follower, POLL_FALLBACK, WatchMode};
use rogg_core::text::LogText;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn open(path: &Path) -> Arc<LogText> {
    Arc::new(LogText::open(path, &path.with_extension("cache"), &CancelToken::new(), |_| {}).unwrap())
}

/// Waits up to 10 s for an event that `want` accepts; earlier events are skipped.
fn wait(follower: &Follower, want: impl Fn(&FollowEvent) -> bool) -> FollowEvent {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(event) = follower.events().recv_timeout(Duration::from_millis(50))
            && want(&event)
        {
            return event;
        }
    }
    panic!("no matching follow event within 10 s");
}

#[test]
fn appends_arrive_as_growing_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live.log");
    std::fs::write(&path, "one\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Path(path.clone()), POLL_FALLBACK);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"two\nthree\n").unwrap();
    let FollowEvent::Grew(text) = wait(&follower, |e| matches!(e, FollowEvent::Grew(t) if t.line_count() == 3)) else { unreachable!() };
    assert_eq!(text.line_text(2), "three");
}

#[test]
fn truncation_and_replacement_end_the_follower() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Path(path.clone()), POLL_FALLBACK);
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(2).unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Truncated));

    let rotated = dir.path().join("r.log");
    std::fs::write(&rotated, "old\n").unwrap();
    let follower = Follower::spawn(open(&rotated), WatchMode::Path(rotated.clone()), POLL_FALLBACK);
    let fresh = dir.path().join("r.log.new");
    std::fs::write(&fresh, "new\n").unwrap();
    std::fs::rename(&fresh, &rotated).unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Replaced));
}

#[test]
fn poll_mode_sees_growth_without_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\n").unwrap();
    let follower = Follower::spawn(open(&path), WatchMode::Poll, Duration::from_millis(20));
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"b\n").unwrap();
    wait(&follower, |e| matches!(e, FollowEvent::Grew(t) if t.line_count() == 2));
    follower.stop();
}
```

Add to `crates/rogg-core/tests/filter_props.rs` (extend the imports with `rogg_core::engine::{TailBase, run_filter_lines}`):

```rust
proptest! {
    /// Spec §4.5 + Review Focus 3: a tail scan after an append, merged with the previous result, equals a full scan of
    /// the grown text — even when the cut fell inside a line that the append completes.
    #[test]
    fn tail_scan_merge_equals_a_full_scan(lines in lines(), spec in spec(), cut in any::<prop::sample::Index>(), chunk in 1u64..9) {
        let full = lines.join("\n").into_bytes();
        let cut = cut.index(full.len() + 1);
        let old = LogText::from_bytes(full[..cut].to_vec());
        let new = LogText::from_bytes(full.clone());
        let compiled = CompiledFilter::compile(&spec).unwrap();
        let prev = run_filter(&old, &compiled, chunk, &CancelToken::new()).unwrap();
        let base = TailBase::new(&old, &compiled, &prev.matches, &prev.term_hits);
        let tail = run_filter_lines(&new, &compiled, base.from..new.line_count(), chunk, &CancelToken::new()).unwrap();
        let (matches, hits) = base.merge(tail.matches, &tail.term_hits);
        let whole = run_filter(&new, &compiled, chunk, &CancelToken::new()).unwrap();
        prop_assert_eq!(matches, whole.matches);
        prop_assert_eq!(hits, whole.term_hits);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test growth --test follow --test filter_props`
Expected: FAIL to compile — `Refresh`, `rogg_core::follow`, `TailBase`, `run_filter_lines` not found.

- [ ] **Step 3: Implement growth in `text.rs` and `index.rs`**

In `crates/rogg-core/src/index.rs`, add to `impl LineIndex`:

```rust
    /// Whether the indexed bytes end with `\n` (otherwise the last line may still be extended by an append).
    pub fn ends_with_newline(&self) -> bool {
        self.ends_with_newline
    }
```

In `crates/rogg-core/src/text.rs` (import `crate::index::build_index_into`, `std::os::unix::fs::{FileExt, MetadataExt}`):

1. Add a field to `LogText`, set in `open_handle` to `data.bytes()[..data.bytes().len().min(HEAD_SAMPLE)].to_vec()` and in `from_bytes` to `Vec::new()`:

```rust
    /// A copy of the file's first bytes when it was mapped: a file rewritten in place (emptied and written again, as
    /// `logrotate copytruncate` does) is told apart from one that only grew, even when it is as long as before.
    head: Vec<u8>,
```

with, near the top of the file:

```rust
/// Bytes of a file's start remembered to detect in-place rewrites.
const HEAD_SAMPLE: usize = 64;
```

2. Add before `impl LogText`:

```rust
/// What `LogText::refresh` found.
pub enum Refresh {
    Unchanged,
    /// The file grew: a new snapshot whose index extends this one's (only the appended bytes were scanned).
    Grew(LogText),
    /// The file is shorter than what is mapped, or its start changed: it was truncated or rewritten (reopen it).
    Truncated,
}
```

3. And in `impl LogText`:

```rust
    /// Lines an append cannot change: every line, or all but the last when the text does not end with `\n`.
    pub fn stable_lines(&self) -> u64 {
        let n = self.line_count();
        if n == 0 || self.index.ends_with_newline() { n } else { n - 1 }
    }

    /// (device, inode) of the mapped file; live follow compares it with the path's to detect replacement.
    pub fn file_identity(&self) -> Option<(u64, u64)> {
        let meta = self.file.as_ref()?.metadata().ok()?;
        Some((meta.dev(), meta.ino()))
    }

    /// Checks the mapped file (spec §4.5). A longer file whose start is unchanged gives a new snapshot that maps the
    /// whole file and indexes only the appended bytes (an unfinished last line continues). A shorter file, or one whose
    /// first bytes changed, is `Truncated`. Texts without a file handle (in memory, UTF-16 conversions) are always
    /// `Unchanged`. A rewrite that keeps the first 64 bytes is indistinguishable from an append.
    pub fn refresh(&self, cancel: &CancelToken) -> Result<Refresh, CoreError> {
        let Some(file) = &self.file else { return Ok(Refresh::Unchanged) };
        let path = self.backing.clone().unwrap_or_default();
        let len = file.metadata().map_err(CoreError::io(&path))?.len();
        let have = self.mapped_len();
        if len < have {
            return Ok(Refresh::Truncated);
        }
        let mut head = vec![0u8; self.head.len()];
        if !head.is_empty() && (file.read_exact_at(&mut head, 0).is_err() || head != self.head) {
            return Ok(Refresh::Truncated);
        }
        if len == have {
            return Ok(Refresh::Unchanged);
        }
        let map = SafeMap::map(file).map_err(CoreError::io(&path))?;
        let start = self.start.min(map.bytes().len());
        let bytes = &map.bytes()[start..];
        let mut index = build_index_into(self.index.clone(), bytes, DEFAULT_CHUNK_BYTES, cancel, |_, _| {})?;
        index.shrink_to_fit();
        let endings = if self.endings == LineEndings::None { LineEndings::detect(&bytes[..bytes.len().min(ENDINGS_SAMPLE_BYTES)]) } else { self.endings };
        let head = map.bytes()[..map.bytes().len().min(HEAD_SAMPLE)].to_vec();
        Ok(Refresh::Grew(LogText {
            data: Data::Mapped(map),
            start: self.start,
            index,
            encoding: self.encoding,
            endings,
            source_path: self.source_path.clone(),
            backing: self.backing.clone(),
            file: self.file.clone(),
            head,
        }))
    }
```

- [ ] **Step 4: Tail scans in the engine and search**

In `crates/rogg-core/src/engine.rs`, replace `run_filter` and `FilterJob::spawn` and add `run_filter_lines`, `spawn_from` and `TailBase`:

```rust
/// Blocking scan of the whole text.
pub fn run_filter(text: &LogText, filter: &CompiledFilter, chunk_lines: u64, cancel: &CancelToken) -> Result<FilterResult, CoreError> {
    run_filter_lines(text, filter, 0..text.line_count(), chunk_lines, cancel)
}

/// Blocking scan of `lines` only (tail scans after growth).
pub fn run_filter_lines(text: &LogText, filter: &CompiledFilter, lines: Range<u64>, chunk_lines: u64, cancel: &CancelToken) -> Result<FilterResult, CoreError> {
    let mut events = Vec::new();
    scan_lines(text, filter, lines, chunk_lines, cancel, |e| events.push(e));
    collect(events)
}

/// What a tail scan adds to (spec §4.5: after an append only the new lines are scanned): the previous result's matches
/// before `from` and its term hits without the lines from `from` on. `from` is the previous text's `stable_lines()`,
/// so an unfinished last line is scanned again — an append may complete it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailBase {
    pub from: u64,
    pub matches: Vec<u64>,
    pub term_hits: Vec<u64>,
}

impl TailBase {
    pub fn new(prev: &LogText, filter: &CompiledFilter, matches: &[u64], term_hits: &[u64]) -> TailBase {
        let from = prev.stable_lines();
        let kept = matches.partition_point(|&m| m < from);
        let mut hits = term_hits.to_vec();
        hits.resize(filter.spec().terms.len(), 0);
        if from < prev.line_count() {
            let redo = filter.scan(prev, from..prev.line_count());
            for (h, r) in hits.iter_mut().zip(&redo.term_hits) {
                *h = h.saturating_sub(*r);
            }
        }
        TailBase { from, matches: matches[..kept].to_vec(), term_hits: hits }
    }

    /// The grown text's result: these matches followed by the tail scan's (which start at `from`), hits added up.
    pub fn merge(mut self, tail_matches: Vec<u64>, tail_hits: &[u64]) -> (Vec<u64>, Vec<u64>) {
        self.matches.extend(tail_matches);
        if self.term_hits.len() < tail_hits.len() {
            self.term_hits.resize(tail_hits.len(), 0);
        }
        for (h, t) in self.term_hits.iter_mut().zip(tail_hits) {
            *h += t;
        }
        (self.matches, self.term_hits)
    }
}
```

and in `impl FilterJob`:

```rust
    pub fn spawn(text: Arc<LogText>, filter: Arc<CompiledFilter>, chunk_lines: u64) -> Self {
        Self::spawn_from(text, filter, 0, chunk_lines)
    }

    /// A background scan of the lines from `from` to the end (a tail scan when `from > 0`).
    pub fn spawn_from(text: Arc<LogText>, filter: Arc<CompiledFilter>, from: u64, chunk_lines: u64) -> Self {
        let cancel = CancelToken::new();
        let (tx, events) = mpsc::channel();
        let token = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("rogg-filter".into())
            .spawn(move || {
                scan_lines(&text, &filter, from..text.line_count(), chunk_lines, &token, |e| {
                    let _ = tx.send(e);
                })
            })
            .expect("spawn filter thread");
        Self { cancel, events, thread: Some(thread) }
    }
```

In `crates/rogg-core/src/search.rs`, add the variant and its arm:

```rust
pub enum SearchScope {
    All,
    /// The lines from this one to the end (counting only what an append added).
    From(u64),
    /// The shown lines (ascending, as `FilteredView::shown_lines` yields them).
    Lines(Arc<Vec<u64>>),
}
```

```rust
            SearchScope::From(from) => run_filter_lines(text, &self.filter, *from..text.line_count(), chunk_lines, cancel).map(|r| SearchResult { lines: r.matches }),
```

(import `run_filter_lines` from `crate::engine`).

- [ ] **Step 5: Implement the follower**

Create `crates/rogg-core/src/follow.rs` and add `pub mod follow;` to `lib.rs`:

```rust
//! Live follow (spec §4.5): a thread that watches a text's file and reports growth (a new snapshot indexing only the
//! appended bytes), truncation and replacement. FSEvents through `notify` wakes it at once; a timer covers file
//! systems that send no events (network mounts) and files rogg writes itself.

use crate::cancel::CancelToken;
use crate::error::CoreError;
use crate::text::{LogText, Refresh};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// How often the file is checked when no event arrives.
pub const POLL_FALLBACK: Duration = Duration::from_millis(500);
/// Events arriving together (a writer flushing in pieces) are handled as one.
pub const DEBOUNCE: Duration = Duration::from_millis(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchMode {
    /// A file on disk: watch its folder for events and check whether the path now names another file (rotation).
    Path(PathBuf),
    /// Only poll the mapped file for growth (a `.gz` being decompressed, a capture rogg writes itself).
    Poll,
}

pub enum FollowEvent {
    Grew(Arc<LogText>),
    /// The file shrank (spec §4.5: re-index from scratch).
    Truncated,
    /// The path now names another file: rotated by rename, or deleted and recreated.
    Replaced,
    Failed(String),
}

pub struct Follower {
    stop: CancelToken,
    events: Receiver<FollowEvent>,
    /// Keeps the wake-up channel open when there is no watcher; dropping the follower closes it and ends the thread.
    _ping: Sender<()>,
    _watcher: Option<RecommendedWatcher>,
}

impl Follower {
    pub fn spawn(text: Arc<LogText>, mode: WatchMode, poll: Duration) -> Follower {
        let stop = CancelToken::new();
        let (tx, events) = mpsc::channel();
        let (ping, pings) = mpsc::channel();
        let watcher = match &mode {
            WatchMode::Path(path) => watch(path, ping.clone()),
            WatchMode::Poll => None,
        };
        let token = stop.clone();
        let _detached = std::thread::Builder::new().name("rogg-follow".into()).spawn(move || run(text, mode, poll, pings, tx, token));
        Follower { stop, events, _ping: ping, _watcher: watcher }
    }

    pub fn events(&self) -> &Receiver<FollowEvent> {
        &self.events
    }

    pub fn stop(&self) {
        self.stop.cancel();
    }
}

impl Drop for Follower {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

/// Watches the folder of `path` (a rotation replaces the file, so the folder is what stays).
fn watch(path: &Path, ping: Sender<()>) -> Option<RecommendedWatcher> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty())?;
    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        let _ = ping.send(());
    })
    .ok()?;
    watcher.watch(dir, RecursiveMode::NonRecursive).ok()?;
    Some(watcher)
}

/// The path now names a different file than the one mapped. A missing path is not a replacement (yet).
fn replaced(path: &Path, text: &LogText) -> bool {
    let Some(identity) = text.file_identity() else { return false };
    std::fs::metadata(path).is_ok_and(|meta| (meta.dev(), meta.ino()) != identity)
}

fn run(mut current: Arc<LogText>, mode: WatchMode, poll: Duration, pings: Receiver<()>, tx: Sender<FollowEvent>, stop: CancelToken) {
    loop {
        match pings.recv_timeout(poll) {
            Ok(()) => {
                std::thread::sleep(DEBOUNCE);
                while pings.try_recv().is_ok() {}
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if stop.is_cancelled() {
            return;
        }
        if let WatchMode::Path(path) = &mode
            && replaced(path, &current)
        {
            let _ = tx.send(FollowEvent::Replaced);
            return;
        }
        match current.refresh(&stop) {
            Ok(Refresh::Unchanged) => {}
            Ok(Refresh::Grew(text)) => {
                current = Arc::new(text);
                if tx.send(FollowEvent::Grew(current.clone())).is_err() {
                    return;
                }
            }
            Ok(Refresh::Truncated) => {
                let _ = tx.send(FollowEvent::Truncated);
                return;
            }
            Err(CoreError::Cancelled) => return,
            Err(e) => {
                let _ = tx.send(FollowEvent::Failed(e.to_string()));
                return;
            }
        }
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core`
Expected: PASS — `growth` (3 tests + property), `follow` (3), `filter_props` (3 properties), and everything before.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): growing texts, tail scans and the live-follow watcher

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Capturing stdin and FIFOs into the cache

**Files:**
- Create: `crates/rogg-core/src/stream.rs`
- Modify: `crates/rogg-core/src/lib.rs`
- Test: `crates/rogg-core/tests/stream.rs`

**Interfaces:**
- Consumes: `Cache::streams_dir`, `STREAMS_DIR` naming `stream-<pid>-<n>.log` (Task 2).
- Produces (`rogg_core::stream`):
  - `stdin_has_data() -> bool` (stdin is a pipe, FIFO, socket or redirected regular file — not a terminal or `/dev/null`); `fd_has_data(fd: RawFd) -> bool`; `is_stream_path(&Path) -> bool` (a FIFO or socket on disk, e.g. `<(cmd)` → `/dev/fd/63`)
  - `StreamCapture::start<R: Read + Send + 'static>(open: impl FnOnce() -> io::Result<R> + Send + 'static, cache: &Cache) -> Result<StreamCapture, CoreError>` — creates `streams/stream-<pid>-<n>.log` at once, then opens and copies the input on its own thread (opening a FIFO blocks until a writer appears, so it never happens on the caller's thread); `path() -> &Path`, `bytes() -> u64`, `is_done() -> bool`, `error() -> Option<String>`. The capture file is kept after the process ends and swept at the next start (`Cache::sweep_streams`).

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/tests/stream.rs`:

```rust
use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use rogg_core::stream::{StreamCapture, fd_has_data, is_stream_path};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

fn wait_for(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "condition not reached within 10 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_pipe_is_copied_as_it_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
    let (reader, mut writer) = std::io::pipe().unwrap();
    assert!(fd_has_data(reader.as_raw_fd()), "a pipe carries data");
    let capture = StreamCapture::start(move || Ok(reader), &cache).unwrap();
    assert!(capture.path().starts_with(cache.streams_dir()));
    assert!(capture.path().exists(), "the capture file exists before any data arrives");
    let name = capture.path().file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with(&format!("stream-{}-", std::process::id())) && name.ends_with(".log"), "{name}");
    writer.write_all(b"one\ntwo\n").unwrap();
    wait_for(|| std::fs::read(capture.path()).unwrap() == b"one\ntwo\n");
    assert!(!capture.is_done());
    writer.write_all(b"three\n").unwrap();
    drop(writer);
    wait_for(|| capture.is_done());
    assert_eq!(std::fs::read(capture.path()).unwrap(), b"one\ntwo\nthree\n");
    assert_eq!(capture.bytes(), 14);
    assert_eq!(capture.error(), None);
}

#[test]
fn opening_errors_are_reported_not_raised() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), DEFAULT_LIMIT_BYTES);
    let capture = StreamCapture::start(|| Err::<std::fs::File, _>(std::io::Error::other("no writer")), &cache).unwrap();
    wait_for(|| capture.is_done());
    assert!(capture.error().unwrap().contains("no writer"));
}

#[test]
fn terminals_and_dev_null_carry_no_data_but_fifos_do() {
    let null = std::fs::File::open("/dev/null").unwrap();
    assert!(!fd_has_data(null.as_raw_fd()));
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f.log");
    std::fs::write(&file, "x").unwrap();
    assert!(fd_has_data(std::fs::File::open(&file).unwrap().as_raw_fd()), "`rogg < file.log`");
    assert!(!is_stream_path(&file));
    let fifo = dir.path().join("fifo");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert!(is_stream_path(&fifo));
    assert!(!is_stream_path(&dir.path().join("missing")));
}
```

(`libc` is a regular `rogg-core` dependency since Task 1, so the test can call `libc::mkfifo`; `std::io::pipe` is stable since Rust 1.87.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test stream`
Expected: FAIL to compile — `rogg_core::stream` not found.

- [ ] **Step 3: Implement `stream.rs`**

Create `crates/rogg-core/src/stream.rs` and add `pub mod stream;` to `lib.rs`:

```rust
//! stdin and FIFOs (spec §4.1): data that arrives over time is copied into a cache file as it comes, and the tab
//! follows that file like a growing log.

use crate::cache::Cache;
use crate::error::CoreError;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::RawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Whether `fd` carries data to read: a pipe, FIFO, socket or regular file — not a terminal, `/dev/null` or other
/// device (an app started from Finder has `/dev/null` on stdin).
pub fn fd_has_data(fd: RawFd) -> bool {
    // SAFETY: `fstat` only writes into `st`; an invalid fd returns an error.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return false;
    }
    matches!(st.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK | libc::S_IFREG)
}

/// stdin when something is piped or redirected into rogg.
pub fn stdin_has_data() -> bool {
    fd_has_data(libc::STDIN_FILENO)
}

/// A FIFO or socket on disk (`mkfifo`, `<(cmd)` → `/dev/fd/63`): read it as a stream, never map it.
pub fn is_stream_path(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.file_type().is_fifo() || m.file_type().is_socket())
}

pub struct StreamCapture {
    path: PathBuf,
    bytes: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

impl StreamCapture {
    /// Creates `streams/stream-<pid>-<n>.log` and copies the input opened by `open` into it on a background thread,
    /// writing each read through at once so a follower sees it. `open` runs on that thread: opening a FIFO blocks
    /// until a writer appears.
    pub fn start<R: Read + Send + 'static>(open: impl FnOnce() -> io::Result<R> + Send + 'static, cache: &Cache) -> Result<StreamCapture, CoreError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = cache.streams_dir();
        std::fs::create_dir_all(&dir).map_err(CoreError::io(&dir))?;
        let path = dir.join(format!("stream-{}-{}.log", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let mut out = File::create(&path).map_err(CoreError::io(&path))?;
        let capture = StreamCapture {
            path,
            bytes: Arc::new(AtomicU64::new(0)),
            done: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
        };
        let (bytes, done, error) = (capture.bytes.clone(), capture.done.clone(), capture.error.clone());
        let spawned = std::thread::Builder::new().name("rogg-stream".into()).spawn(move || {
            let result = (|| -> io::Result<()> {
                let mut input = open()?;
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let n = match input.read(&mut buf) {
                        Ok(0) => return Ok(()),
                        Ok(n) => n,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(e),
                    };
                    out.write_all(&buf[..n])?;
                    bytes.fetch_add(n as u64, Ordering::Relaxed);
                }
            })();
            if let Err(e) = result {
                *error.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
            }
            done.store(true, Ordering::Release);
        });
        spawned.map_err(|source| CoreError::Io { path: capture.path.clone(), source })?;
        Ok(capture)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes captured so far.
    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// The input ended (or failed).
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --test stream && cargo test -p rogg-core && cargo clippy -p rogg-core --all-targets`
Expected: PASS; no new clippy warnings in `rogg-core`.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): capture stdin and FIFOs into the cache

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: rogg-ui — log-view tail mode, appended rows, tree-row notes and gallery states

**Files:**
- Modify: `crates/rogg-ui/src/logview/state.rs`, `crates/rogg-ui/src/logview/scroll.rs`, `crates/rogg-ui/src/logview/element.rs`, `crates/rogg-ui/src/sidebar.rs`
- Modify (exhaustive matches / struct literals): `crates/rogg-ui/tests/logview.rs`, `crates/rogg-ui/tests/sidebar.rs`, `crates/rogg-gallery/src/replica/state.rs`, `crates/rogg-gallery/src/fixtures.rs`, `crates/rogg-gallery/src/pages.rs`, `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/sidebar_model.rs`
- Test: `crates/rogg-ui/tests/logview.rs`, `crates/rogg-ui/tests/sidebar.rs`

**Interfaces:**
- Consumes: `LogViewState`, `ScrollGeometry`, `Sidebar`/`TreeRow` (M0/M2).
- Produces:
  - `LogViewEvent::TailLeft` — the user scrolled away from the end while in tail mode.
  - `LogViewState::set_tail(&mut self, on: bool, cx)`, `tail() -> bool`, `at_bottom() -> bool`, `extend_rows(&mut self, rows: Rc<dyn LogRowSource>, cx)` (rows that start with the current ones: keeps anchor, selection and wrapped heights).
  - `ScrollGeometry::is_at_bottom(&self, ScrollAnchor) -> bool`.
  - `TreeRow::Folder { label, expanded, depth, note: Option<SharedString>, failed: bool }` — `note` renders right-aligned (10 px, faint; error colour when `failed`) with id `tree-note-{i}`; a failed folder's label uses the error colour.
  - Gallery specimens: Sidebar page "Folder tree: listing, failed archive, nested members"; Chrome page pane headers with the live control (`Pause` / `Paused +1,204`); Empty states page `Extracting api.log`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-ui/tests/logview.rs`:

```rust
fn tail_left_count(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> Rc<RefCell<usize>> {
    let count = Rc::new(RefCell::new(0));
    let c = count.clone();
    cx.update(|cx| {
        cx.subscribe(state, move |_, ev: &LogViewEvent, _| {
            if *ev == LogViewEvent::TailLeft {
                *c.borrow_mut() += 1;
            }
        })
        .detach();
    });
    count
}

fn last_visible(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> usize {
    state.read_with(cx, |s, _| s.last_rows().last().map(|g| g.row).unwrap())
}

#[gpui_kit::test]
fn tail_mode_follows_appended_rows_until_the_user_scrolls_up(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let left = tail_left_count(cx, &state);
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.set_tail(true, cx));
    render(cx, w);
    assert_eq!(last_visible(cx, &state), 99);
    assert!(state.read_with(cx, |s, _| s.tail() && s.at_bottom()));
    state.update(cx, |s, cx| s.extend_rows(rows(150, None), cx));
    render(cx, w);
    assert_eq!(last_visible(cx, &state), 149, "appended rows scroll into view");
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-200.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(state.read_with(cx, |s, _| s.tail()), "scrolling down at the end stays in tail mode");
    assert_eq!(*left.borrow(), 0);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(100.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(!state.read_with(cx, |s, _| s.tail() || s.at_bottom()));
    assert_eq!(*left.borrow(), 1);
    state.update(cx, |s, cx| s.extend_rows(rows(200, None), cx));
    render(cx, w);
    assert!(last_visible(cx, &state) < 199, "once left, appended rows no longer pull the view");
}

#[gpui_kit::test]
fn jumping_to_an_earlier_row_leaves_tail_mode(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let left = tail_left_count(cx, &state);
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| {
        s.set_tail(true, cx);
        s.scroll_to_row(99, ScrollTo::Visible, cx);
    });
    render(cx, w);
    assert!(state.read_with(cx, |s, _| s.tail()), "the last row keeps tail mode");
    state.update(cx, |s, cx| s.scroll_to_row(10, ScrollTo::Center, cx));
    render(cx, w);
    assert!(!state.read_with(cx, |s, _| s.tail()));
    assert_eq!(*left.borrow(), 1);
}

#[gpui_kit::test]
fn extend_rows_keeps_the_position_and_selection(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| {
        s.set_selected(Some(1040), cx);
        s.scroll_to_row(40, ScrollTo::Top, cx);
    });
    render(cx, w);
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    state.update(cx, |s, cx| s.extend_rows(rows(300, None), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| (s.last_rows()[0].row, s.selected())), (first, Some(1040)));
}
```

In the same file, the event formatter of `click_selects_and_gutter_click_toggles_pin` gains an arm: `LogViewEvent::TailLeft => "tail-left".to_string(),`.

Append to `crates/rogg-ui/tests/sidebar.rs` (and add `note: None, failed: false` to the existing `TreeRow::Folder` literal at the top of the file):

```rust
#[gpui_kit::test]
fn folder_rows_show_a_note_and_a_failed_state(cx: &mut TestAppContext) {
    common::setup(cx);
    let rows = vec![
        TreeRow::Folder { label: "bundle".into(), expanded: true, depth: 0, note: None, failed: false },
        TreeRow::Folder { label: "inner.tgz".into(), expanded: true, depth: 1, note: Some("listing…".into()), failed: false },
        TreeRow::Folder { label: "broken.zip".into(), expanded: false, depth: 1, note: Some("unreadable".into()), failed: true },
    ];
    let w = common::open(cx, 211., 400., move |_, _| Sidebar::new().rows(rows.clone()).into_any_element());
    assert!(!common::exists(cx, w, "tree-note-0"));
    assert!(common::exists(cx, w, "tree-note-1") && common::exists(cx, w, "tree-note-2"));
    let row = common::bounds(cx, w, "tree-row-2");
    let note = common::bounds(cx, w, "tree-note-2");
    assert!(note.right() <= row.right() && note.right() >= row.right() - px(9.) - px(1.), "the note sits at the row's right padding");
}
```

(Use the file's existing imports; add `px` from `gpui_kit` if it is not imported yet.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --test logview --test sidebar`
Expected: FAIL to compile — `set_tail`, `extend_rows`, `LogViewEvent::TailLeft`, `TreeRow::Folder { note, failed }` not found.

- [ ] **Step 3: Implement tail mode and `extend_rows`**

In `crates/rogg-ui/src/logview/scroll.rs`, add to `impl ScrollGeometry`:

```rust
    /// Whether `a` shows the end of the content (the last row and the bottom padding); true when everything fits.
    pub fn is_at_bottom(&self, a: ScrollAnchor) -> bool {
        self.row_count == 0 || !before(a, self.bottom())
    }
```

In `crates/rogg-ui/src/logview/state.rs`:

1. Add the event variant:

```rust
    /// The user scrolled away from the end while in tail mode (live follow pauses on this).
    TailLeft,
```

2. Add a field `pub(crate) tail: bool,` to `LogViewState` (after `wrap`), initialised to `false` in `new`.

3. Add the methods (after `set_rows`):

```rust
    /// Like `set_rows` for rows that start with the current ones (lines were appended): keeps the scroll position, the
    /// selection and the measured wrapped heights.
    pub fn extend_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
        cx.notify();
    }

    /// Stick-to-bottom mode (live follow, spec §4.5): every layout keeps the last row in view, so appended rows scroll
    /// in. Scrolling away — the wheel, the scrollbar, or a jump to an earlier row — leaves it and emits `TailLeft`.
    pub fn set_tail(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tail = on;
        cx.notify();
    }

    pub fn tail(&self) -> bool {
        self.tail
    }

    /// Whether the view shows the end of its rows (true before the first layout and when everything fits).
    pub fn at_bottom(&self) -> bool {
        !self.initialized || self.with_geometry(|g| g.is_at_bottom(self.anchor))
    }

    /// After a user scroll: leaves tail mode when the end is no longer in view.
    pub(crate) fn user_scrolled(&mut self, cx: &mut Context<Self>) {
        if self.tail && !self.with_geometry(|g| g.is_at_bottom(self.anchor)) {
            self.tail = false;
            cx.emit(LogViewEvent::TailLeft);
        }
    }
```

4. In `scroll_to_row`, leave tail mode for a jump to any row but the last:

```rust
    pub fn scroll_to_row(&mut self, row: usize, how: ScrollTo, cx: &mut Context<Self>) {
        if self.tail && row + 1 < self.rows.row_count() {
            self.tail = false;
            cx.emit(LogViewEvent::TailLeft);
        }
        self.pending_scroll = Some((row, how));
        cx.notify();
    }
```

5. In `begin_layout` and `record_heights`, keep the end in view while tailing:

```rust
        self.anchor = match self.applied_scroll {
            Some(request) => self.resolve_scroll(request),
            None if self.tail => self.with_geometry(|g| g.bottom()),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
```

(the same three-arm `match` replaces the two-arm one in both methods).

In `crates/rogg-ui/src/logview/element.rs`, call `user_scrolled` after every user scroll:
- the `ScrollWheelEvent` listener: `s.scroll_by_px(-f32::from(delta.y), -f32::from(delta.x)); s.user_scrolled(cx); cx.notify();`
- both `drag_vthumb` calls (mouse down on the scrollbar, and `Drag::VThumb` in the mouse-move listener): call `s.user_scrolled(cx);` right after `s.drag_vthumb(..)`;
- in `LogViewState::autoscroll_select` (state.rs), call `self.user_scrolled(cx);` after `self.scroll_by_px(dy, 0.);`.

- [ ] **Step 4: Tree-row notes**

In `crates/rogg-ui/src/sidebar.rs`, extend the variant and its rendering:

```rust
pub enum TreeRow {
    /// A folder or an archive. `note` (10 px, right-aligned like a file size) tells what is happening (`listing…`);
    /// `failed` draws the label and the note in the error colour.
    Folder { label: SharedString, expanded: bool, depth: usize, note: Option<SharedString>, failed: bool },
    File { label: SharedString, size: SharedString, active: bool, depth: usize, failed: bool },
}
```

```rust
        TreeRow::Folder { label, expanded, depth, note, failed } => base
            .pl(px(9. + depth as f32 * 18.))
            .pr(px(9.))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if failed { p.error } else { p.text })
            .rounded(px(4.))
            .hover(move |s| s.bg(p.hover))
            .child(div().flex_none().text_color(p.muted).child(if expanded { "⌄" } else { "›" }))
            .child(icon(IconName::Folder).size(px(13.)).text_color(p.muted))
            .child(div().min_w_0().truncate().child(label))
            .when_some(note, |d, note| {
                d.child(
                    div()
                        .id(SharedString::from(format!("tree-note-{i}")))
                        .test_support()
                        .ml_auto()
                        .flex_none()
                        .text_size(px(10.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(if failed { p.error } else { p.faint })
                        .child(note),
                )
            })
            .into_any_element(),
```

Update the struct literals: `crates/rogg-gallery/src/fixtures.rs` (`sidebar_rows`), `crates/rogg-ui/tests/sidebar.rs`, and `crates/rogg-app/src/sidebar_model.rs` (`rows_with_failures`) gain `note: None, failed: false`. Exhaustive matches on `LogViewEvent` gain a no-op arm: `crates/rogg-gallery/src/replica/state.rs` (`LogViewEvent::TailLeft => {}`) and `crates/rogg-app/src/tab.rs` `on_log_event` (`LogViewEvent::TailLeft => {}` — Task 13 gives it meaning).

- [ ] **Step 5: Gallery specimens**

In `crates/rogg-gallery/src/fixtures.rs`, add:

```rust
/// A folder root with a nested archive being listed, an unreadable archive and a compressed log (M3 states).
pub fn sidebar_tree_rows() -> Vec<TreeRow> {
    vec![
        TreeRow::Folder { label: "support-2026-10-05".into(), expanded: true, depth: 0, note: None, failed: false },
        TreeRow::Folder { label: "nodes".into(), expanded: true, depth: 1, note: None, failed: false },
        TreeRow::File { label: "node-1.log".into(), size: "412 MB".into(), active: false, depth: 2, failed: false },
        TreeRow::Folder { label: "bundle-node-2.tgz".into(), expanded: true, depth: 1, note: Some("listing…".into()), failed: false },
        TreeRow::Folder { label: "broken.zip".into(), expanded: false, depth: 1, note: Some("unreadable".into()), failed: true },
        TreeRow::File { label: "app.log.1.gz".into(), size: "38 MB".into(), active: true, depth: 1, failed: false },
    ]
}
```

In `crates/rogg-gallery/src/pages.rs`:
- `sidebar(p)` returns a `row()` with the existing section plus:

```rust
        .child(section(
            "Folder tree: nested archive listing, unreadable archive, compressed log",
            frame(
                211.,
                640.,
                p,
                Sidebar::new()
                    .count("3 files".into())
                    .rows(fixtures::sidebar_tree_rows())
                    .footer(Some(SidebarFooter { title: "support-2026-10-05".into(), lines: vec!["3 files · 450 MB".into()] })),
            ),
            p,
        ))
```

(import `rogg_ui::sidebar::SidebarFooter`);
- in `chrome`, add two frames to the "Pane headers" column (no find field, so no input is rendered twice on the page):

```rust
                .child(frame(1100., 40., p, PaneHeader::new("ph-live", "Full log", "1,204,331 lines").tool(ToolButton::new("ph-live-pause", IconName::Pause, "Pause")).tool(ToolButton::new("ph-live-wrap", IconName::Wrap, "Wrap"))))
                .child(frame(
                    1100.,
                    40.,
                    p,
                    PaneHeader::new("ph-paused", "Full log", "1,205,535 lines")
                        .tool(ToolButton::new("ph-live-paused", IconName::Pause, "Paused").pressed(true).badge(Some("+1,204".into())))
                        .tool(ToolButton::new("ph-paused-wrap", IconName::Wrap, "Wrap")),
                ))
```

- in `empty_states`, add `.child(frame(600., 300., p, empty::loading("Extracting api.log", Some(0.42), noop.clone())))` before the error frame.

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui && cargo test -p rogg-gallery && cargo test -p rogg-app && just visual-check; echo "visual-check exit $?"`
Expected: rogg-ui, rogg-gallery and rogg-app tests PASS. `just visual-check` fails only on the 9 replica captures that already failed at the end of M2 (the horizontal scrollbar, ruling R3 — pending with the user); the replica sidebar is pixel-identical because `note: None, failed: false` renders exactly as before. Do **not** run `npm run approve`.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-ui crates/rogg-gallery crates/rogg-app
git commit -m "feat(ui): log-view tail mode, appended rows, tree-row notes and gallery states

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: App — the cache store and opening every kind of source

**Files:**
- Create: `crates/rogg-app/src/cache.rs`
- Modify: `crates/rogg-app/Cargo.toml` (dev-dependency `rogg-genlog`), `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/paths.rs` (doc only), `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/document/load.rs`, `crates/rogg-app/src/tab_view.rs`, `crates/rogg-app/src/workspace/mod.rs`
- Test: `crates/rogg-app/tests/sources.rs`

**Interfaces:**
- Consumes: `Cache`, `DEFAULT_LIMIT_BYTES` (Task 2); `SourceRef`, `open_source`, `OpenedSource`, `OpenUpdate`, `OpenStage`, `Inflating`, `InflatePoll` (Tasks 5–6); `Follower`, `WatchMode`, `FollowEvent` (Task 7); `sniff_head` (Task 4).
- Produces:
  - `rogg_app::cache::{CacheStore(pub Arc<Cache>) (Global), CacheStore::get(&App) -> Arc<Cache>, start_maintenance(Arc<Cache>), evict_soon(&App)}`; `install_globals` installs the store and starts the maintenance thread (sweep partials, sweep streams, evict).
  - `Source::{File(PathBuf), Member(SourceRef), Stream { name: SharedString, path: PathBuf }, Memory { name, bytes }}` with `title()`, `path()` (files only), `source_ref() -> Option<SourceRef>`, `crumbs() -> Vec<SharedString>`, `tooltip() -> SharedString`, `location() -> SharedString` (error-state line).
  - `DocumentEvent { Grew { lines: u64 }, Reloaded }` (`Document: EventEmitter<DocumentEvent>`; `Reloaded` is emitted from Task 11).
  - `Document::load_revision() -> u64` (bumped on each load from scratch; `text_revision` is bumped on loads *and* growth), `Document::job_progress() -> Option<(SharedString, f32)>` (`Decompressing N%`, else `Filtering N%`).
  - Loading labels `Extracting <name>`, `Decompressing <name>`, `Converting UTF-16 to UTF-8`, `Indexing`.
  - Documents follow growth from this task on (plain files and streams through `WatchMode::Path`, a decompressing `.gz` through `WatchMode::Poll` every `INFLATE_POLL` = 100 ms); here a growth simply rescans — Task 11 makes it incremental and handles truncation.

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-app/Cargo.toml`, add to `[dev-dependencies]`: `rogg-genlog = { path = "../rogg-genlog" }`.

Create `crates/rogg-app/tests/sources.rs`:

```rust
mod common;
use gpui_kit::{AppContext, Entity, SharedString, TestAppContext};
use rogg_app::cache::CacheStore;
use rogg_app::document::{Document, DocumentInit, LoadState, Source};
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, write_archive, write_gz, write_stored_zip};
use std::path::Path;

fn open(cx: &mut TestAppContext, source: Source) -> Entity<Document> {
    cx.new(|cx| Document::new(source, DocumentInit::default(), cx))
}

fn lines(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<Vec<String>> {
    doc.read_with(cx, |d, _| d.text().map(|t| (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()))
}

fn failure(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| match &d.load {
        LoadState::Failed { message } => Some(message.to_string()),
        _ => None,
    })
}

fn files_with_ext(dir: &Path, ext: &str) -> usize {
    std::fs::read_dir(dir).map(|r| r.flatten().filter(|e| e.path().extension().is_some_and(|x| x == ext)).count()).unwrap_or(0)
}

fn member(zip: &Path, path: &str) -> Source {
    Source::Member(SourceRef::member(SourceRef::File(zip.to_path_buf()), path))
}

#[gpui_kit::test]
fn archive_members_open_in_the_background(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("bundle.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"one\ntwo error\nthree\n")]).unwrap();
    let source = member(&zip, "logs/api.log");
    assert_eq!(source.title(), SharedString::from("api.log"));
    assert_eq!(source.crumbs(), ["bundle.zip", "logs", "api.log"].map(SharedString::from));
    assert_eq!(source.tooltip(), SharedString::from("api.log · bundle.zip"));
    assert_eq!(source.location(), SharedString::from("bundle.zip › logs › api.log"));
    assert!(source.path().is_none());
    let doc = open(cx, source);
    assert!(matches!(doc.read_with(cx, |d, _| d.load.clone()), LoadState::Loading { .. }), "members always open off the UI thread");
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    assert_eq!(lines(&doc, cx).unwrap(), ["one", "two error", "three"]);
    assert_eq!(doc.read_with(cx, |d, _| (d.load_revision(), d.text_revision())), (1, 1));
}

#[gpui_kit::test]
fn open_members_are_leased_until_their_tab_closes(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("bundle.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    let doc = open(cx, member(&zip, "a.log"));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    let cache = cx.update(|cx| CacheStore::get(cx));
    assert_eq!(files_with_ext(cache.dir(), "member"), 1);
    assert_eq!(cache.evict_to(0).removed, 0, "the open member is leased");
    drop(doc);
    cx.run_until_parked();
    assert_eq!(cache.evict_to(0).removed, 1, "closing the tab releases it");
    assert_eq!(files_with_ext(cache.dir(), "member"), 0);
}

#[gpui_kit::test]
fn gz_files_decompress_into_a_growing_text(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let body: String = (0..50_000).map(|i| format!("line {i}\n")).collect();
    let gz = dir.path().join("app.log.1.gz");
    write_gz(&gz, body.as_bytes()).unwrap();
    let doc = open(cx, Source::File(gz));
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_count() == 50_000) && d.job_progress().is_none()));
    let all = lines(&doc, cx).unwrap();
    assert_eq!((all[0].as_str(), all[49_999].as_str()), ("line 0", "line 49999"));
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 1, "growth while decompressing is not a reload");
}

#[gpui_kit::test]
fn a_damaged_member_shows_the_error_state_and_others_still_open(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let bad = open(cx, member(&zip, "logs/api.log"));
    common::wait_until(cx, |cx| failure(&bad, cx).is_some());
    assert!(failure(&bad, cx).unwrap().contains("logs/api.log"));
    let good = open(cx, member(&zip, "README"));
    common::wait_until(cx, |cx| lines(&good, cx).is_some());
    assert_eq!(lines(&good, cx).unwrap(), ["read me"]);
}

#[gpui_kit::test]
fn archives_opened_as_files_say_so(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    let doc = open(cx, Source::File(zip));
    common::wait_until(cx, |cx| failure(&doc, cx).is_some());
    assert!(failure(&doc, cx).unwrap().contains("is an archive"));
}

#[gpui_kit::test]
fn small_utf16_files_lease_their_conversion(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("w.log");
    std::fs::write(&path, [0xFF, 0xFE, b'h', 0, b'i', 0, b'\n', 0]).unwrap();
    let doc = open(cx, Source::File(path));
    assert_eq!(lines(&doc, cx).unwrap(), ["hi"], "small files still open synchronously");
    let cache = cx.update(|cx| CacheStore::get(cx));
    assert_eq!(files_with_ext(cache.dir(), "utf8"), 1);
    assert_eq!(cache.evict_to(0).removed, 0);
    drop(doc);
    cx.run_until_parked();
    assert_eq!(cache.evict_to(0).removed, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test sources`
Expected: FAIL to compile — `rogg_app::cache`, `Source::Member`, `load_revision`, … not found.

- [ ] **Step 3: The cache store**

Create `crates/rogg-app/src/cache.rs`:

```rust
//! The app's cache (spec §4.2): one `rogg_core::cache::Cache` shared by every document, so leases from all tabs keep
//! their entries; swept and trimmed on background threads.

use gpui_kit::{App, Global};
use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use std::sync::Arc;

pub struct CacheStore(pub Arc<Cache>);

impl Global for CacheStore {}

impl CacheStore {
    /// The shared cache (a fresh one under the default directory when the app globals are not installed).
    pub fn get(cx: &App) -> Arc<Cache> {
        cx.try_global::<CacheStore>().map(|s| s.0.clone()).unwrap_or_else(|| Cache::new(rogg_core::paths::cache_dir(), DEFAULT_LIMIT_BYTES))
    }
}

/// Startup maintenance: removes partial files and stream captures left by processes that died, then trims the cache
/// to its limit (M1 carry-over: stale `*.partial` sweep).
pub fn start_maintenance(cache: Arc<Cache>) {
    let _detached = std::thread::Builder::new().name("rogg-cache".into()).spawn(move || {
        cache.sweep_partials();
        cache.sweep_streams();
        cache.evict();
    });
}

/// Trims the cache on a background thread (after a document wrote new entries).
pub fn evict_soon(cx: &App) {
    let cache = CacheStore::get(cx);
    let _detached = std::thread::Builder::new().name("rogg-cache-evict".into()).spawn(move || {
        cache.evict();
    });
}
```

In `crates/rogg-app/src/lib.rs`, add `pub mod cache;` and, at the start of `install_globals`:

```rust
    let store = rogg_core::cache::Cache::new(paths.cache.clone(), rogg_core::cache::DEFAULT_LIMIT_BYTES);
    cache::start_maintenance(store.clone());
    cx.set_global(cache::CacheStore(store));
```

In `crates/rogg-app/src/paths.rs`, the `cache` field's doc becomes `/// The cache directory (spec §4.2): conversions, extracted members, decompressed files, stream captures.`

- [ ] **Step 4: Sources and the open pipeline**

Replace `crates/rogg-app/src/document/load.rs` with:

```rust
//! Opening a source on a background thread — extraction, decompression, conversion, indexing; the document polls the
//! channel.

use rogg_core::cache::Cache;
use rogg_core::source::{OpenUpdate, OpenedSource, SourceRef, open_source};
use rogg_core::{CancelToken, CoreError};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum Message {
    Progress(OpenUpdate),
    Done(Result<OpenedSource, CoreError>),
}

pub(crate) enum OpenPoll {
    /// Still working; the latest progress since the last poll, if any.
    Pending(Option<OpenUpdate>),
    Done(Result<OpenedSource, CoreError>),
    /// The thread ended without a result (it panicked or could not be spawned).
    Died,
}

pub(crate) struct OpenJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
}

impl OpenJob {
    pub(crate) fn spawn(source: SourceRef, cache: Arc<Cache>) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        // Detached: on cancel, `open_source` returns at its next check and the thread ends.
        let _detached = std::thread::Builder::new().name("rogg-open".into()).spawn(move || {
            let progress = tx.clone();
            let result = open_source(&source, &cache, &token, |u| {
                let _ = progress.send(Message::Progress(u));
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

In `crates/rogg-app/src/document/mod.rs`:

1. Update the module doc's first sentence to "One open log: its source, its text once loaded, the cache entries it reads and the background jobs working on it." and the imports:

```rust
use crate::cache::{CacheStore, evict_soon};
use gpui_kit::{App, Context, EventEmitter, Global, SharedString};
use load::{OpenJob, OpenPoll};
use rogg_core::archive::{Content, sniff_head};
use rogg_core::cache::CacheLease;
use rogg_core::filter::FilterSpec;
use rogg_core::follow::{FollowEvent, Follower, POLL_FALLBACK, WatchMode};
use rogg_core::source::{InflatePoll, Inflating, OpenStage, OpenUpdate, OpenedSource, SourceRef, open_source};
use rogg_core::text::LogText;
use rogg_core::{CancelToken, CoreError};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;
```

2. After `POLL_INTERVAL`, add:

```rust
/// A `.gz` being decompressed is checked this often for new bytes.
pub const INFLATE_POLL: Duration = Duration::from_millis(100);
```

3. Replace `Source` and its impl:

```rust
#[derive(Clone, Debug)]
pub enum Source {
    /// A file on disk: plain text (live-followed) or `.gz` (decompressed into the cache).
    File(PathBuf),
    /// A member of an archive, possibly nested (spec §4.1).
    Member(SourceRef),
    /// Captured stdin or a FIFO: a cache file that grows while data arrives (spec §4.1).
    Stream { name: SharedString, path: PathBuf },
    /// In-memory text (the demo and tests); `name` is the tab title.
    Memory { name: SharedString, bytes: Arc<Vec<u8>> },
}

impl Source {
    pub fn title(&self) -> SharedString {
        match self {
            Source::File(path) => SourceRef::File(path.clone()).name().into(),
            Source::Member(source) => source.name().into(),
            Source::Stream { name, .. } | Source::Memory { name, .. } => name.clone(),
        }
    }

    /// The file on disk for plain and `.gz` files.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Source::File(path) => Some(path),
            _ => None,
        }
    }

    /// Files and members as the source tree and the recent list know them.
    pub fn source_ref(&self) -> Option<SourceRef> {
        match self {
            Source::File(path) => Some(SourceRef::File(path.clone())),
            Source::Member(source) => Some(source.clone()),
            _ => None,
        }
    }

    /// The info-bar breadcrumb (`incident.zip › logs › api.log`).
    pub fn crumbs(&self) -> Vec<SharedString> {
        match self.source_ref() {
            Some(source) => source.crumbs().into_iter().map(SharedString::from).collect(),
            None => vec![self.title()],
        }
    }

    /// The tab tooltip: a file's path, `api.log · incident.zip` for a member.
    pub fn tooltip(&self) -> SharedString {
        self.source_ref().map(|s| SharedString::from(s.tooltip())).unwrap_or_else(|| self.title())
    }

    /// Where the source is, for the error state: a file's path, a member's breadcrumb.
    pub fn location(&self) -> SharedString {
        match self {
            Source::File(path) => path.display().to_string().into(),
            Source::Member(source) => source.crumbs().join(" › ").into(),
            Source::Stream { name, .. } => name.clone(),
            Source::Memory { .. } => SharedString::default(),
        }
    }
}
```

4. After `LoadState`, add the event and replace the `Document` struct, `loading`, and the load path (keep `EngineConfig`, `DocumentInit`, `LoadState`, `ensure_polling` and `Drop` as they are):

```rust
/// What a document tells its tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentEvent {
    /// The text grew by `lines` lines (live follow, a `.gz` decompressing).
    Grew { lines: u64 },
    /// The file was truncated or replaced and is being reloaded (spec §4.5).
    Reloaded,
}

pub struct Document {
    pub source: Source,
    pub load: LoadState,
    pub filter: FilterState,
    search: search::SearchState,
    number_base: u64,
    text_revision: u64,
    load_revision: u64,
    open_job: Option<OpenJob>,
    /// Cache entries the text reads (spec §4.2: never evicted while open).
    leases: Vec<CacheLease>,
    /// A `.gz` still decompressing into the text's file.
    inflating: Option<Inflating>,
    /// Size of the decompressed file once `inflating` finished; the follower stops when the text reaches it.
    inflated_len: Option<u64>,
    /// Watches the text's file for growth (live follow, a decompressing `.gz`).
    follower: Option<Follower>,
    polling: bool,
}

impl EventEmitter<DocumentEvent> for Document {}

fn loading(update: &OpenUpdate) -> LoadState {
    let label = match update.stage {
        OpenStage::Extracting => format!("Extracting {}", update.name),
        OpenStage::Decompressing => format!("Decompressing {}", update.name),
        OpenStage::Transcoding => "Converting UTF-16 to UTF-8".to_string(),
        OpenStage::Indexing => "Indexing".to_string(),
    };
    let fraction = if update.total == 0 { 0. } else { update.done as f32 / update.total as f32 };
    LoadState::Loading { fraction, label: label.into() }
}

/// Whether `path` must go through the source pipeline off the UI thread (a `.gz` or an archive), even when small.
fn needs_pipeline(path: &Path) -> bool {
    let mut head = [0u8; 512];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    matches!(sniff_head(&head[..n]), Content::Gzip | Content::Archive(_))
}

fn in_memory(text: LogText) -> OpenedSource {
    OpenedSource { text, leases: Vec::new(), inflating: None, watch: None }
}
```

In `impl Document`, `new` initialises the new fields (`load_revision: 0, leases: Vec::new(), inflating: None, inflated_len: None, follower: None`). Add the accessors and replace `start_load`, `finish_load`, `poll_jobs`, `poll_open`:

```rust
    /// Bumped each time a text is loaded from scratch (open, retry, reload) — not when it grows.
    pub fn load_revision(&self) -> u64 {
        self.load_revision
    }

    /// The footer progress: a `.gz` decompressing, else a running background scan.
    pub fn job_progress(&self) -> Option<(SharedString, f32)> {
        if let Some(inflating) = &self.inflating {
            let f = inflating.fraction();
            return Some((format!("Decompressing {:.0}%", f * 100.).into(), f));
        }
        self.filter.progress.map(|f| (format!("Filtering {:.0}%", f * 100.).into(), f))
    }

    fn stop_jobs(&mut self) {
        self.open_job = None;
        self.follower = None;
        self.inflating = None;
        self.inflated_len = None;
    }

    fn start_load(&mut self, cx: &mut Context<Self>) {
        self.stop_jobs();
        self.load = LoadState::Loading { fraction: 0., label: "Opening".into() };
        let cache = CacheStore::get(cx);
        match self.source.clone() {
            Source::Memory { bytes, .. } => self.finish_load(Ok(in_memory(LogText::from_bytes(bytes.to_vec()))), cx),
            Source::Stream { path, .. } => {
                let result = LogText::open(&path, cache.dir(), &CancelToken::new(), |_| {});
                self.finish_load(result.map(|text| OpenedSource { text, leases: Vec::new(), inflating: None, watch: Some(path) }), cx);
            }
            Source::File(path) => match std::fs::metadata(&path) {
                Err(source) => self.finish_load(Err(CoreError::Io { path, source }), cx),
                Ok(meta) if meta.is_file() && meta.len() <= EngineConfig::get(cx).sync_bytes && !needs_pipeline(&path) => {
                    let result = open_source(&SourceRef::File(path), &cache, &CancelToken::new(), |_| {});
                    self.finish_load(result, cx);
                }
                Ok(_) => self.spawn_open(SourceRef::File(path), cx),
            },
            Source::Member(source) => self.spawn_open(source, cx),
        }
    }

    fn spawn_open(&mut self, source: SourceRef, cx: &mut Context<Self>) {
        self.open_job = Some(OpenJob::spawn(source, CacheStore::get(cx)));
        cx.notify();
        self.ensure_polling(cx);
    }

    fn finish_load(&mut self, result: Result<OpenedSource, CoreError>, cx: &mut Context<Self>) {
        self.load = match result {
            Ok(opened) => {
                self.text_revision += 1;
                self.load_revision += 1;
                if !opened.leases.is_empty() {
                    evict_soon(cx);
                }
                self.leases = opened.leases;
                let text = Arc::new(opened.text);
                if let Some(inflating) = opened.inflating {
                    self.inflating = Some(inflating);
                    self.follower = Some(Follower::spawn(text.clone(), WatchMode::Poll, INFLATE_POLL));
                } else if let Some(path) = opened.watch {
                    self.follower = Some(Follower::spawn(text.clone(), WatchMode::Path(path), POLL_FALLBACK));
                }
                LoadState::Ready(text)
            }
            Err(CoreError::Cancelled) => LoadState::Failed { message: "Opening cancelled".into() },
            // The error state shows the path on its own line, so the title is just the OS error.
            Err(CoreError::Io { source, .. }) => LoadState::Failed { message: source.to_string().into() },
            Err(e) => LoadState::Failed { message: e.to_string().into() },
        };
        if matches!(self.load, LoadState::Ready(_)) {
            self.start_filter(cx);
            self.refresh_search(Pane::Full, cx);
            if self.follower.is_some() {
                self.ensure_polling(cx);
            }
        }
        cx.notify();
    }

    /// Drains every job channel once; returns whether any job is still running.
    fn poll_jobs(&mut self, cx: &mut Context<Self>) -> bool {
        let active = self.poll_open(cx) | self.poll_inflate(cx) | self.poll_follow(cx) | self.poll_filter(cx) | self.poll_search(cx);
        if !active {
            self.polling = false;
        }
        active
    }

    fn poll_open(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(job) = &self.open_job else { return false };
        match job.poll() {
            OpenPoll::Pending(Some(update)) => {
                self.load = loading(&update);
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

    fn poll_inflate(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(inflating) = &mut self.inflating else { return false };
        let before = inflating.progress();
        match inflating.poll() {
            InflatePoll::Running => {
                if inflating.progress() != before {
                    cx.notify();
                }
                true
            }
            InflatePoll::Done(Ok(())) => {
                self.inflated_len = std::fs::metadata(inflating.dst()).ok().map(|m| m.len());
                self.inflating = None;
                cx.notify();
                self.follower.is_some()
            }
            InflatePoll::Done(Err(e)) => {
                self.stop_jobs();
                self.load = LoadState::Failed { message: e.to_string().into() };
                cx.notify();
                false
            }
        }
    }

    fn poll_follow(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(follower) = &self.follower else { return false };
        let mut grew = None;
        let mut ended = false;
        loop {
            match follower.events().try_recv() {
                Ok(FollowEvent::Grew(text)) => grew = Some(text),
                Ok(FollowEvent::Truncated | FollowEvent::Replaced | FollowEvent::Failed(_)) | Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if let Some(text) = grew {
            self.on_grew(text, cx);
        }
        if ended {
            self.follower = None;
        }
        // A finished decompression needs no watching once the text has caught up with the complete file.
        if let (Some(len), Some(text)) = (self.inflated_len, self.text())
            && text.mapped_len() >= len
        {
            self.follower = None;
            self.inflated_len = None;
        }
        self.follower.is_some()
    }

    /// A grown snapshot of the text. Rescans for now; Task 11 scans only the appended lines.
    fn on_grew(&mut self, text: Arc<LogText>, cx: &mut Context<Self>) {
        let added = text.line_count().saturating_sub(self.text().map_or(0, |t| t.line_count()));
        self.load = LoadState::Ready(text);
        self.text_revision += 1;
        self.start_filter(cx);
        self.refresh_search(Pane::Full, cx);
        cx.emit(DocumentEvent::Grew { lines: added });
        cx.notify();
    }
```

Delete the old `use crate::paths::AppPaths;` and `OpenProgress` imports (the cache directory now comes from `CacheStore`).

- [ ] **Step 5: Use the source's labels in the window**

In `crates/rogg-app/src/tab_view.rs`:
- crumbs: `let crumbs = t.labels.crumbs.clone().unwrap_or_else(|| d.source.crumbs());`
- progress: `let progress = d.job_progress().map(|(label, fraction)| Progress { label, fraction });`
- error placeholder: `let path = d.source.location();` (passed to `empty::error` instead of the file path).

In `crates/rogg-app/src/workspace/mod.rs` `titlebar`, the tooltip becomes `t.labels.tooltip.clone().unwrap_or_else(|| d.source.tooltip())`.

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: PASS — the 6 new tests and every M2 test (files still open synchronously below 4 MiB; the demo and gallery are unaffected).

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-app Cargo.lock
git commit -m "feat(app): open archive members and .gz files through the shared cache

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: App — live documents: incremental filtering and search, reload on truncation or replacement

**Files:**
- Modify: `crates/rogg-app/src/document/mod.rs`, `crates/rogg-app/src/document/filter.rs`, `crates/rogg-app/src/document/search.rs`, `crates/rogg-app/src/tab.rs`
- Test: `crates/rogg-app/tests/live.rs`

**Interfaces:**
- Consumes: `TailBase`, `run_filter_lines`, `FilterJob::spawn_from`, `SearchScope::From`, `LogText::{stable_lines, mapped_len}`, `FollowEvent` (Task 7); `DocumentEvent`, follower wiring (Task 10); `LogViewState::extend_rows` (Task 9).
- Produces:
  - `Document::is_live() -> bool` — a stream, or a file that has grown since it was opened (not a `.gz` still decompressing).
  - Growth scans only the appended lines (plus an unfinished last line): `FilterState::tail_scans: u64` counts tail scans, `FilterState::scans` still counts full scans; a scan that finishes after the text grew is followed by a tail scan; the full log's search is extended the same way.
  - `FilterState::pins_revision: u64`, bumped when pins change; the full log rows are rebuilt on text, pins or search changes — not on filter publishes (M2 carry-over) — and *extended* (`extend_rows`, wrapped heights kept) when only the text grew.
  - Truncation or replacement (spec §4.5): pins are cleared, `DocumentEvent::Reloaded` is emitted, and the file is loaded from scratch (`load_revision` + 1). Task 13 turns the event into the toast `File was truncated or replaced — reloaded.`
  - `Document::poll_jobs` keeps polling while any job exists (a reload started inside the poll is polled too).

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/live.rs`:

```rust
mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentEvent, DocumentInit, EngineConfig, Pane, Source};
use rogg_core::filter::FilterSpec;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

fn append(path: &Path, s: &str) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(s.as_bytes()).unwrap();
}

fn doc_for(cx: &mut TestAppContext, path: &Path, expr: &str) -> Entity<Document> {
    let init = DocumentInit { spec: FilterSpec::from_expression(expr), ..Default::default() };
    cx.new(|cx| Document::new(Source::File(path.to_path_buf()), init, cx))
}

fn line_count(doc: &Entity<Document>, cx: &mut TestAppContext) -> u64 {
    doc.read_with(cx, |d, _| d.text().map_or(0, |t| t.line_count()))
}

fn matched(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.matches.to_vec())
}

fn events(cx: &mut TestAppContext, doc: &Entity<Document>) -> Rc<RefCell<Vec<DocumentEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    cx.update(|cx| cx.subscribe(doc, move |_, ev: &DocumentEvent, _| s.borrow_mut().push(ev.clone())).detach());
    seen
}

#[gpui_kit::test]
fn appended_lines_are_indexed_filtered_and_counted(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "live.log", &["error one", "ok", ""]);
    let doc = doc_for(cx, &path, "error");
    let seen = events(cx, &doc);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "ok", cx));
    assert_eq!(matched(&doc, cx), [0]);
    assert!(!doc.read_with(cx, |d, _| d.is_live()));
    append(&path, "ok again\nerror two\n");
    common::wait_until(cx, |cx| line_count(&doc, cx) == 4);
    assert_eq!(matched(&doc, cx), [0, 3]);
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.scans, d.filter.tail_scans)), (1, 1), "only the appended lines were scanned");
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Full).result.as_ref().map(|r| r.lines.clone())), Some(vec![1, 2]));
    assert!(doc.read_with(cx, |d, _| d.is_live()));
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 1);
    assert!(seen.borrow().contains(&DocumentEvent::Grew { lines: 2 }));
}

#[gpui_kit::test]
fn an_unfinished_last_line_is_rescanned_when_completed(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\nerr").unwrap();
    let doc = doc_for(cx, &path, "error");
    assert!(matched(&doc, cx).is_empty());
    append(&path, "or here\n");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_text(1) == "error here")));
    assert_eq!(matched(&doc, cx), [1]);
    let hits = doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().term_hits[0].lines);
    assert_eq!(hits, 1, "the completed line counts once");
}

#[gpui_kit::test]
fn appends_during_a_running_scan_are_scanned_once(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let body: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let path = dir.path().join("big.log");
    std::fs::write(&path, body).unwrap();
    let doc = doc_for(cx, &path, "alpha");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_some()));
    append(&path, &"alpha tail\n".repeat(1000));
    common::wait_until(cx, |cx| {
        doc.read_with(cx, |d, _| d.filter.progress.is_none() && d.filter.matches.len() == 101_000 && d.text().is_some_and(|t| t.line_count() == 201_000))
    });
    let found = matched(&doc, cx);
    assert!(found.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
    assert_eq!(*found.last().unwrap(), 200_999);
    let hits = doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().term_hits[0].lines);
    assert_eq!(hits, 101_000);
}

#[gpui_kit::test]
fn truncation_reloads_with_a_toast_and_drops_pins(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "t.log", &["a", "b", "c", "d", ""]);
    let doc = doc_for(cx, &path, "a");
    let seen = events(cx, &doc);
    doc.update(cx, |d, cx| d.toggle_pin(2, cx));
    // `logrotate copytruncate`: the file is emptied in place, then written again.
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    append(&path, "new one\n");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some()));
    assert!(seen.borrow().contains(&DocumentEvent::Reloaded));
    assert!(doc.read_with(cx, |d, _| d.filter.pins.is_empty()), "pins referred to the old content");
    common::wait_until(cx, |cx| line_count(&doc, cx) == 1);
}

#[gpui_kit::test]
fn a_replaced_file_is_reloaded(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "r.log", &["old", ""]);
    let doc = doc_for(cx, &path, "new");
    let fresh = common::write_log(dir.path(), "r.log.1", &["new one", "new two", ""]);
    std::fs::rename(&fresh, &path).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some_and(|t| t.line_count() == 2)));
    assert_eq!(matched(&doc, cx), [0, 1]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test live`
Expected: FAIL to compile — `is_live`, `tail_scans`, `PaneSearch::result` lines, … missing; once compiling, `appended_lines_are_indexed_filtered_and_counted` fails on `(scans, tail_scans)` (growth rescans everything) and the truncation tests time out.

- [ ] **Step 3: Tail scans in the filter pipeline**

In `crates/rogg-app/src/document/filter.rs`:

1. Imports: add `use rogg_core::engine::{TailBase, run_filter_lines};` and `use rogg_core::text::LogText;`.

2. Add fields to `FilterState` (initialised to `None` / `0` in `new`):

```rust
    /// The text `matches` and `term_hits` are complete for; None before the first finished scan.
    scanned: Option<Arc<LogText>>,
    /// The text the running scan reads.
    job_text: Option<Arc<LogText>>,
    /// What the running tail scan adds to; None while a full scan runs.
    tail_base: Option<TailBase>,
    /// Tail scans started so far (tests check that appends never rescan the whole file).
    pub tail_scans: u64,
    /// Bumped when pins change: the full log rows depend on pins, not on the filtered view.
    pub pins_revision: u64,
```

3. `toggle_pin` bumps `self.filter.pins_revision += 1;` before `rebuild_view`.

4. In `start_filter`, after retiring the old job, reset `self.filter.scanned = None; self.filter.job_text = None; self.filter.tail_base = None;`; in its async branch set `self.filter.job_text = Some(text.clone());` before spawning; its sync branch passes the text to `finish_scan`:

```rust
            match run_filter(&text, &filter, DEFAULT_CHUNK_LINES, &CancelToken::new()) {
                Ok(result) => self.finish_scan(result.matches, result.term_hits, result.duration, Some(text), cx),
```

5. Add the tail scan:

```rust
    /// Scans only the lines appended since the last finished scan (spec §4.5), re-scanning that text's unfinished last
    /// line, and merges the result. Small appends are scanned here; large ones as a background tail job.
    pub(super) fn scan_tail(&mut self, cx: &mut Context<Self>) {
        let (Some(text), Some(filter)) = (self.text().cloned(), self.filter.compiled.clone()) else { return };
        let Some(prev) = self.filter.scanned.clone() else {
            self.start_filter(cx);
            return;
        };
        if !filter.is_active() {
            self.filter.scanned = Some(text);
            self.rebuild_view(cx);
            return;
        }
        let base = TailBase::new(&prev, &filter, &self.filter.matches, &self.filter.term_hits);
        self.filter.tail_scans += 1;
        if text.mapped_len().saturating_sub(prev.mapped_len()) <= EngineConfig::get(cx).sync_bytes {
            match run_filter_lines(&text, &filter, base.from..text.line_count(), DEFAULT_CHUNK_LINES, &CancelToken::new()) {
                Ok(result) => {
                    let (matches, hits) = base.merge(result.matches, &result.term_hits);
                    let duration = self.filter.duration;
                    self.finish_scan(matches, hits, duration, Some(text), cx);
                }
                Err(e) => {
                    self.filter.error = Some(e.to_string().into());
                    cx.notify();
                }
            }
        } else {
            let from = base.from;
            self.filter.partial.clear();
            self.filter.dirty = false;
            self.filter.tail_base = Some(base);
            self.filter.job_text = Some(text.clone());
            self.filter.job = Some(FilterJob::spawn_from(text, filter, from, DEFAULT_CHUNK_LINES));
            self.ensure_polling(cx);
        }
    }

    /// The text grew past what the last finished scan covered (more lines, or a longer unfinished last line).
    fn grew_since_scan(&self) -> bool {
        match (self.text(), &self.filter.scanned) {
            (Some(text), Some(scanned)) => text.mapped_len() > scanned.mapped_len(),
            _ => false,
        }
    }
```

6. `poll_filter`: a tail job neither reports progress nor publishes partial results (its context would flicker), and its result is merged:

```rust
                Ok(FilterEvent::Progress { lines_done, total_lines }) => {
                    if self.filter.tail_base.is_none() {
                        self.filter.progress = Some(if total_lines == 0 { 1. } else { lines_done as f32 / total_lines as f32 });
                    }
                }
```

```rust
        if let Some((term_hits, duration)) = finished {
            // The worker has finished, so dropping the job joins at once.
            self.filter.job = None;
            let text = self.filter.job_text.take();
            let partial = std::mem::take(&mut self.filter.partial);
            let (matches, hits, duration) = match self.filter.tail_base.take() {
                Some(base) => {
                    let (m, h) = base.merge(partial, &term_hits);
                    (m, h, self.filter.duration)
                }
                None => (partial, term_hits, duration),
            };
            self.finish_scan(matches, hits, duration, text, cx);
            return false;
        }
        if died {
            if let Some(job) = self.filter.job.take() {
                retire(job);
            }
            self.filter.job_text = None;
            self.filter.tail_base = None;
            self.filter.progress = None;
            self.filter.error = Some("Filtering stopped unexpectedly; showing partial results".into());
            self.filter.matches = Arc::new(std::mem::take(&mut self.filter.partial));
            self.rebuild_view(cx);
            return false;
        }
        if self.filter.tail_base.is_none()
            && publish_due(self.filter.dirty, self.filter.last_publish.map(|t| t.elapsed()), self.filter.last_rebuild)
        {
```

(the publishing body is unchanged).

7. `finish_scan` records what it covers and catches up with growth that arrived meanwhile:

```rust
    fn finish_scan(&mut self, matches: Vec<u64>, term_hits: Vec<u64>, duration: Duration, scanned: Option<Arc<LogText>>, cx: &mut Context<Self>) {
        self.filter.matches = Arc::new(matches);
        self.filter.term_hits = term_hits;
        self.filter.duration = duration;
        self.filter.progress = None;
        self.filter.last_publish = None;
        self.filter.dirty = false;
        self.filter.scanned = scanned;
        self.rebuild_view(cx);
        // Lines appended while the scan ran are scanned now (each appended line exactly once).
        if self.grew_since_scan() {
            self.scan_tail(cx);
        }
    }
```

- [ ] **Step 4: Tail searches**

In `crates/rogg-app/src/document/search.rs`:

1. Add to `PaneSearch` (private, `Default`):

```rust
    /// The text `result` is complete for.
    covered: Option<Arc<LogText>>,
    /// The text the running count reads.
    job_text: Option<Arc<LogText>>,
    /// Lines found before a running tail count's first line.
    tail_base: Option<Vec<u64>>,
```

2. In `run_search`: reset `s.tail_base = None; s.covered = None;`, set `s.covered = Some(text.clone())` when the synchronous count succeeds, and `s.job_text = Some(text.clone())` when spawning.

3. Add:

```rust
    /// After growth: the full log's count runs on the appended lines only and is merged (the filtered pane re-counts
    /// when its view changes). A running count continues on its snapshot; its end catches up.
    pub(super) fn grow_search(&mut self, cx: &mut Context<Self>) {
        let Some(text) = self.text().cloned() else { return };
        let sync_bytes = EngineConfig::get(cx).sync_bytes;
        let s = &mut self.search.panes[Pane::Full.index()];
        if s.job.is_some() {
            return;
        }
        let (Some(search), Some(prev), Some(result)) = (s.compiled.clone(), s.covered.clone(), s.result.as_ref()) else { return };
        if text.mapped_len() <= prev.mapped_len() {
            return;
        }
        let from = prev.stable_lines();
        let mut base = result.lines.clone();
        base.truncate(base.partition_point(|&l| l < from));
        if text.mapped_len() - prev.mapped_len() <= sync_bytes {
            if let Ok(tail) = search.run(&text, &SearchScope::From(from), DEFAULT_CHUNK_LINES, &CancelToken::new()) {
                base.extend(tail.lines);
                s.result = Some(SearchResult { lines: base });
                s.covered = Some(text);
            }
            cx.notify();
        } else {
            s.tail_base = Some(base);
            s.job_text = Some(text.clone());
            s.job = Some(SearchJob::spawn(text, search, SearchScope::From(from)));
            self.ensure_polling(cx);
        }
    }

    pub(super) fn search_running(&self) -> bool {
        self.search.panes.iter().any(|s| s.job.is_some())
    }
```

4. In `poll_search`, a finished count merges a tail base and records what it covers; afterwards the full pane catches up with growth:

```rust
            match job.rx.try_recv() {
                Ok(result) => {
                    s.result = result.ok().map(|mut r| {
                        if let Some(mut base) = s.tail_base.take() {
                            base.extend(r.lines);
                            r.lines = base;
                        }
                        r
                    });
                    s.covered = s.job_text.take();
                    s.job = None;
                    changed = true;
                }
```

and after the loop, before `if changed { cx.notify(); }`:

```rust
        if changed {
            self.grow_search(cx);
        }
```

(`grow_search` returns at once unless the full log grew past what its count covers.)

- [ ] **Step 5: Live state, growth and reload in the document**

In `crates/rogg-app/src/document/mod.rs`:

1. Add a field `live: bool` (initialised `false`), and the accessor:

```rust
    /// Whether the text grows as data arrives: a stream, or a file seen growing (live follow, spec §4.5). A `.gz`
    /// still decompressing is not live.
    pub fn is_live(&self) -> bool {
        self.live
    }
```

2. In `start_load`, a stream is live from the start: in the `Source::Stream` branch set `self.live = true;` before `finish_load`; every other branch starts with `self.live = false;` (put `self.live = matches!(self.source, Source::Stream { .. });` right after `stop_jobs()`).

3. Replace `on_grew`:

```rust
    /// A grown snapshot of the text: only the appended lines are scanned and counted (spec §4.5).
    fn on_grew(&mut self, text: Arc<LogText>, cx: &mut Context<Self>) {
        let added = text.line_count().saturating_sub(self.text().map_or(0, |t| t.line_count()));
        if self.inflating.is_none() && self.inflated_len.is_none() {
            self.live = true;
        }
        self.load = LoadState::Ready(text);
        self.text_revision += 1;
        // A running scan continues on its snapshot; when it finishes, it scans what was appended meanwhile.
        if self.filter.job.is_none() {
            self.scan_tail(cx);
        }
        self.grow_search(cx);
        cx.emit(DocumentEvent::Grew { lines: added });
        cx.notify();
    }

    /// Spec §4.5: a truncated or replaced file is re-read from scratch. Pins referred to the old content.
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.filter.pins.clear();
        self.filter.pins_revision += 1;
        cx.emit(DocumentEvent::Reloaded);
        self.start_load(cx);
    }
```

4. In `poll_follow`, a truncation or replacement reloads (after the follower's borrow ends):

```rust
        let mut grew = None;
        let mut reload = false;
        let mut ended = false;
        loop {
            match follower.events().try_recv() {
                Ok(FollowEvent::Grew(text)) => grew = Some(text),
                Ok(FollowEvent::Truncated | FollowEvent::Replaced) => {
                    reload = true;
                    break;
                }
                Ok(FollowEvent::Failed(_)) | Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if reload {
            self.reload(cx);
            return true;
        }
        if let Some(text) = grew {
            self.on_grew(text, cx);
        }
```

(the rest of `poll_follow` is unchanged).

5. `poll_jobs` keeps polling while any job exists, so a job started inside a poll (a reload's open, a tail scan) is polled too:

```rust
    fn poll_jobs(&mut self, cx: &mut Context<Self>) -> bool {
        self.poll_open(cx);
        self.poll_inflate(cx);
        self.poll_follow(cx);
        self.poll_filter(cx);
        self.poll_search(cx);
        let active = self.has_jobs();
        if !active {
            self.polling = false;
        }
        active
    }

    fn has_jobs(&self) -> bool {
        self.open_job.is_some() || self.inflating.is_some() || self.follower.is_some() || self.filter.job.is_some() || self.search_running()
    }
```

(the `poll_*` functions keep returning whether their job is still running; `poll_jobs` now decides from `has_jobs` instead).

- [ ] **Step 6: Rows per revision in the tab**

In `crates/rogg-app/src/tab.rs`, replace `Applied` and `sync_rows`:

```rust
/// Document revisions already turned into rows.
#[derive(Clone, Copy, Default, PartialEq)]
struct Applied {
    load: u64,
    text: u64,
    pins: u64,
    filter: u64,
    search: [u64; 2],
}
```

```rust
    /// Rebuilds the row sources whose document data changed: the full log on text, pins or its search; the filtered log
    /// on text, the filtered view or its search. When only the text grew, the full log's rows are extended (keeping
    /// measured wrapped heights).
    pub fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let (now, full_rows, filtered_rows) = {
            let doc = self.doc.read(cx);
            let now = Applied {
                load: doc.load_revision(),
                text: doc.text_revision(),
                pins: doc.filter.pins_revision,
                filter: doc.filter.revision,
                search: [doc.search(Pane::Full).revision, doc.search(Pane::Filtered).revision],
            };
            let a = self.applied;
            let full_changed = now.text != a.text || now.pins != a.pins || now.search[0] != a.search[0];
            let filtered_changed = now.text != a.text || now.filter != a.filter || now.search[1] != a.search[1];
            (now, full_changed.then(|| doc.full_rows()).flatten(), filtered_changed.then(|| doc.filtered_rows()).flatten())
        };
        let before = self.applied;
        let loaded = now.load != before.load;
        let only_grew = !loaded && now.pins == before.pins && now.search[0] == before.search[0];
        self.applied = now;
        if let Some(rows) = full_rows {
            self.full.update(cx, |s, cx| if only_grew { s.extend_rows(rows, cx) } else { s.set_rows(rows, cx) });
        }
        if let Some(rows) = filtered_rows {
            self.filtered.update(cx, |s, cx| s.set_rows(rows, cx));
        }
        if loaded && let Some(number) = self.pending_selection.take() {
            let in_filtered = self.filtered.read(cx).rows().row_for_number(number).is_some();
            self.full.update(cx, |s, cx| s.set_selected(Some(number), cx));
            self.filtered.update(cx, |s, cx| s.set_selected(in_filtered.then_some(number), cx));
            self.selection = Some(number);
            self.selection_announced = false;
        }
        cx.notify();
    }
```

- [ ] **Step 7: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: PASS — the 5 live tests and everything before (M2 filter tests: progressive publishes still rebuild only the filtered pane).

- [ ] **Step 8: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): follow growing logs incrementally and reload truncated or replaced files

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: App — filtered views built off the UI thread, and the search carry-overs

**Files:**
- Modify: `crates/rogg-app/src/document/mod.rs` (`EngineConfig::view_sync_work`, polling), `crates/rogg-app/src/document/filter.rs`, `crates/rogg-app/src/document/search.rs`
- Modify (struct literals): `crates/rogg-app/tests/document.rs`, `crates/rogg-app/tests/search.rs`, `crates/rogg-app/tests/filter.rs`, `crates/rogg-app/tests/live.rs`
- Test: `crates/rogg-app/tests/filter.rs` (3 new tests), unit test in `search.rs`

**Interfaces:**
- Consumes: `FilteredView::build`, `FilterStats::from_parts` (M1); the document's poll loop (Tasks 10–11).
- Produces:
  - `EngineConfig { sync_bytes: u64, view_sync_work: u64 }` — `view_sync_work` (default 200,000) bounds the work (`(matches + pins) × context`) of a view rebuilt on the UI thread; larger rebuilds (pin and context changes, growth, publishes) run on a `rogg-view` thread (ruling R10, M2 carry-over).
  - While a background rebuild runs, the current view, stats and rows stay; a result that is no longer the newest request is dropped. `FilterState::view_builds_off_thread: u64` counts them (tests).
  - The filtered pane's search re-counts only on final views, not on every progressive publish (M2 carry-over).
  - `PaneSearch::failed: bool` → label `failed` when a search worker dies (M2 carry-over); `PaneSearch::runs: u64` counts started counts (tests).

- [ ] **Step 1: Write the failing tests**

Append to `crates/rogg-app/tests/filter.rs` (it already has `doc_from`):

```rust
fn every_tenth(n: usize) -> String {
    (0..n).map(|i| if i % 10 == 5 { "match here\n" } else { "filler\n" }).collect()
}

#[gpui_kit::test]
fn large_view_rebuilds_run_off_the_ui_thread_and_keep_the_old_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 0, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().len()), 100, "a context-free view is built at once");
    doc.update(cx, |d, cx| d.set_context(2, cx));
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.view.context_lines(), d.filter.view_builds_off_thread)), (0, 1), "the old view stays meanwhile");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 400));
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.scans, d.filter.stats.as_ref().unwrap().context_lines)), (1, 400), "no rescan; stats follow the view");
}

#[gpui_kit::test]
fn a_newer_view_wins_over_an_older_one(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 0, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    doc.update(cx, |d, cx| {
        d.set_context(1, cx);
        d.set_context(3, cx);
    });
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() > 0));
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 600));
    for _ in 0..20 {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.context_lines()), 600, "the ±1 result never replaces the ±3 one");
}

#[gpui_kit::test]
fn the_filtered_search_waits_for_the_final_view_of_a_scan(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let text: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_search(Pane::Filtered, "alpha", cx));
    let before = doc.read_with(cx, |d, _| d.search(Pane::Filtered).runs);
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && !d.search(Pane::Filtered).is_running()));
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Filtered).runs), before + 1, "one count, on the final view");
    let hits = doc.read_with(cx, |d, _| d.search(Pane::Filtered).result.as_ref().map(|r| r.count()));
    assert_eq!(hits, Some(100_000));
}
```

(import `rogg_app::document::Pane` there if the file does not already.)

In `crates/rogg-app/src/document/search.rs`, add a unit test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dead_worker_is_labelled_failed() {
        let mut s = PaneSearch { query: "timeout".into(), ..Default::default() };
        assert_eq!(s.label().as_deref(), Some("…"));
        s.failed = true;
        assert_eq!(s.label().as_deref(), Some("failed"));
    }
}
```

Replace every `EngineConfig { sync_bytes: 0 }` in `crates/rogg-app/tests/{document,search,filter,live}.rs` with `EngineConfig { sync_bytes: 0, ..Default::default() }`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test filter && cargo test -p rogg-app --lib search`
Expected: FAIL to compile — `view_sync_work`, `view_builds_off_thread`, `runs`, `failed` not found.

- [ ] **Step 3: The setting**

In `crates/rogg-app/src/document/mod.rs`:

```rust
/// Inputs at or below `sync_bytes` are opened, filtered and searched synchronously; filtered views whose work
/// (`(matches + pins) × context`) is at most `view_sync_work` are composed synchronously (ruling R10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    pub sync_bytes: u64,
    pub view_sync_work: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { sync_bytes: 4 << 20, view_sync_work: 200_000 }
    }
}
```

`poll_jobs` also calls `self.poll_view(cx);`, and `has_jobs` adds `|| self.filter.view_job.is_some()`.

- [ ] **Step 4: Background view rebuilds**

In `crates/rogg-app/src/document/filter.rs` (add `use std::sync::mpsc::{self, Receiver};` next to the existing `TryRecvError` import):

```rust
/// A filtered view being composed on a background thread (R10): the request number it answers, and the channel.
pub(super) struct ViewJob {
    request: u64,
    rx: Receiver<(FilteredView, FilterStats)>,
}
```

Fields on `FilterState` (initialised `None` / `0`):

```rust
    pub(super) view_job: Option<ViewJob>,
    /// Rebuilds requested so far; only the newest one's result is installed.
    view_requests: u64,
    /// Rebuilds that ran off the UI thread (tests).
    pub view_builds_off_thread: u64,
```

Replace `rebuild_view` and add `install_view` / `poll_view`:

```rust
    /// Recomposes the filtered view and statistics from the current matches, pins and context. While a scan runs the
    /// view is built without context (and without the exclusion check), so a progressive publish stays cheap. Views
    /// whose work exceeds `EngineConfig::view_sync_work` are built on a background thread; the current view stays until
    /// the new one lands, and a result that is no longer the newest request is dropped.
    pub(super) fn rebuild_view(&mut self, cx: &mut Context<Self>) {
        let total = self.text().map_or(0, |t| t.line_count());
        let f = &self.filter;
        let context = if f.progress.is_some() { 0 } else { f.spec.context };
        let work = (f.matches.len() as u64 + f.pins.len() as u64).saturating_mul(context as u64);
        let excluding = match (self.text(), &f.compiled) {
            (Some(text), Some(filter)) if context > 0 => Some((text.clone(), filter.clone())),
            _ => None,
        };
        let (matches, pins, spec, hits, duration) = (f.matches.clone(), f.pins.clone(), f.spec.clone(), f.term_hits.clone(), f.duration);
        let build = move || {
            let view = match &excluding {
                Some((text, filter)) => FilteredView::build(matches.clone(), &pins, context, total, |n| filter.is_excluded(text.line(n))),
                None => FilteredView::build(matches.clone(), &pins, context, total, |_| false),
            };
            let stats = FilterStats::from_parts(&spec, matches.len() as u64, &hits, total, duration, &view);
            (view, stats)
        };
        self.filter.view_requests += 1;
        if work <= EngineConfig::get(cx).view_sync_work {
            // A newer, cheap request supersedes a pending background one.
            self.filter.view_job = None;
            let (view, stats) = build();
            self.install_view(view, stats, cx);
        } else {
            let (tx, rx) = mpsc::channel();
            let _detached = std::thread::Builder::new().name("rogg-view".into()).spawn(move || {
                let _ = tx.send(build());
            });
            self.filter.view_job = Some(ViewJob { request: self.filter.view_requests, rx });
            self.filter.view_builds_off_thread += 1;
            self.ensure_polling(cx);
        }
    }

    fn install_view(&mut self, view: FilteredView, stats: FilterStats, cx: &mut Context<Self>) {
        self.filter.view = Arc::new(view);
        self.filter.stats = Some(stats);
        self.filter.revision += 1;
        // While a scan publishes partial results, the filtered pane's count waits for the final view.
        if self.filter.progress.is_none() {
            self.refresh_search(super::Pane::Filtered, cx);
        }
        cx.notify();
    }

    /// Installs a finished background view if it answers the newest request; returns whether one is still pending.
    pub(super) fn poll_view(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(job) = &self.filter.view_job else { return false };
        match job.rx.try_recv() {
            Ok((view, stats)) => {
                let newest = job.request == self.filter.view_requests;
                self.filter.view_job = None;
                if newest {
                    self.install_view(view, stats, cx);
                }
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => {
                self.filter.view_job = None;
                false
            }
        }
    }
```

The progressive-publish path in `poll_filter` measures `last_rebuild` around `rebuild_view` as before; a publish is context-free, so it always builds synchronously.

- [ ] **Step 5: Search failure and run count**

In `crates/rogg-app/src/document/search.rs`:
- `PaneSearch` gains `pub failed: bool` and `pub runs: u64`;
- `label()` returns `Some("failed".into())` when `self.failed` (checked after `invalid`);
- `run_search` sets `s.failed = false;` and, when a compiled query is counted (synchronously or by spawning a job), `s.runs += 1;`;
- in `poll_search`, `Err(TryRecvError::Disconnected)` sets `s.failed = true; s.job_text = None; s.tail_base = None;` besides `s.job = None;`.

- [ ] **Step 6: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: PASS — the 3 new filter tests, the unit test, and all earlier tests (small views keep building synchronously, so M2's tests and the demo are unchanged).

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): build large filtered views off the UI thread; count filtered hits on final views

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: App — live follow in the tab: the control, Space, stick-to-bottom and the reload toast

**Files:**
- Modify: `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/tab_view.rs`, `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/workspace/keys.rs`
- Test: `crates/rogg-app/tests/live_tab.rs`

**Interfaces:**
- Consumes: `DocumentEvent`, `Document::is_live` (Tasks 10–11); `LogViewState::{set_tail, at_bottom}`, `LogViewEvent::TailLeft` (Task 9); `ToolButton::badge` (M0).
- Produces:
  - `Tab::live: LiveFollow { following: bool, new_lines: u64 }`, `Tab::toggle_live(&mut self, cx)`; `tab::RELOADED_TOAST = "File was truncated or replaced — reloaded."`.
  - Behaviour (spec §4.5): a stream follows from the start; a file that starts growing follows when its end was in view, else it waits paused and counts. Following keeps both panes at their end (log-view tail mode). Pausing — Space, the control, or scrolling the full log away from its end — stops that; `new_lines` counts lines that arrived since. Resuming jumps to the end and clears the count.
  - The full-log header shows, for live documents, `ToolButton("tool-live", Pause, "Pause")` while following and `ToolButton("tool-live", Pause, "Paused").pressed(true).badge("+1,204")` while paused (the approved M0 gallery state), before `Wrap`.
  - Action `ToggleLive`, bound to `space` in the log view context and listed as `Pause / resume live follow` · `Space`; ignored behind dialogs.

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-app/tests/live_tab.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, ScrollDelta, TestAppContext, point, px};
use rogg_app::args::Args;
use rogg_app::document::Source;
use rogg_app::tab::{RELOADED_TOAST, Tab, TabInit};
use rogg_app::workspace::Workspace;
use std::io::Write;
use std::path::{Path, PathBuf};

fn append(path: &Path, s: &str) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(s.as_bytes()).unwrap();
}

fn lines(tab: &Entity<Tab>, cx: &mut TestAppContext) -> u64 {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().map_or(0, |x| x.line_count()))
}

fn open_stream(cx: &mut TestAppContext, dir: &Path, first: usize) -> (AnyWindowHandle, Entity<Workspace>, Entity<Tab>, PathBuf) {
    let path = dir.join("stream.log");
    std::fs::write(&path, (0..first).map(|i| format!("early {i}\n")).collect::<String>()).unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    let source = Source::Stream { name: "stdin".into(), path: path.clone() };
    let tab = cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_source(source, TabInit::default(), window, cx))).unwrap();
    cx.run_until_parked();
    (w, ws, tab, path)
}

#[gpui_kit::test]
fn streams_follow_new_lines_until_paused(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws, tab, path) = open_stream(cx, dir.path(), 1);
    assert!(common::exists(cx, w, "tool-live"));
    assert!(tab.read_with(cx, |t, _| t.live.following));
    append(&path, &(0..200).map(|i| format!("line {i}\n")).collect::<String>());
    common::wait_until(cx, |cx| lines(&tab, cx) == 201);
    common::render(cx, w);
    let full = tab.read_with(cx, |t, _| t.full.clone());
    assert_eq!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row), 200, "the newest line is in view");
    common::press(cx, w, "tab");
    common::press(cx, w, "space");
    assert!(!tab.read_with(cx, |t, _| t.live.following));
    append(&path, "a\nb\nc\nd\ne\n");
    common::wait_until(cx, |cx| lines(&tab, cx) == 206);
    assert_eq!(tab.read_with(cx, |t, _| t.live.new_lines), 5);
    common::render(cx, w);
    assert!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row) < 205, "paused: the view stays");
    common::click(cx, w, "tool-live");
    assert_eq!(tab.read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (true, 0));
    common::render(cx, w);
    assert_eq!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row), 205, "resuming jumps to the end");
}

#[gpui_kit::test]
fn scrolling_the_full_log_up_pauses_following(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws, tab, _path) = open_stream(cx, dir.path(), 300);
    common::render(cx, w);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-top", ScrollDelta::Pixels(point(px(0.), px(200.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!tab.read_with(cx, |t, _| t.live.following));
}

#[gpui_kit::test]
fn files_follow_when_their_end_was_in_view(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let short = common::write_log(dir.path(), "short.log", &["one", "two", ""]);
    let long_lines: Vec<String> = (0..1000).map(|i| format!("l{i}")).collect();
    let mut long_refs: Vec<&str> = long_lines.iter().map(String::as_str).collect();
    long_refs.push("");
    let long = common::write_log(dir.path(), "long.log", &long_refs);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![short.clone(), long.clone()] });
    common::render(cx, w);
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert!(!common::exists(cx, w, "tool-live"), "a file is not live until it grows");
    append(&short, "three\n");
    append(&long, "more\nand more\n");
    common::wait_until(cx, |cx| lines(&tabs[0], cx) == 3 && lines(&tabs[1], cx) == 1002);
    assert!(tabs[0].read_with(cx, |t, _| t.live.following), "everything fitted: the end was in view");
    assert_eq!(tabs[1].read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (false, 2), "scrolled to the top: paused, counting");
}

#[gpui_kit::test]
fn a_truncated_file_reloads_with_a_toast(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "t.log", &["a", "b", ""]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.as_ref() == RELOADED_TOAST)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test live_tab`
Expected: FAIL to compile — `RELOADED_TOAST`, `Tab::live` not found.

- [ ] **Step 3: Live follow in the tab**

In `crates/rogg-app/src/tab.rs` (import `crate::document::DocumentEvent`):

```rust
/// Spec §4.5 reload toast.
pub const RELOADED_TOAST: &str = "File was truncated or replaced — reloaded.";

/// Live follow (spec §4.5): following keeps both panes at their end as lines arrive; paused counts what arrived.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveFollow {
    pub following: bool,
    /// Lines that arrived since following was paused (`+1,204` on the control).
    pub new_lines: u64,
}
```

Add `pub live: LiveFollow` and `live_started: bool` to `Tab` (both default), subscribe to the document in `new`:

```rust
            cx.subscribe(&doc, |tab, _, ev: &DocumentEvent, cx| tab.on_doc_event(ev, cx)),
```

and at the end of `new`, before `tab.sync_rows(cx)`, start following a stream:

```rust
        if matches!(tab.doc.read(cx).source, Source::Stream { .. }) {
            tab.live_started = true;
            tab.set_following(true, cx);
        }
```

Methods:

```rust
    fn on_doc_event(&mut self, ev: &DocumentEvent, cx: &mut Context<Self>) {
        match ev {
            DocumentEvent::Reloaded => cx.emit(TabEvent::Toast(RELOADED_TOAST.into())),
            DocumentEvent::Grew { lines } => {
                if !self.doc.read(cx).is_live() {
                    return;
                }
                if !self.live_started {
                    // A file that starts growing follows when its end was in view (`tail -f`); else it waits, counting.
                    self.live_started = true;
                    let following = self.full.read(cx).at_bottom();
                    self.set_following(following, cx);
                    if !following {
                        self.live.new_lines = *lines;
                    }
                } else if !self.live.following {
                    self.live.new_lines += lines;
                }
                cx.notify();
            }
        }
    }

    fn set_following(&mut self, on: bool, cx: &mut Context<Self>) {
        self.live.following = on;
        if on {
            self.live.new_lines = 0;
        }
        self.full.update(cx, |s, cx| s.set_tail(on, cx));
        self.filtered.update(cx, |s, cx| s.set_tail(on, cx));
        cx.notify();
    }

    /// Space / the live control: pause or resume following (live documents only).
    pub fn toggle_live(&mut self, cx: &mut Context<Self>) {
        if self.doc.read(cx).is_live() {
            let on = !self.live.following;
            self.set_following(on, cx);
        }
    }
```

In `on_log_event`, the `TailLeft` arm pauses when the full log leaves its end:

```rust
            LogViewEvent::TailLeft => {
                if pane == Pane::Full && self.live.following {
                    self.live.following = false;
                    self.filtered.update(cx, |s, cx| s.set_tail(false, cx));
                    cx.notify();
                }
            }
```

- [ ] **Step 4: The control and the key**

In `crates/rogg-app/src/tab_view.rs`, build the full-log header with the live control before Wrap:

```rust
            let mut top_header = PaneHeader::new("pane-top-head", "Full log", full_count).find(FindField::new("find-top", t.find[0].clone()).count(d.search(Pane::Full).label()));
            if d.is_live() {
                let paused = !t.live.following;
                top_header = top_header.tool(
                    ToolButton::new("tool-live", IconName::Pause, if paused { "Paused" } else { "Pause" })
                        .pressed(paused)
                        .badge((paused && t.live.new_lines > 0).then(|| SharedString::from(format!("+{}", group_digits(t.live.new_lines)))))
                        .on_click(tab_act(tab, |t, _, cx| t.toggle_live(cx))),
                );
            }
            let top_header = top_header
                .tool(ToolButton::new("tool-wrap-top", IconName::Wrap, "Wrap").pressed(t.full.read(cx).wrap()).on_click(tab_act(tab, |t, _, cx| t.toggle_wrap(Pane::Full, cx))))
                .into_any_element();
```

In `crates/rogg-app/src/actions.rs`, add `ToggleLive` to the `actions!` list and, after the `Reveal in full log` entry:

```rust
        shortcut!("Pause / resume live follow", "Space", "space", ToggleLive, LOG),
```

In `crates/rogg-app/src/workspace/keys.rs` `bind`:

```rust
        .on_action(unless_dialog(cx, |ws, _: &ToggleLive, _, cx| ws.with_tab(cx, |t, cx| t.toggle_live(cx))))
```

(The keyboard-shortcuts dialog gains this row; the app's `--capture app --dialog shortcuts` capture is one row taller than in the M2 report — noted in Task 17.)

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: PASS (4 new tests; the keymap uniqueness test still passes — `space` is new in the log-view context).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app
git commit -m "feat(app): live follow with pause, Space, stick-to-bottom and the reload toast

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: App — folders and archives in the sidebar

**Files:**
- Create: `crates/rogg-app/src/sources.rs`
- Modify: `crates/rogg-core/src/recent.rs` (archive members), `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/sidebar_model.rs`, `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-app/src/workspace/chrome.rs`
- Test: `crates/rogg-app/tests/tree.rs`; `crates/rogg-app/tests/workspace.rs` (`folders_open_their_text_files` is replaced); unit tests in `recent.rs`, `sidebar_model.rs`

**Interfaces:**
- Consumes: `SourceTree`, `NodeKind`, `Listing`, `scan_folder`, `list_archive`, `sniff` (Tasks 4–6); `Source::source_ref`, `CacheStore` (Task 10); `TreeRow::Folder { note, failed }` (Task 9).
- Produces:
  - `RecentEntry { path, is_folder, members: Vec<String> }` — `members` (serde default, omitted when empty) is the member chain of an archive's last opened member; `RecentEntry::member(archive, members)`, `RecentEntry::source() -> Option<SourceRef>`; `label()` of a member entry is `incident.zip › api.log`.
  - `rogg_app::sources::{SourcesState { tree, expanded }, TreeJob, TreeResult}`; `Workspace::sources` (pub(crate)).
  - Opening: a folder becomes a tree root scanned on a background thread (`Listing` shows `listing…`); an archive (sniffed, by content) becomes a root listed on a background thread; nested archives list when first expanded; a folder or archive opened by the user opens its first log (a top-level file first) once listed. Clicking a file or member row opens it (or activates its tab) and records it in Recent (archive members as member entries).
  - `RowAction::{ToggleNode(NodeId), OpenNode(NodeId)}`; `OpenFile::source: Option<SourceRef>`; `sidebar_model::{tree_rows, tree_footer}`; footer `N members · X expanded` / `Members open individually` for an archive, `N files · X` for a folder.
  - `Workspace::{open_folder, open_archive, open_node, toggle_node}`; `chrome::{folder_files, FOLDER_FILE_LIMIT}` are removed (the bounded recursive scan replaces them).

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-core/src/recent.rs` tests, add:

```rust
    #[test]
    fn archive_entries_remember_their_member() {
        let entry = RecentEntry::member("/b/incident.zip", vec!["nested/inner.tgz".into(), "app/x.log".into()]);
        assert_eq!(entry.label(), "incident.zip › x.log");
        assert_eq!(
            entry.source(),
            Some(SourceRef::member(SourceRef::member(SourceRef::File("/b/incident.zip".into()), "nested/inner.tgz"), "app/x.log"))
        );
        assert_eq!(RecentEntry::file("/a.log").source(), None);
        let json = serde_json::to_string(&RecentEntry::file("/a.log")).unwrap();
        assert!(!json.contains("members"), "plain entries keep the M2 format: {json}");
        let old: RecentEntry = serde_json::from_str(r#"{"path":"/x.zip","is_folder":false}"#).unwrap();
        assert!(old.members.is_empty());
        let mut list = RecentList::default();
        list.record(RecentEntry::file("/b/incident.zip"));
        list.record(RecentEntry::member("/b/incident.zip", vec!["api.log".into()]));
        assert_eq!(list.entries().len(), 1, "one entry per archive: the last opened member");
    }
```

Create `crates/rogg-app/tests/tree.rs`:

```rust
mod common;
use gpui_kit::{AnyWindowHandle, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::LoadState;
use rogg_app::recent::RecentStore;
use rogg_app::workspace::Workspace;
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, write_archive, write_stored_zip};

fn titles(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<String> {
    ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect())
}

const ROW_IDS: [&str; 12] = [
    "tree-row-0", "tree-row-1", "tree-row-2", "tree-row-3", "tree-row-4", "tree-row-5",
    "tree-row-6", "tree-row-7", "tree-row-8", "tree-row-9", "tree-row-10", "tree-row-11",
];

fn rows(cx: &mut TestAppContext, w: AnyWindowHandle) -> usize {
    ROW_IDS.iter().take_while(|&&id| common::exists(cx, w, id)).count()
}

fn wait_rows(cx: &mut TestAppContext, w: AnyWindowHandle, n: usize) {
    common::wait_until(cx, |cx| rows(cx, w) == n);
}

#[gpui_kit::test]
fn folders_list_their_logs_and_open_the_first(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let logs = dir.path().join("logs");
    std::fs::create_dir_all(logs.join("sub")).unwrap();
    common::write_log(&logs, "one.log", &["1"]);
    common::write_log(&logs, "two.log", &["2"]);
    common::write_log(&logs, ".hidden.log", &["h"]);
    common::write_log(&logs.join("sub"), "three.log", &["3"]);
    std::fs::write(logs.join("blob.bin"), [0u8, 159, 146, 150]).unwrap();
    std::fs::write(logs.join("utf16.log"), [0xFF, 0xFE, b'h', 0, b'i', 0]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![logs.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["one.log"], "the first top-level log opens");
    // logs (expanded), sub (collapsed), one.log, two.log, utf16.log
    wait_rows(cx, w, 5);
    common::click(cx, w, "tree-row-1");
    wait_rows(cx, w, 6);
    common::click(cx, w, "tree-row-2");
    assert_eq!(titles(&ws, cx), ["one.log", "three.log"]);
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 1);
    cx.update(|cx| {
        let entries = cx.global::<RecentStore>().entries();
        assert!(entries[0].path.ends_with("three.log"), "a clicked file is recorded");
        assert!(entries.iter().any(|e| e.is_folder && e.path.ends_with("logs")));
        assert!(!entries.iter().any(|e| e.path.ends_with("one.log")), "the automatically opened file is not");
    });
}

#[gpui_kit::test]
fn archives_list_lazily_and_open_nested_members(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let inner = dir.path().join("inner.tgz");
    write_archive(&inner, BundleKind::TarGz, &[("app/x.log", b"nested\n")]).unwrap();
    let inner_bytes = std::fs::read(&inner).unwrap();
    let zip = dir.path().join("outer.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"api\n"), ("inner.tgz", &inner_bytes), ("README", b"read me\n")]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["README"]);
    // outer.zip, logs (collapsed), inner.tgz (not listed yet), README
    wait_rows(cx, w, 4);
    common::click(cx, w, "tree-row-2");
    // … inner.tgz lists in the background: + app
    wait_rows(cx, w, 5);
    common::click(cx, w, "tree-row-3");
    wait_rows(cx, w, 6);
    common::click(cx, w, "tree-row-4");
    assert_eq!(titles(&ws, cx), ["README", "x.log"]);
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    let crumbs: Vec<String> = tab.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs().iter().map(|c| c.to_string()).collect());
    assert_eq!(crumbs, ["outer.zip", "inner.tgz", "app", "x.log"]);
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).text().is_some()));
    let footer = ws.read_with(cx, |ws, cx| ws.sidebar_footer(cx)).unwrap();
    assert_eq!(footer.title.as_ref(), "outer.zip");
    assert_eq!(footer.lines.iter().map(|l| l.to_string()).collect::<Vec<_>>(), ["3 members · 19 B expanded", "Members open individually"]);
    cx.update(|cx| {
        let entry = &cx.global::<RecentStore>().entries()[0];
        assert_eq!(entry.members, ["inner.tgz", "app/x.log"]);
        assert!(entry.path.ends_with("outer.zip"));
    });
}

#[gpui_kit::test]
fn a_damaged_member_fails_only_its_own_tab(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    // d.zip, api.log (opened first, by name), README
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    wait_rows(cx, w, 3);
    common::click(cx, w, "tree-row-2");
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert_eq!(tabs.len(), 2);
    let state = |i: usize, cx: &mut TestAppContext| tabs[i].read_with(cx, |t, cx| t.doc.read(cx).load.clone());
    common::wait_until(cx, |cx| matches!(state(0, cx), LoadState::Failed { .. }));
    common::wait_until(cx, |cx| matches!(state(1, cx), LoadState::Ready(_)));
    let LoadState::Failed { message } = state(0, cx) else { unreachable!() };
    assert!(message.contains("api.log"), "{message}");
}

#[gpui_kit::test]
fn unreadable_archives_are_marked_and_toasted(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let junk = dir.path().join("junk.zip");
    std::fs::write(&junk, b"PK\x03\x04 not really a zip").unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![junk] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.starts_with("Could not read junk.zip"))));
    assert!(common::exists(cx, w, "tree-note-0"), "the archive row says it is unreadable");
    assert!(ws.read_with(cx, |ws, _| ws.tabs().is_empty()));
}

#[gpui_kit::test]
fn huge_folders_list_the_first_files_with_a_toast(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let big = dir.path().join("big");
    std::fs::create_dir(&big).unwrap();
    for i in 0..=rogg_core::folder::MAX_SCAN_FILES {
        std::fs::write(big.join(format!("f{i:05}.log")), "").unwrap();
    }
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![big] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.contains("showing the first 10,000 files"))));
}

#[gpui_kit::test]
fn recent_archive_members_reopen_their_member(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("r.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n"), ("b.log", b"b\n")]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    wait_rows(cx, w, 3);
    common::click(cx, w, "tree-row-2");
    assert_eq!(titles(&ws, cx), ["a.log", "b.log"]);
    common::press(cx, w, "cmd-w");
    common::press(cx, w, "cmd-w");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_recent(0, window, cx))).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["b.log"], "the member opens, not the archive's first file");
    let source = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.read(cx).source.source_ref());
    assert_eq!(source, Some(SourceRef::member(SourceRef::File(zip.canonicalize().unwrap()), "b.log")));
}
```

In `crates/rogg-app/tests/workspace.rs`, delete `folders_open_their_text_files` (replaced by `tree.rs`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core --lib recent && cargo test -p rogg-app --test tree`
Expected: FAIL to compile — `RecentEntry::member`, `members`, `sidebar_footer` not found.

- [ ] **Step 3: Recent entries for archive members**

In `crates/rogg-core/src/recent.rs` (import `crate::source::SourceRef`):

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub is_folder: bool,
    /// For an archive: the member chain of the last member opened in it (spec §6.3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
}

impl RecentEntry {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: false, members: Vec::new() }
    }

    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: true, members: Vec::new() }
    }

    /// An archive with the member last opened in it (outermost member first, for nested archives).
    pub fn member(archive: impl Into<PathBuf>, members: Vec<String>) -> Self {
        Self { path: archive.into(), is_folder: false, members }
    }

    /// The member to reopen; None for files and folders.
    pub fn source(&self) -> Option<SourceRef> {
        if self.members.is_empty() {
            return None;
        }
        Some(self.members.iter().fold(SourceRef::File(self.path.clone()), |archive, m| SourceRef::member(archive, m.clone())))
    }

    /// The list label: the last path component, or `incident.zip › api.log` for an archive member.
    pub fn label(&self) -> String {
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string());
        match self.source() {
            Some(source) => format!("{name} › {}", source.name()),
            None => name,
        }
    }
```

(`is_missing` is unchanged.)

- [ ] **Step 4: The sources state and its jobs**

Create `crates/rogg-app/src/sources.rs` and add `pub mod sources;` to `lib.rs`:

```rust
//! The sidebar's folders and archives (spec §4.1, §5.3): the source tree, which containers are expanded, and the
//! background scans and listings that fill it.

use rogg_core::archive::ArchiveEntry;
use rogg_core::cache::Cache;
use rogg_core::folder::{DEFAULT_SCAN_DEPTH, FolderScan, scan_folder};
use rogg_core::source::{SourceRef, list_archive};
use rogg_core::tree::{NodeId, SourceTree};
use rogg_core::{CancelToken, CoreError};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub enum TreeResult {
    Scanned(Result<FolderScan, CoreError>),
    Listed(Result<Vec<ArchiveEntry>, CoreError>),
    /// The worker ended without a result.
    Died,
}

/// A folder scan or archive listing running for one tree node.
pub struct TreeJob {
    pub node: NodeId,
    cancel: CancelToken,
    rx: Receiver<TreeResult>,
}

impl TreeJob {
    pub fn scan(node: NodeId, dir: PathBuf) -> TreeJob {
        Self::spawn(node, move |token| TreeResult::Scanned(scan_folder(&dir, DEFAULT_SCAN_DEPTH, token)))
    }

    /// Lists an archive (extracting it into the cache first when it is a member of another archive).
    pub fn list(node: NodeId, source: SourceRef, cache: Arc<Cache>) -> TreeJob {
        Self::spawn(node, move |token| TreeResult::Listed(list_archive(&source, &cache, token)))
    }

    fn spawn(node: NodeId, work: impl FnOnce(&CancelToken) -> TreeResult + Send + 'static) -> TreeJob {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let _detached = std::thread::Builder::new().name("rogg-tree".into()).spawn(move || {
            let _ = tx.send(work(&token));
        });
        TreeJob { node, cancel, rx }
    }

    /// The result once the job has finished.
    pub fn poll(&self) -> Option<TreeResult> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(TreeResult::Died),
        }
    }
}

impl Drop for TreeJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub struct SourcesState {
    pub tree: SourceTree,
    /// Expanded folders and archives (roots start expanded).
    pub expanded: HashSet<NodeId>,
    pub(crate) jobs: Vec<TreeJob>,
    /// Roots whose first log opens when their scan or listing arrives (the user opened the folder or archive itself).
    pub(crate) open_first: HashSet<NodeId>,
    pub(crate) polling: bool,
}
```

- [ ] **Step 5: Tree rows and footers**

In `crates/rogg-app/src/sidebar_model.rs` (imports: `rogg_core::source::SourceRef`, `rogg_core::stats::{group_digits, noun}`, `rogg_core::tree::{Listing, NodeId, NodeKind, SourceTree}`, `rogg_ui::sidebar::SidebarFooter` is already there):

- `OpenFile` gains `pub source: Option<SourceRef>,` (update the two unit tests' literals with `source: None`);
- `RowAction` gains `ToggleNode(NodeId)` and `OpenNode(NodeId)`;
- add:

```rust
/// Rows for the tree's roots (folders and archives) and their expanded contents, with the action of each row. A file
/// or member row is active and failed like the tab showing it.
pub fn tree_rows(tree: &SourceTree, expanded: &HashSet<NodeId>, files: &[OpenFile], active: Option<EntityId>) -> (Vec<TreeRow>, Vec<RowAction>) {
    let mut rows = Vec::new();
    let mut actions = Vec::new();
    for (id, depth) in tree.visible(|id| expanded.contains(&id)) {
        let node = tree.node(id);
        if tree.is_container(id) {
            let (note, failed) = match &node.listing {
                Listing::Listing => (Some("listing…"), false),
                Listing::Failed(_) => (Some("unreadable"), true),
                Listing::Pending | Listing::Listed => (None, false),
            };
            rows.push(TreeRow::Folder { label: node.name.clone().into(), expanded: expanded.contains(&id), depth, note: note.map(SharedString::from), failed });
            actions.push(RowAction::ToggleNode(id));
        } else {
            let tab = node.source.as_ref().and_then(|s| files.iter().find(|f| f.source.as_ref() == Some(s)));
            rows.push(TreeRow::File {
                label: node.name.clone().into(),
                size: node.size.map(format_size).unwrap_or_default().into(),
                active: tab.is_some_and(|f| Some(f.tab) == active),
                depth: depth.max(1),
                failed: tab.is_some_and(|f| f.failed),
            });
            actions.push(RowAction::OpenNode(id));
        }
    }
    (rows, actions)
}

/// The sidebar footer for a root (spec §5.3: `10 members · 8.4 GB expanded` / `Members open individually`).
pub fn tree_footer(tree: &SourceTree, root: NodeId) -> SidebarFooter {
    let node = tree.node(root);
    let sum = tree.summary(root);
    let lines = match node.kind {
        NodeKind::Archive(_) => vec![
            SharedString::from(format!("{} {} · {} expanded", group_digits(sum.files), noun(sum.files, "member", "members"), format_size(sum.bytes))),
            SharedString::from("Members open individually"),
        ],
        _ => vec![SharedString::from(format!("{} {} · {}", group_digits(sum.files), noun(sum.files, "file", "files"), format_size(sum.bytes)))],
    };
    SidebarFooter { title: node.name.clone().into(), lines }
}
```

and a unit test:

```rust
    #[test]
    fn tree_rows_follow_listing_states_and_tabs() {
        use rogg_core::archive::{ArchiveEntry, ArchiveKind};
        let mut tree = SourceTree::new();
        let zip = tree.add_root(std::path::Path::new("/b/x.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(10));
        tree.set_listing(zip, Listing::Listing);
        let expanded = HashSet::from([zip]);
        let (rows, _) = tree_rows(&tree, &expanded, &[], None);
        assert!(matches!(&rows[0], TreeRow::Folder { note: Some(n), failed: false, .. } if n.as_ref() == "listing…"));
        tree.insert_listing(zip, &[ArchiveEntry { path: "a.log".into(), size: 1200 }]);
        let tab = EntityId::from(7u64);
        let a = SourceRef::member(SourceRef::File("/b/x.zip".into()), "a.log");
        let files = [OpenFile { tab, title: "a.log".into(), path: None, bytes: None, failed: true, source: Some(a) }];
        let (rows, actions) = tree_rows(&tree, &expanded, &files, Some(tab));
        assert!(matches!(&rows[1], TreeRow::File { active: true, failed: true, depth: 1, .. }));
        assert!(matches!(actions[1], RowAction::OpenNode(_)));
        tree.set_listing(zip, Listing::Failed("damaged".into()));
        let (rows, _) = tree_rows(&tree, &expanded, &files, None);
        assert!(matches!(&rows[0], TreeRow::Folder { failed: true, .. }));
        let footer = tree_footer(&tree, zip);
        assert_eq!(footer.lines[0].as_ref(), "1 member · 1 KB expanded");
    }
```

- [ ] **Step 6: Opening folders and archives in the workspace**

In `crates/rogg-app/src/workspace/mod.rs`, add the field `pub(crate) sources: crate::sources::SourcesState,` (initialised `Default::default()`).

In `crates/rogg-app/src/workspace/chrome.rs`, delete `FOLDER_FILE_LIMIT`, `folder_files`, `looks_like_text` and their two unit tests, and replace `open_paths`, `open_folder`, `open_recent`, `open_files`, `active_footer`, `sidebar` and `sidebar_action` with the code below (keep the rest; extend the imports with `crate::cache::CacheStore`, `crate::document::POLL_INTERVAL`, `crate::sources::{TreeJob, TreeResult}`, `rogg_core::archive::{ArchiveKind, Content, sniff}`, `rogg_core::folder::MAX_SCAN_FILES`, `rogg_core::source::SourceRef`, `rogg_core::tree::{Listing, NodeId, NodeKind}`):

```rust
/// What an opened or dropped path holds.
enum PathKind {
    Folder,
    Archive(ArchiveKind, u64),
    File,
}

fn classify(path: &Path) -> PathKind {
    let Ok(meta) = std::fs::metadata(path) else { return PathKind::File };
    if meta.is_dir() {
        return PathKind::Folder;
    }
    match sniff(path) {
        Ok(Content::Archive(kind)) => PathKind::Archive(kind, meta.len()),
        _ => PathKind::File,
    }
}

impl Workspace {
    /// Opens each path: folders and archives as sidebar roots (spec §4.1), files in a tab (or by activating the tab
    /// already showing them). Opened files, folders and archives are recorded in Recent.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            match classify(&path) {
                PathKind::Folder => self.open_folder(path, window, cx),
                PathKind::Archive(kind, size) => self.open_archive(path, kind, size, true, window, cx),
                PathKind::File => {
                    if path.exists() {
                        self.record_recent(RecentEntry::file(&path), cx);
                    }
                    self.open_file(path, window, cx);
                }
            }
        }
    }

    /// A folder becomes a tree root, scanned on a background thread; its first log opens when the scan arrives.
    pub fn open_folder(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.record_recent(RecentEntry::folder(&dir), cx);
        if let Some(root) = self.sources.tree.root_for(&dir) {
            self.sources.expanded.insert(root);
            cx.notify();
            return;
        }
        let root = self.sources.tree.add_root(&dir, NodeKind::Folder, None);
        self.sources.tree.set_listing(root, Listing::Listing);
        self.sources.expanded.insert(root);
        self.sources.open_first.insert(root);
        self.sources.jobs.push(TreeJob::scan(root, dir));
        self.ensure_tree_polling(window, cx);
        cx.notify();
    }

    /// An archive becomes a tree root, listed on a background thread; with `open_first`, its first log opens when the
    /// listing arrives.
    pub fn open_archive(&mut self, path: PathBuf, kind: ArchiveKind, size: u64, open_first: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open_first {
            self.record_recent(RecentEntry::file(&path), cx);
        }
        if let Some(root) = self.sources.tree.root_for(&path) {
            self.sources.expanded.insert(root);
            cx.notify();
            return;
        }
        let root = self.sources.tree.add_root(&path, NodeKind::Archive(kind), Some(size));
        self.sources.expanded.insert(root);
        if open_first {
            self.sources.open_first.insert(root);
        }
        self.start_listing(root, window, cx);
        cx.notify();
    }

    fn start_listing(&mut self, node: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.sources.tree.node(node).source.clone() else { return };
        self.sources.tree.set_listing(node, Listing::Listing);
        self.sources.jobs.push(TreeJob::list(node, source, CacheStore::get(cx)));
        self.ensure_tree_polling(window, cx);
    }

    fn ensure_tree_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sources.polling {
            return;
        }
        self.sources.polling = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                match this.update_in(cx, |ws, window, cx| ws.poll_tree(window, cx)) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// Applies finished scans and listings; returns whether any job is still running.
    fn poll_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut finished = Vec::new();
        self.sources.jobs.retain(|job| match job.poll() {
            Some(result) => {
                finished.push((job.node, result));
                false
            }
            None => true,
        });
        for (node, result) in finished {
            let name = self.sources.tree.node(node).name.clone();
            match result {
                TreeResult::Scanned(Ok(scan)) => {
                    self.sources.tree.insert_folder(node, &scan);
                    if scan.files.is_empty() {
                        self.toast(format!("No log files in {name}"), cx);
                    }
                    if scan.truncated {
                        self.toast(format!("{name}: showing the first {} files", group_digits(MAX_SCAN_FILES as u64)), cx);
                    }
                }
                TreeResult::Listed(Ok(entries)) => self.sources.tree.insert_listing(node, &entries),
                TreeResult::Scanned(Err(e)) | TreeResult::Listed(Err(e)) => {
                    self.sources.tree.set_listing(node, Listing::Failed(e.to_string()));
                    self.toast(format!("Could not read {name}: {e}"), cx);
                }
                TreeResult::Died => {
                    self.sources.tree.set_listing(node, Listing::Failed("stopped unexpectedly".into()));
                    self.toast(format!("Could not read {name}: stopped unexpectedly"), cx);
                }
            }
            if self.sources.open_first.remove(&node)
                && let Some(first) = self.sources.tree.first_openable(node)
            {
                self.open_node(first, false, window, cx);
            }
        }
        cx.notify();
        let active = !self.sources.jobs.is_empty();
        if !active {
            self.sources.polling = false;
        }
        active
    }

    /// The tab showing `source`.
    fn tab_for(&self, source: &SourceRef, cx: &App) -> Option<usize> {
        self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.source_ref().as_ref() == Some(source))
    }

    /// Opens a file or member row (or activates the tab showing it). `record` puts it in Recent: true for the user's
    /// clicks, false when a folder or archive opens its first log by itself.
    pub fn open_node(&mut self, id: NodeId, record: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.sources.tree.node(id).source.clone() else { return };
        self.open_source_ref(source, record, window, cx);
    }

    fn open_source_ref(&mut self, source: SourceRef, record: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.tab_for(&source, cx) {
            self.activate(index, window, cx);
            return;
        }
        if record {
            let entry = match &source {
                SourceRef::File(path) => RecentEntry::file(path),
                SourceRef::Member { .. } => RecentEntry::member(source.root_path(), source.member_chain().iter().map(|m| m.to_string()).collect()),
            };
            self.record_recent(entry, cx);
        }
        let document = match source {
            SourceRef::File(path) => Source::File(path),
            member => Source::Member(member),
        };
        self.open_source(document, TabInit::default(), window, cx);
    }

    /// Folder and archive rows expand and collapse; an archive is listed the first time it opens.
    pub fn toggle_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.sources.expanded.remove(&id) {
            self.sources.expanded.insert(id);
            let node = self.sources.tree.node(id);
            if matches!(node.kind, NodeKind::Archive(_)) && node.listing == Listing::Pending {
                self.start_listing(id, window, cx);
            }
        }
        cx.notify();
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
            return;
        }
        match (entry.source(), classify(&entry.path)) {
            // An archive member: the archive appears in the sidebar and the member opens.
            (Some(member), PathKind::Archive(kind, size)) => {
                self.open_archive(entry.path.clone(), kind, size, false, window, cx);
                self.open_source_ref(member, true, window, cx);
            }
            _ => self.open_paths(vec![entry.path], window, cx),
        }
    }
```

Change `open_files` to fill `source: d.source.source_ref()` (and keep `path`). Replace `active_footer` and `sidebar`:

```rust
    fn active_footer(&self, cx: &App) -> Option<SidebarFooter> {
        let tab = self.active_tab()?;
        let d = tab.read(cx).doc.read(cx);
        if let Some(node) = d.source.source_ref().and_then(|s| self.sources.tree.find(&s)) {
            return Some(sidebar_model::tree_footer(&self.sources.tree, self.sources.tree.root_of(node)));
        }
        let mut lines: Vec<SharedString> = vec![match d.source.path().and_then(Path::parent) {
            Some(dir) => dir.display().to_string().into(),
            None => d.source.location(),
        }];
        if let Some(text) = d.text() {
            let n = text.line_count();
            lines.push(format!("{} {} · {}", group_digits(n), noun(n, "line", "lines"), crate::format::format_size(text.bytes().len() as u64)).into());
        }
        Some(SidebarFooter { title: d.title(), lines })
    }

    /// The footer the sidebar shows now (tests).
    pub fn sidebar_footer(&self, cx: &App) -> Option<SidebarFooter> {
        match &self.fixed_sidebar {
            Some(fixed) => Some(fixed.footer.clone()),
            None => self.active_footer(cx),
        }
    }

    pub(crate) fn sidebar(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        if !self.sidebar_visible {
            return None;
        }
        let files = self.open_files(cx);
        let active = self.active_tab().map(|t| t.entity_id());
        let failed: HashSet<EntityId> = files.iter().filter(|f| f.failed).map(|f| f.tab).collect();
        let (rows, actions, count, recent) = match &self.fixed_sidebar {
            Some(fixed) => {
                let groups = self.sidebar_groups(&files);
                let (rows, actions) = sidebar_model::rows_with_failures(&groups, active, &self.collapsed, &failed);
                (rows, actions, fixed.count.clone(), fixed.recent.clone())
            }
            None => {
                let tree = &self.sources.tree;
                let (mut rows, mut actions) = sidebar_model::tree_rows(tree, &self.sources.expanded, &files, active);
                // Files opened on their own (not from a folder or archive) keep the M2 grouping by folder.
                let loose: Vec<OpenFile> = files.into_iter().filter(|f| f.source.as_ref().is_none_or(|s| tree.find(s).is_none())).collect();
                let (more_rows, more_actions) = sidebar_model::rows_with_failures(&sidebar_model::group_open_files(&loose), active, &self.collapsed, &failed);
                rows.extend(more_rows);
                actions.extend(more_actions);
                let total = tree.roots().iter().map(|&r| tree.summary(r).files).sum::<u64>() + loose.len() as u64;
                (rows, actions, format!("{total} {}", noun(total, "file", "files")).into(), recent_items(cx))
            }
        };
        let actions = Rc::new(actions);
        Some(
            Sidebar::new()
                .width(self.sidebar_width)
                .count(count)
                .rows(rows)
                .recent(recent)
                .footer(self.sidebar_footer(cx))
                .on_row(act_ix(this, move |ws, i, window, cx| {
                    if let Some(action) = actions.get(i).cloned() {
                        ws.sidebar_action(action, window, cx);
                    }
                }))
                .on_recent(act_ix(this, |ws, i, window, cx| ws.open_recent(i, window, cx)))
                .into_any_element(),
        )
    }
```

and in `sidebar_action` add:

```rust
            RowAction::ToggleNode(id) => self.toggle_node(id, window, cx),
            RowAction::OpenNode(id) => self.open_node(id, true, window, cx),
```

`sidebar_groups` stays (it is only used for the fixed demo sidebar now).

- [ ] **Step 7: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-core && cargo test -p rogg-app`
Expected: PASS — the 6 tree tests, the recent and sidebar-model unit tests, and the M2 workspace/demo tests (loose files are grouped as before; the demo's fixed sidebar is unchanged, so `tests/demo.rs` still sees `tree-row-10`).

- [ ] **Step 8: Commit**

```bash
git add crates/rogg-core crates/rogg-app
git commit -m "feat(app): folders and archives as a lazily listed sidebar tree

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: App — stdin and FIFOs as live tabs

**Files:**
- Modify: `crates/rogg-app/src/args.rs`, `crates/rogg-app/src/main.rs`, `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-app/src/workspace/chrome.rs`, `crates/rogg-app/src/sidebar_model.rs`
- Test: `crates/rogg-app/tests/streams.rs`; unit tests in `args.rs`, `sidebar_model.rs`

**Interfaces:**
- Consumes: `StreamCapture`, `stdin_has_data`, `is_stream_path` (Task 8); `Source::Stream` and live follow (Tasks 10–13).
- Produces:
  - `rogg -` reads stdin; `rogg` with data piped or redirected into it adds `-` itself (spec §4.1 "when stdin is not a TTY": a pipe, FIFO, socket or regular file — never a terminal or `/dev/null`). `USAGE = "usage: rogg [--demo] [FILE...]   (FILE \`-\` reads stdin)"`.
  - `Workspace::open_stream<R: Read + Send + 'static>(&mut self, name: &str, open: impl FnOnce() -> io::Result<R> + Send + 'static, window, cx) -> Option<Entity<Tab>>` — a live tab titled `name` on the capture file; `open_stdin` (once per run) and `open_fifo(path)` use it. FIFO and socket paths opened or dropped become stream tabs (never mapped, never sniffed — opening a FIFO blocks until a writer appears, which happens on the capture thread). Stream tabs are not recorded in Recent and not de-duplicated.
  - Sidebar: stream tabs are grouped under `Streams` (`OpenFile::stream: bool`).

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-app/src/args.rs` tests, add:

```rust
    #[test]
    fn a_dash_reads_stdin() {
        assert_eq!(parse("- a.log").unwrap().paths, [PathBuf::from("-"), PathBuf::from("a.log")]);
        assert!(parse("--x").is_err(), "other dashes are still unknown options");
    }
```

In `crates/rogg-app/src/sidebar_model.rs` tests, add `stream: false` to the existing `OpenFile` literals and:

```rust
    #[test]
    fn streams_are_grouped_apart() {
        let s = EntityId::from(9u64);
        let files = [OpenFile { tab: s, title: "stdin".into(), path: None, bytes: None, failed: false, source: None, stream: true }];
        let groups = group_open_files(&files);
        assert_eq!((groups[0].key.as_ref(), groups[0].label.as_ref()), ("Streams", "Streams"));
    }
```

Create `crates/rogg-app/tests/streams.rs`:

```rust
mod common;
use gpui_kit::{Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::recent::RecentStore;
use rogg_app::tab::Tab;
use std::io::Write;

fn lines(tab: &Entity<Tab>, cx: &mut TestAppContext) -> u64 {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().map_or(0, |x| x.line_count()))
}

#[gpui_kit::test]
fn piped_data_opens_as_a_live_tab(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let (reader, mut writer) = std::io::pipe().unwrap();
    let tab = cx
        .update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("stdin", move || Ok(reader), window, cx)))
        .unwrap()
        .expect("a stream tab");
    writer.write_all(b"one\ntwo\n").unwrap();
    common::wait_until(cx, |cx| lines(&tab, cx) == 2);
    assert_eq!(tab.read_with(cx, |t, cx| t.title(cx).to_string()), "stdin");
    assert!(tab.read_with(cx, |t, _| t.live.following), "a stream follows from the start");
    assert!(common::exists(cx, w, "tool-live"));
    writer.write_all(b"three\n").unwrap();
    drop(writer);
    common::wait_until(cx, |cx| lines(&tab, cx) == 3);
    cx.update(|cx| assert!(cx.global::<RecentStore>().entries().is_empty(), "streams are not recent files"));
}

#[gpui_kit::test]
fn fifos_are_read_as_streams(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let fifo = dir.path().join("events");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![fifo.clone()] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab()).expect("the FIFO opens a tab at once");
    assert_eq!(tab.read_with(cx, |t, cx| t.title(cx).to_string()), "events");
    let writer = std::thread::spawn(move || {
        let mut f = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
        f.write_all(b"x\ny\n").unwrap();
    });
    common::wait_until(cx, |cx| lines(&tab, cx) == 2);
    writer.join().unwrap();
}
```

Add `libc.workspace = true` to `[dev-dependencies]` of `crates/rogg-app/Cargo.toml` (for `mkfifo`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test streams && cargo test -p rogg-app --lib`
Expected: FAIL to compile — `open_stream`, `OpenFile::stream` not found; `a_dash_reads_stdin` fails (`-` is rejected as an unknown option).

- [ ] **Step 3: Arguments and `main`**

In `crates/rogg-app/src/args.rs`:

```rust
pub const USAGE: &str = "usage: rogg [--demo] [FILE...]   (FILE `-` reads stdin)";
```

and in `parse`, before the unknown-flag arm: `Some("-") => out.paths.push(PathBuf::from("-")),`.

`crates/rogg-app/src/main.rs`:

```rust
fn main() {
    match rogg_app::args::Args::parse(std::env::args_os().skip(1)) {
        Ok(mut args) => {
            // Data piped or redirected into rogg opens as a live tab (spec §4.1).
            if rogg_core::stream::stdin_has_data() && !args.paths.iter().any(|p| p.as_os_str() == "-") {
                args.paths.push("-".into());
            }
            rogg_app::run(args)
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message == rogg_app::args::USAGE { 0 } else { 2 });
        }
    }
}
```

- [ ] **Step 4: Stream tabs**

In `crates/rogg-app/src/workspace/mod.rs`, add the fields `pub(crate) streams: Vec<rogg_core::stream::StreamCapture>,` (the captures keep their threads' state alive) and `stdin_opened: bool` (both default).

In `crates/rogg-app/src/workspace/chrome.rs`:
- `PathKind` gains `Stream`; `classify` checks it right after `is_dir` — before anything reads the file:

```rust
    if rogg_core::stream::is_stream_path(path) {
        return PathKind::Stream;
    }
```

- `open_paths` handles `-` before canonicalising, and streams:

```rust
        for path in paths {
            if path.as_os_str() == "-" {
                self.open_stdin(window, cx);
                continue;
            }
            let path = path.canonicalize().unwrap_or(path);
            match classify(&path) {
                PathKind::Stream => self.open_fifo(path, window, cx),
                // … the other arms as in Task 14
            }
        }
```

- add:

```rust
    /// stdin (spec §4.1): captured into the cache as it arrives, in a live tab. Opened once per run.
    pub fn open_stdin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.stdin_opened, true) {
            return;
        }
        self.open_stream("stdin", || Ok(std::io::stdin()), window, cx);
    }

    /// A FIFO or socket (`mkfifo`, `<(cmd)`): read as a stream, never mapped. Opening it waits for a writer, which
    /// happens on the capture thread.
    pub fn open_fifo(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "pipe".into());
        self.open_stream(&name, move || std::fs::File::open(&path), window, cx);
    }

    /// Starts copying `open`'s data into the cache and opens a live tab on the copy. Streams are not recent files.
    pub fn open_stream<R: std::io::Read + Send + 'static>(
        &mut self,
        name: &str,
        open: impl FnOnce() -> std::io::Result<R> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Tab>> {
        match rogg_core::stream::StreamCapture::start(open, &CacheStore::get(cx)) {
            Ok(capture) => {
                let source = Source::Stream { name: name.to_string().into(), path: capture.path().to_path_buf() };
                self.streams.push(capture);
                Some(self.open_source(source, TabInit::default(), window, cx))
            }
            Err(e) => {
                self.toast(format!("Could not read {name}: {e}"), cx);
                None
            }
        }
    }
```

(import `crate::tab::Tab`.) `open_files` sets `stream: matches!(d.source, Source::Stream { .. })`.

In `crates/rogg-app/src/sidebar_model.rs`, `OpenFile` gains `pub stream: bool,` and `group_open_files` puts streams in their own group:

```rust
        let (key, label): (SharedString, SharedString) = match f.path.as_ref().and_then(|p| p.parent()) {
            _ if f.stream => ("Streams".into(), "Streams".into()),
            Some(dir) => (/* … as before … */),
            None => ("In memory".into(), "In memory".into()),
        };
```

- [ ] **Step 5: Run the tests**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app`
Expected: PASS. Then a hand check: `printf 'one\ntwo\n' | cargo run -p rogg-app` opens a `stdin` tab with two lines (close the window afterwards).

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-app Cargo.lock
git commit -m "feat(app): stdin and FIFOs open as live stream tabs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 16: Carry-over fixes — display cut, path spellings, late filtered selection, drag handles, demo with files, Hide, clippy

**Files:**
- Modify: `crates/rogg-ui/src/logview/text.rs`, `crates/rogg-ui/src/logview/element.rs`, `crates/rogg-ui/src/layout.rs`, `crates/rogg-ui/src/filter.rs`
- Modify: `crates/rogg-app/src/rows.rs`, `crates/rogg-app/src/tab.rs`, `crates/rogg-app/src/demo.rs`, `crates/rogg-app/src/actions.rs`, `crates/rogg-app/src/menus.rs`, `crates/rogg-app/src/lib.rs`, `crates/rogg-app/src/workspace/mod.rs`, `crates/rogg-app/src/workspace/chrome.rs`
- Modify (clippy): `crates/rogg-ui/tests/common/mod.rs`, `crates/rogg-ui/tests/logview_real_text.rs`, `crates/rogg-gallery/src/replica/state.rs`
- Test: unit tests in `rows.rs`, `layout.rs`, `menus.rs`; `crates/rogg-app/tests/workspace.rs`, `crates/rogg-app/tests/demo.rs`, `crates/rogg-app/tests/tab.rs`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `rogg_ui::logview::text::MAX_DISPLAY_TEXT_BYTES = 3 * MAX_DISPLAY_BYTES + 3`: the log view cuts only decoded text longer than this; producers cut raw lines at `MAX_DISPLAY_BYTES` (on a UTF-8 boundary) and append `…` themselves (`rogg_app::rows::display_line`), so a ≤ 64 KB line full of invalid bytes is never marked as cut (M2 carry-over).
  - `Workspace` de-duplicates file tabs by canonical path (M2 carry-over: a missing file opened raw, retried, then opened canonically).
  - A selection that becomes shown in the filtered pane after a background scan is selected there (M2 carry-over).
  - `BandDrag { grab }` / `SidebarDrag { grab }` remember where the handle was grabbed (`BandDrag::band_top(pointer_y)`, `SidebarDrag::width(pointer_x)`), so a drag never jumps; the dragged handle's resize cursor stays for the whole drag (M2 carry-over).
  - `rogg --demo FILE` activates the demo's first tab (M2 carry-over).
  - App menu `rogg`: `Hide rogg` (⌘H), `Hide Others` (⌥⌘H), `Show All`, separator, `Quit rogg`; actions `Hide`, `HideOthers`, `ShowAll` (M2 carry-over R11).
  - `cargo clippy --workspace --all-targets` prints no warnings (the pre-existing ones are fixed).

- [ ] **Step 1: Write the failing tests**

In `crates/rogg-app/src/rows.rs` tests, replace `long_lines_are_truncated_for_display_but_copied_in_full`'s first assertion and add two tests:

```rust
        assert_eq!(display.len(), MAX_DISPLAY_BYTES + '…'.len_utf8(), "cut at the limit and marked");
        assert!(display.ends_with('…'));
```

```rust
    #[test]
    fn invalid_bytes_in_a_short_line_are_not_marked_as_cut() {
        let t = Arc::new(LogText::from_bytes(vec![0xFF; 30_000]));
        let display = display_line(&t, 0);
        assert_eq!(display.chars().count(), 30_000, "every invalid byte becomes one U+FFFD");
        assert!(!display.ends_with('…'));
        let (shown, cut) = rogg_ui::logview::text::display_text(&display, rogg_ui::logview::text::MAX_DISPLAY_TEXT_BYTES);
        assert!(!cut && shown.len() == display.len(), "the log view keeps it whole");
    }

    #[test]
    fn the_display_cut_never_splits_a_character() {
        let mut raw = vec![b'a'; MAX_DISPLAY_BYTES - 1];
        raw.extend("é".as_bytes()); // straddles the limit
        raw.extend(b"tail");
        let t = Arc::new(LogText::from_bytes(raw));
        let display = display_line(&t, 0);
        assert_eq!(display.as_ref(), format!("{}…", "a".repeat(MAX_DISPLAY_BYTES - 1)));
    }
```

In `crates/rogg-ui/src/layout.rs` tests, add:

```rust
    #[test]
    fn drags_keep_the_grab_offset() {
        let band = BandDrag::default();
        band.grab.set(6.);
        assert_eq!(band.band_top(px(100.)), px(98.), "handle grabbed 6 px into it, 4 px above the band");
        let side = SidebarDrag::default();
        side.grab.set(1.);
        assert_eq!(side.width(px(250.)), px(252.), "edge grabbed 1 px into it, 3 px left of the sidebar edge");
    }
```

In `crates/rogg-app/src/menus.rs` tests, extend `menus_follow_the_spec_and_list_recent_entries`:

```rust
        let app = &menus[0];
        assert_eq!(app.items.len(), 5, "Hide rogg, Hide Others, Show All, separator, Quit rogg");
```

and in `crates/rogg-app/src/actions.rs` tests:

```rust
    #[test]
    fn hide_is_bound_to_cmd_h() {
        let rows = shortcut_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Hide rogg" && keys == "⌘ H"));
    }
```

Append to `crates/rogg-app/tests/workspace.rs`:

```rust
#[gpui_kit::test]
fn one_tab_per_file_whatever_the_path_spelling(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let raw = dir.path().join("sub/../late.log");
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![raw] });
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1, "a missing file opens an error tab");
    common::write_log(dir.path(), "late.log", &["here"]);
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    doc.update(cx, |d, cx| d.retry(cx));
    let canonical = dir.path().join("late.log").canonicalize().unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![canonical], window, cx))).unwrap();
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1, "the same file activates its tab");
}
```

Append to `crates/rogg-app/tests/demo.rs`:

```rust
#[gpui_kit::test]
fn demo_with_files_shows_the_demo_first(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let file = common::write_log(dir.path(), "mine.log", &["x"]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: true, paths: vec![file] });
    let title = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).title(cx).to_string());
    assert_eq!(title, rogg_app::demo::TABS[0]);
}
```

Append to `crates/rogg-app/tests/tab.rs` (use its existing imports; add `EngineConfig`, `TabInit`, `DocumentInit`, `FilterSpec` as needed):

```rust
#[gpui_kit::test]
fn a_selection_shown_after_a_background_scan_is_selected_in_the_filtered_pane(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..50_000).map(|i| if i % 100 == 0 { format!("needle {i}") } else { format!("hay {i}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let path = common::write_log(dir.path(), "big.log", &refs);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let init = TabInit { doc: DocumentInit { spec: FilterSpec::from_expression("needle"), ..Default::default() }, selection: Some(201), ..Default::default() };
    let tab = cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_source(Source::File(path), init, window, cx))).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).filter.progress.is_none() && t.doc.read(cx).text().is_some()));
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).selected() == Some(201)));
}
```

(Display number 201 is line 200, `needle 200`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-ui --lib layout && cargo test -p rogg-app`
Expected: FAIL — `MAX_DISPLAY_TEXT_BYTES`, `BandDrag::default`, the Hide actions don't exist; the path-spelling, demo and late-selection tests fail.

- [ ] **Step 3: The display cut**

In `crates/rogg-ui/src/logview/text.rs`, after `MAX_DISPLAY_BYTES`:

```rust
/// The log view cuts decoded text only beyond this. Row producers cut raw lines at `MAX_DISPLAY_BYTES` before decoding
/// and mark the cut themselves; decoding can triple a line's size (each invalid byte becomes a 3-byte U+FFFD), so a
/// short line full of invalid bytes still fits.
pub const MAX_DISPLAY_TEXT_BYTES: usize = 3 * MAX_DISPLAY_BYTES + 3;
```

In `crates/rogg-ui/src/logview/element.rs`, `shape_row` calls `display_text(&line.text, MAX_DISPLAY_TEXT_BYTES)` (import it); the horizontal extent keeps `min(MAX_DISPLAY_BYTES + 1)` (a displayed line holds at most that many characters).

In `crates/rogg-app/src/rows.rs`, replace `display_line`:

```rust
/// Line `i` for display (spec §4.4): a line over `MAX_DISPLAY_BYTES` is cut there — never inside a UTF-8 character —
/// and marked with `…`; invalid bytes become U+FFFD. Only the kept bytes are decoded, so a multi-megabyte line costs
/// no more than a 64 KB one.
pub fn display_line(text: &LogText, i: u64) -> SharedString {
    let raw = text.line(i);
    if raw.len() <= MAX_DISPLAY_BYTES {
        return String::from_utf8_lossy(raw).into_owned().into();
    }
    // Back up over continuation bytes (at most 3) so the cut falls before a character, not inside it.
    let mut cut = MAX_DISPLAY_BYTES;
    while cut > MAX_DISPLAY_BYTES - 3 && raw[cut] & 0xC0 == 0x80 {
        cut -= 1;
    }
    let mut display = String::from_utf8_lossy(&raw[..cut]).into_owned();
    display.push('…');
    display.into()
}
```

- [ ] **Step 4: One tab per file**

In `crates/rogg-app/src/workspace/chrome.rs`:

```rust
/// Two spellings of one file: equal paths, or equal canonical paths once both exist.
fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}
```

and `open_file` finds an existing tab with `source.path().is_some_and(|p| same_file(p, &path))`.

- [ ] **Step 5: A late filtered selection**

In `crates/rogg-app/src/tab.rs` `sync_rows`, after the filtered rows are replaced:

```rust
        // The tab's selection may become shown only now (a background scan finished, a filter changed): select it in
        // the filtered pane too, without scrolling (M2 carry-over).
        if let Some(number) = self.selection
            && self.filtered.read(cx).selected().is_none()
            && self.filtered.read(cx).rows().row_for_number(number).is_some()
        {
            self.filtered.update(cx, |s, cx| s.set_selected(Some(number), cx));
        }
```

- [ ] **Step 6: Drag handles keep their grab offset and cursor**

In `crates/rogg-ui/src/layout.rs`, replace the drag markers:

```rust
/// The filter band's handle being dragged. `grab` is where the pointer took the handle (px below its top), recorded
/// when the drag starts so the band does not jump.
#[derive(Clone, Default)]
pub struct BandDrag {
    pub grab: Rc<Cell<f32>>,
}

impl BandDrag {
    /// The handle sits 4 px above the band's top edge (`filter.rs`).
    const HANDLE_ABOVE: f32 = 4.;

    /// Where the band's top edge goes for the pointer at `y`.
    pub fn band_top(&self, y: Pixels) -> Pixels {
        y - px(self.grab.get()) + px(Self::HANDLE_ABOVE)
    }
}

/// The sidebar's resize edge being dragged; `grab` is the pointer's offset into the 6 px edge.
#[derive(Clone, Default)]
pub struct SidebarDrag {
    pub grab: Rc<Cell<f32>>,
}

impl SidebarDrag {
    /// The edge starts 3 px left of the sidebar's right edge.
    const EDGE_INSET: f32 = 3.;

    /// The sidebar width for the pointer at `x` (the sidebar starts at the window's left edge).
    pub fn width(&self, x: Pixels) -> Pixels {
        x - px(self.grab.get()) + px(Self::EDGE_INSET)
    }
}
```

(imports `std::cell::Cell`, `std::rc::Rc`). The `on_drag` constructors record the offset — in `layout.rs`:

```rust
                        .on_drag(SidebarDrag::default(), |drag: &SidebarDrag, offset: Point<Pixels>, _, cx| {
                            drag.grab.set(f32::from(offset.x));
                            cx.new(|_| EmptyView)
                        }),
```

and in `filter.rs`:

```rust
                        .on_drag(BandDrag::default(), |drag: &BandDrag, offset: Point<Pixels>, _, cx| {
                            drag.grab.set(f32::from(offset.y));
                            cx.new(|_| EmptyView)
                        })
```

Keep the dragged handle's cursor for the whole drag: in `MainWindowLayout`'s render (both the panes and the single-body layouts), add as the root's last child

```rust
            .child(
                gpui_kit::canvas(|_, _, _| {}, |_, _, window, cx| {
                    if let Some(style) = cx.active_drag_cursor_style() {
                        window.set_window_cursor_style(style);
                    }
                })
                .absolute()
                .size_0(),
            )
```

(`set_window_cursor_style` overrides every hitbox cursor; it only applies while a drag with a cursor is active).

In `crates/rogg-app/src/workspace/mod.rs` `root`:

```rust
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<BandDrag>, _, cx| {
                let top = e.drag(cx).band_top(e.event.position.y);
                ws.drag_band(top, cx);
            }))
            .on_drag_move(cx.listener(|ws, e: &DragMoveEvent<SidebarDrag>, _, cx| {
                let width = e.drag(cx).width(e.event.position.x);
                ws.drag_sidebar(width, cx);
            })),
```

and `drag_sidebar(width)` takes the width (doc: "Sidebar edge drag: sets the sidebar's width, clamped to 155–360 px"). The existing `drag_handles_resize_panes_and_sidebar` test calls these directly and keeps passing.

- [ ] **Step 7: `--demo FILE`, Hide, and clippy**

`crates/rogg-app/src/demo.rs` `open`: remember `let first = ws.tabs().len();` before opening the demo tabs and `ws.activate(first, window, cx)` after.

`crates/rogg-app/src/actions.rs`: add `Hide`, `HideOthers`, `ShowAll` to `actions!`, and before `Quit` in the keymap:

```rust
        shortcut!("Hide rogg", "⌘ H", "cmd-h", Hide, None),
        shortcut!("Hide others", "⌥ ⌘ H", "alt-cmd-h", HideOthers, None),
```

`crates/rogg-app/src/lib.rs` `install_globals`, next to the `Quit` handler:

```rust
    cx.on_action(|_: &actions::Hide, cx| cx.hide());
    cx.on_action(|_: &actions::HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &actions::ShowAll, cx| cx.unhide_other_apps());
```

`crates/rogg-app/src/menus.rs`, the `rogg` menu:

```rust
        Menu::new("rogg").items([
            MenuItem::action("Hide rogg", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit rogg", Quit),
        ]),
```

Clippy (pre-existing warnings, M2 carry-over):
- `rogg-ui/src/logview/element.rs` `Shaped` and `RowBody`: box the large payload clippy names (`Shaped::Line(Box<ShapedLine>)`; for `RowBody`, the variant clippy reports) and adjust their constructors and matches.
- `rogg-ui/src/logview/text.rs` test: `.expect(&format!("no segment at {}", pos))` → `.unwrap_or_else(|| panic!("no segment at {pos}"))`.
- `rogg-ui/tests/common/mod.rs`: `type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;` and `pub struct Host(pub Build);`.
- `rogg-ui/tests/logview_real_text.rs`: `type Case = (&'static str, fn(&mut HeadlessAppContext));` and `let cases: [Case; 3] = …`.
- `rogg-gallery/src/replica/state.rs` lines ~148 and ~174: collapse the nested `if`s into `if let … && …` chains.

- [ ] **Step 8: Run everything**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep -E "^warning|^error" | grep -v "future version"; just visual-check; echo "visual-check exit $?"`
Expected: all tests PASS; clippy prints no warnings; `just visual-check` fails only on the 9 R3 replica captures (unchanged from M2; do not approve goldens).

- [ ] **Step 9: Commit**

```bash
git add crates
git commit -m "fix: M2 carry-over — display cut, path spellings, late selection, drag handles, demo, Hide, clippy

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 17: End to end — a bundle, a live log, hand checks and notes

**Files:**
- Create: `crates/rogg-app/tests/bundle_e2e.rs`, `docs/superpowers/notes/2026-10-07-m3-manual-checks.md`
- Modify: `justfile`, `docs/superpowers/reviews/2026-10-07-m2-visual.md` (a dated "After M3" note), this plan's "Deferred from M3" section (already written below — keep it in sync with what was built)
- Test: `crates/rogg-app/tests/bundle_e2e.rs`

**Interfaces:**
- Consumes: everything above; `rogg_genlog::bundle::generate_bundle` (Task 3).
- Produces: an end-to-end test of the M3 flows through the real workspace; `just live` / `just pipe` recipes for hand checks; the M3 hand-check list.

- [ ] **Step 1: Write the end-to-end tests**

Create `crates/rogg-app/tests/bundle_e2e.rs`:

```rust
mod common;
use gpui_kit::{Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;
use rogg_genlog::GenOptions;
use rogg_genlog::bundle::{BundleKind, generate_bundle};
use std::io::Write;

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Entity<Tab> {
    ws.read_with(cx, |ws, _| ws.active_tab().unwrap())
}

fn loaded(tab: &Entity<Tab>, cx: &mut TestAppContext) -> bool {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().is_some())
}

/// Spec §8 rogg-app flow on an archive: open a bundle → its first member opens → filter → pin → open another member
/// → come back: the first tab kept its filter and pin.
#[gpui_kit::test]
fn a_bundle_opens_filters_and_keeps_tab_state(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let bundle = dir.path().join("support.tgz");
    generate_bundle(&bundle, BundleKind::TarGz, 3, &GenOptions { size_bytes: 200_000, ..Default::default() }).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![bundle] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    let first = active(&ws, cx);
    common::wait_until(cx, |cx| loaded(&first, cx));
    assert_eq!(first.read_with(cx, |t, cx| t.title(cx).to_string()), "service-00.log");
    let doc = first.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("ERROR", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && !d.filter.matches.is_empty()));
    doc.update(cx, |d, cx| d.toggle_pin(3, cx));
    // support.tgz, service-00.log, service-01.log, service-02.log
    common::click(cx, w, "tree-row-2");
    let second = active(&ws, cx);
    common::wait_until(cx, |cx| loaded(&second, cx));
    assert_eq!(second.read_with(cx, |t, cx| t.title(cx).to_string()), "service-01.log");
    common::click(cx, w, "tree-row-1");
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 0, "the open member's tab is activated, not reopened");
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.spec.expression(), d.filter.pins.contains(&3))), ("ERROR".to_string(), true));
    let footer = ws.read_with(cx, |ws, cx| ws.sidebar_footer(cx)).unwrap();
    assert_eq!(footer.title.as_ref(), "support.tgz");
    assert!(footer.lines[0].starts_with("3 members · "), "{:?}", footer.lines);
}

/// Spec §8: live append and truncate through the window.
#[gpui_kit::test]
fn a_live_log_appends_filters_and_reloads(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &["start", ""]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    let tab = active(&ws, cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("ERROR", cx)).unwrap();
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    for i in 0..5 {
        writeln!(f, "line {i} ERROR").unwrap();
    }
    drop(f);
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.matches.len() == 5));
    assert!(tab.read_with(cx, |t, cx| t.doc.read(cx).is_live()));
    std::fs::write(&path, "fresh\n").unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some_and(|t| t.line_count() == 1)));
    assert!(doc.read_with(cx, |d, _| d.filter.matches.is_empty()), "the reloaded file is filtered again");
}
```

- [ ] **Step 2: Run them**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test -p rogg-app --test bundle_e2e`
Expected: PASS (the features exist since Tasks 10–15; if one fails, the defect is in the task that owns that behaviour — fix it there, with a test).

- [ ] **Step 3: Recipes for hand checks**

Append to `justfile`:

```make
# Appends a log line every 100 ms to FILE (live follow by hand; Ctrl-C to stop).
live out="target/genlog/live.log":
    mkdir -p target/genlog
    touch {{out}}
    while true; do echo "$(date '+%Y-%m-%d %H:%M:%S.000')  INFO   ticker       tick $RANDOM" >> {{out}}; sleep 0.1; done

# Pipes FILE into rogg (stdin capture).
pipe file="target/genlog/sample.log":
    cat {{file}} | cargo run --release -p rogg-app
```

- [ ] **Step 4: The hand-check list**

Create `docs/superpowers/notes/2026-10-07-m3-manual-checks.md`:

```markdown
# M3 hand checks

The automated tests cover the behaviour; these checks cover what only a real machine shows (FSEvents, real bundles,
memory, the native window). Use release builds (`just demo`, `just run …`).

## Bundles
- `just bundle tgz 10 1G target/genlog/big.tgz`, then `just run target/genlog/big.tgz`: the sidebar lists 10 members
  with sizes; the first member opens with `Extracting service-00.log` progress and a working Cancel; the footer reads
  `10 members · 10.0 GB expanded` / `Members open individually`.
- A real support bundle (zip with nested `.tgz` and rotated `*.log.N.gz`): nested archives list on first expand
  (`listing…`), `.gz` members decompress, a damaged member shows the error state with Retry, other members open.
- Reopen a member: it opens at once (cache hit). `~/Library/Caches/rogg` holds `*.member` / `*.log` entries; with
  the limit lowered by hand in code to 1 GB, closed members are evicted on the next open; open ones never are.

## Compressed files
- `gzip -k` a 1 GB log and open the `.gz`: rows appear while the footer shows `Decompressing N%`; scrolling and
  filtering work meanwhile; the end arrives without a reload.

## Live follow
- `just live` in one terminal, `just run target/genlog/live.log` in another: new lines scroll in (`Pause` control);
  Space pauses (`Paused +N` counts), the wheel up pauses too; Space resumes at the end.
- `: > target/genlog/live.log` while it runs: the toast `File was truncated or replaced — reloaded.`, no crash.
- Rotation: `mv live.log live.log.1` then let `just live` recreate it: the tab reloads the new file.
- Truncate a 1 GB file (`: > big.log`) while scrolling it: no crash (SIGBUS guard), then the reload.

## stdin and FIFOs
- `just pipe`: a `stdin` tab fills while `cat` runs and follows its end.
- `rogg <(tail -f target/genlog/live.log)`: a stream tab that keeps following.
- Starting rogg from Finder (stdin is `/dev/null`) opens no stdin tab.

## Window
- ⌘H hides rogg, ⌥⌘H hides others, the rogg menu has Show All.
- Dragging the filter band or the sidebar edge never jumps at the start, and the resize cursor stays during the drag.
```

- [ ] **Step 5: Note the app-capture change in the M2 report**

Run `just visual-app` and append to `docs/superpowers/reviews/2026-10-07-m2-visual.md`:

```markdown
## After M3 (2026-10-07)
The keyboard-shortcuts dialog lists three more shortcuts added in M3 (`Pause / resume live follow` · Space, `Hide
rogg` · ⌘H, `Hide others` · ⌥⌘H), so the app's `--dialog shortcuts` captures are three rows taller than measured
above (new dialog height delta: <value from target/visual-app/results.json>). Nothing else in the app captures changed.
```

Fill `<value …>` from `target/visual-app/results.json` (the `dialog` height delta of `dark-compact--shortcuts`). If any other capture changed by more than the M2 numbers, list it here as well and say why.

- [ ] **Step 6: Run the full suite**

Run: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"; cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep -E "^warning|^error" | grep -v "future version"; just visual-check; echo "visual-check exit $?"`
Expected: all tests PASS, no clippy warnings, `just visual-check` fails only on the 9 R3 replica captures.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-app/tests/bundle_e2e.rs justfile docs/superpowers
git commit -m "test: M3 end-to-end flows, hand-check list and recipes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Deferred from M3

- **R3 (pending with the user, from M2):** the horizontal scrollbar thumb and its hit zone over the bottom row; the replica goldens are re-approved only after that decision.
- **M4:** settings for the cache limit (20 GB default) and folder scan depth (4), session restore (stream tabs are never restored), Open filtered as tab, Export, profiles, statistics popover.
- **Later (post-v1):** opening a file on a hung network mount still stats it on the UI thread before going to the background (a hung `stat` hangs the window); a plain file is indexed before it shows (≈150 ms per GB, so no progressive indexing); a UTF-16 file is not live-followed (its conversion is a snapshot); an in-place rewrite that keeps the file's first 64 bytes reads as an append; listing a `.tar.gz` decompresses it once end to end (tar has no index); a FIFO that fails to open shows an empty stream tab (the failure is reported once with a toast, not in the tab); sidebar rows are not virtualized (R12), so a large folder or archive level (e.g. 10,000 files or 50,000 flat members) builds every row; each live-follow growth copies all matches and the full-log search result on the UI thread (O(matches)); each growth clones the whole line index on the follower thread; every live-followed tab keeps the 16 ms poll loop running; trailing bytes after the last gzip member fail the decode after the data was shown; listing a plain `.tar` reads every byte; a truncated `.gz` switches the tab to the error state (spec §4.13) instead of keeping the decompressed part.
