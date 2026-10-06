# rogg M1 — Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `rogg-core`, the UI-agnostic log engine:
- open a plain file (UTF-8, UTF-8 BOM, or UTF-16 transcoded), memory-map it and build a parallel line index;
- parse filter expressions and compile them;
- scan in parallel with progressive, cancellable results;
- compose matches, pins and context into a gap-aware filtered view;
- produce highlight spans and per-panel search, statistics and history.

It also adds `genlog`, a deterministic log generator, plus property tests and benchmarks that meet the spec's targets.

**Architecture:**
- `LogText` owns the UTF-8 bytes (an `Mmap`, or a `Vec` for small and in-memory inputs) and a compact `LineIndex` (u64 block bases plus u32 offsets).
- `CompiledFilter` turns a `FilterSpec` into one Aho-Corasick automaton (plain mode) or one `RegexSet` (regex mode) for the include terms, and the same for the exclude terms.
  - It scans whole chunks of lines: plain terms in one pass over the chunk's bytes, regex terms line by line.
- The engine fans chunks out over rayon in waves and emits chunk results in order, so a UI can show partial results.
- `FilteredView` is built from three independent inputs (matches, pins, context) and resolves a row index to a line or gap row in O(1024) using checkpoints. It never materializes one row object per line.

**Tech Stack:** Rust 1.99.0 (edition 2024), `memmap2`, `memchr`, `aho-corasick`, `regex`, `rayon`, `encoding_rs`, `thiserror`, `serde`/`serde_json`; dev: `proptest`, `criterion`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-10-06-rogg-design.md`. §4.3–4.4 and §4.6–4.13 are implemented here. §4.1 is partly implemented: plain files only. §4.5 (live follow) and §4.9 (derived sources) are designed for but deferred to M3/M4. Also §8 (`rogg-core` tests, property tests, benchmarks) and §9 (M1).

## Global Constraints

- `rogg-core` has **no gpui dependency** (spec §3.2). It is a plain Rust library: tests are `#[test]` / `proptest!`, with no gpui harness.
- Toolchain: Rust `1.99.0`, edition `2024`. Platform: macOS on Apple Silicon.
- Prefix every cargo or just command with `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH";`.
- New dependencies get **exact pins** in `[workspace.dependencies]`:
  - `memmap2 = "=0.9.11"`, `memchr = "=2.8.3"`, `regex = "=1.13.1"`, `aho-corasick = "=1.1.5"`
  - `rayon = "=1.12.0"`, `encoding_rs = "=0.8.42"`, `thiserror = "=2.0.21"`
  - `proptest = "=1.11.0"`, `criterion = "=0.8.2"`, `tempfile = "=3.27.0"`
  - `genlog` uses no third-party crates.
- Errors are a typed `thiserror` enum (`CoreError`). **The core never panics on input data** (spec §4.13). Indexing past `line_count()` returns an empty line rather than panicking.
- Line content excludes the `\n` terminator and one trailing `\r` (spec §4.3). Matching sees the full line; the 64 KB display truncation is a UI concern (spec §4.4, already in `rogg-ui`).
- Matching is case-insensitive by default. Plain terms use ASCII case folding; regex terms use the regex engine's case-insensitive mode (spec §4.6, §4.8).
- Filter semantics are exactly spec §4.7.
- Performance targets on this M-series Mac (spec §8):
  - index 1 GB < 2 s;
  - filter 1 GB with 4 plain terms < 1.5 s;
  - filter with 2 regex terms < 3 s;
  - index memory for 1 GB / 10 M lines < 100 MB beyond the memory map.
- Persistence paths (macOS): `~/Library/Application Support/rogg/` (history), `~/Library/Caches/rogg/` (transcoded files).
- Commit messages are conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

The input classes and failure modes most likely to hurt a real user. Each is pinned by tests in the owning task.

1. **Degenerate file shapes:** empty file, a file that is only `\n`, no trailing newline, one 1 MB line with no newline, and a block whose byte span would overflow u32. Expected: correct line counts and ranges, no panic, no wraparound. Tests in Task 1 and Task 3.
2. **Line endings and BOMs:** CRLF, mixed and lone `\r`, a UTF-8 BOM, UTF-16 LE/BE, and an odd trailing byte in UTF-16. Expected: a trailing `\r` is never matched or displayed, the BOM never appears in line 1, and a truncated UTF-16 unit becomes U+FFFD. Tests in Task 2 and Task 3.
3. **Invalid UTF-8 in lines:** raw-byte matching still finds terms, display text is lossy, and highlight spans computed on the display string always land on char boundaries (the M0 renderer crashed on misaligned offsets). Tests in Task 3 and Task 8.
4. **Superseded or cancelled scans:** cancel before the first chunk, cancel mid-scan, and dropping a running job. Expected: no `Finished` after cancel, no chunk events after `Cancelled`, and dropping joins the worker thread. Tests in Task 6.
5. **Pathological terms:**
   - a regex that matches the empty string (`x*`, `^`) and an invalid regex, whose error names the offending term;
   - `\|\|` escaping;
   - overlapping terms (`time` and `timeout`), where per-term hit counts each count a line once;
   - 200 terms.

   Tests in Tasks 4, 5 and 8.

---

## File Structure

```
Cargo.toml                                   workspace: add crates/rogg-core, crates/rogg-genlog; exact dependency pins
justfile                                     add `bench` and `genlog` recipes (Task 10, Task 11)
crates/rogg-core/
  Cargo.toml
  src/lib.rs                                 module list + re-exports
  src/cancel.rs                              CancelToken
  src/error.rs                               CoreError
  src/index.rs                               LineIndex, build_index (parallel), IndexProgress
  src/encoding.rs                            Encoding, LineEndings, status_label, transcode_to_utf8
  src/text.rs                                LogText (mmap/owned bytes + index), OpenProgress
  src/filter/mod.rs                          re-exports
  src/filter/spec.rs                         TermState, Term, FilterSpec, split/join_expression
  src/filter/compiled.rs                     CompiledFilter, ScanResult (chunk scan)
  src/engine.rs                              scan_lines, run_filter, FilterJob, FilterEvent, FilterResult
  src/view.rs                                FilteredView, ViewRow
  src/highlight.rs                           Highlighter (display-string spans)
  src/search.rs                              SearchQuery, CompiledSearch, SearchScope, SearchResult
  src/stats.rs                               FilterStats, TermHits, group_digits
  src/history.rs                             History, HistoryEntry, LoadOutcome
  src/paths.rs                               app_support_dir, cache_dir, history_path
  tests/index_props.rs                       proptests: parallel == serial == naive, incremental == full
  tests/text.rs                              LogText on real temp files
  tests/filter_props.rs                      proptest: chunk scan == naive per-line evaluator
  tests/engine.rs                            ordering, progress, cancellation, FilterJob
  tests/view_props.rs                        proptest: FilteredView == naive row model
  tests/memory.rs                            ignored: 1 GB index memory budget (run by `just bench`)
  benches/engine.rs                          criterion: index, filter_plain_4, filter_regex_2
crates/rogg-genlog/
  Cargo.toml                                 lib + bin `genlog`, no dependencies
  src/lib.rs                                 GenOptions, generate, generate_file, parse_size
  src/main.rs                                CLI
docs/superpowers/reviews/2026-10-07-m1-benchmarks.md   benchmark results vs targets (Task 11)
```

---

### Task 1: rogg-core scaffold, cancellation, errors and the line index

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/rogg-core/Cargo.toml`, `crates/rogg-core/src/{lib.rs,cancel.rs,error.rs,index.rs}`
- Test: unit tests in `index.rs`; `crates/rogg-core/tests/index_props.rs`

**Interfaces:**
- Produces:
  - `rogg_core::CancelToken` (Clone, Default): `new()`, `cancel()`, `is_cancelled() -> bool`
  - `rogg_core::CoreError::{Io { path: PathBuf, source: io::Error }, InvalidRegex { term: String, message: String }, Cancelled}`
  - `rogg_core::index::{BLOCK_LINES = 4096, DEFAULT_CHUNK_BYTES = 16 MiB, LineIndex, IndexProgress { bytes_done, bytes_total }, build_index(data, chunk_bytes, &CancelToken, on_wave: FnMut(&LineIndex, IndexProgress)) -> Result<LineIndex, CoreError>, build_index_into(LineIndex, …)}`
  - `LineIndex` methods:
    - `new()`, `with_max_block_span(u64)`, `from_bytes(&[u8])`, `extend_to(&[u8])`
    - `line_count() -> u64`, `data_len() -> u64`, `longest_line() -> u64`
    - `line_start(u64) -> u64`, `line_range(u64) -> Range<u64>` (excludes `\n`, keeps `\r`)
    - `heap_bytes() -> usize`, `shrink_to_fit()`

A line starts at offset 0 (if the data is non-empty) and after every `\n` that is not the last byte. So `"a\n"` has 1 line and `"\n"` has 1 empty line. Incremental `extend_to` gives exactly the same index as building from scratch, which M3 live-follow relies on.

- [ ] **Step 1: Add the crate and dependency pins**

In the root `Cargo.toml`, set `members = ["crates/rogg-ui", "crates/rogg-gallery", "crates/rogg-core"]` and append to `[workspace.dependencies]`:
```toml
memmap2 = "=0.9.11"
memchr = "=2.8.3"
regex = "=1.13.1"
aho-corasick = "=1.1.5"
rayon = "=1.12.0"
encoding_rs = "=0.8.42"
thiserror = "=2.0.21"
proptest = "=1.11.0"
criterion = "=0.8.2"
tempfile = "=3.27.0"
```

`crates/rogg-core/Cargo.toml`:
```toml
[package]
name = "rogg-core"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies]
memmap2.workspace = true
memchr.workspace = true
regex.workspace = true
aho-corasick.workspace = true
rayon.workspace = true
encoding_rs.workspace = true
thiserror.workspace = true
serde.workspace = true
serde_json.workspace = true

[dev-dependencies]
proptest.workspace = true
tempfile.workspace = true
```

`crates/rogg-core/src/lib.rs`:
```rust
//! rogg-core: the UI-agnostic log engine (spec §4). No gpui dependency.

pub mod cancel;
pub mod error;
pub mod index;

pub use cancel::CancelToken;
pub use error::CoreError;
```

`crates/rogg-core/src/cancel.rs`:
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared cancellation flag checked by long-running jobs between chunks.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
```

`crates/rogg-core/src/error.rs`:
```rust
use std::path::PathBuf;

/// Every fallible core operation returns this (spec §4.13). The core never panics on input data.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid regex in term `{term}`: {message}")]
    InvalidRegex { term: String, message: String },
    #[error("cancelled")]
    Cancelled,
}

impl CoreError {
    /// `map_err` helper: `file.read(..).map_err(CoreError::io(path))?`.
    pub fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> CoreError {
        let path = path.to_path_buf();
        move |source| CoreError::Io { path, source }
    }
}
```

- [ ] **Step 2: Write the failing index unit tests**

Create `crates/rogg-core/src/index.rs` containing only this test module (it fails to compile until Step 4):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(idx: &LineIndex) -> Vec<Range<u64>> {
        (0..idx.line_count()).map(|i| idx.line_range(i)).collect()
    }

    #[test]
    fn degenerate_shapes() {
        assert_eq!(LineIndex::from_bytes(b"").line_count(), 0);
        let one_newline = LineIndex::from_bytes(b"\n");
        assert_eq!(ranges(&one_newline), vec![0..0]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"\n\n")), vec![0..0, 1..1]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\nbb\nccc")), vec![0..1, 2..4, 5..8]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\nbb\n")), vec![0..1, 2..4]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\r\nb")), vec![0..2, 3..4], "\\r is kept; LogText strips it");
    }

    #[test]
    fn longest_line_ignores_newlines() {
        assert_eq!(LineIndex::from_bytes(b"a\nbb\nccc").longest_line(), 3);
        assert_eq!(LineIndex::from_bytes(b"abcd\n").longest_line(), 4);
        let huge = vec![b'x'; 1 << 20];
        let idx = LineIndex::from_bytes(&huge);
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.longest_line(), 1 << 20);
    }

    #[test]
    fn starts_survive_block_boundaries_and_span_overflow() {
        let data: Vec<u8> = (0..10_000).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
        let idx = LineIndex::from_bytes(&data);
        assert_eq!(idx.line_count(), 10_000);
        assert_eq!(&data[idx.line_range(4096).start as usize..idx.line_range(4096).end as usize], b"line 4096");
        assert_eq!(&data[idx.line_range(9999).start as usize..idx.line_range(9999).end as usize], b"line 9999");
        // A tiny span limit forces a new block every few lines, exercising the u32-overflow path.
        let mut small = LineIndex::with_max_block_span(10);
        small.extend_to(&data);
        assert_eq!(ranges(&small), ranges(&idx));
    }

    #[test]
    fn memory_is_about_four_bytes_per_line() {
        let data: Vec<u8> = (0..100_000).flat_map(|_| b"xy\n".to_vec()).collect();
        let mut idx = LineIndex::from_bytes(&data);
        idx.shrink_to_fit();
        assert!(idx.heap_bytes() <= 100_000 * 4 + 4096, "heap {}", idx.heap_bytes());
    }

    #[test]
    fn parallel_build_matches_serial_and_reports_progress() {
        let data: Vec<u8> = (0..5_000).flat_map(|i| format!("{i}\r\n").into_bytes()).collect();
        let mut waves = Vec::new();
        let idx = build_index(&data, 997, &CancelToken::new(), |i, p| waves.push((i.line_count(), p))).unwrap();
        assert_eq!(idx, LineIndex::from_bytes(&data));
        assert!(!waves.is_empty());
        assert!(waves.windows(2).all(|w| w[0].1.bytes_done <= w[1].1.bytes_done));
        assert_eq!(waves.last().unwrap().1, IndexProgress { bytes_done: data.len() as u64, bytes_total: data.len() as u64 });
    }

    #[test]
    fn cancelled_build_returns_cancelled() {
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(build_index(b"a\nb\n", 1, &cancel, |_, _| {}), Err(CoreError::Cancelled)));
    }
}
```

Add `pub mod index;` to `lib.rs` (already listed above).

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p rogg-core`
Expected: compile errors, `LineIndex` and `build_index` are not found.

- [ ] **Step 4: Implement the index**

Put this above the tests in `index.rs`:
```rust
//! Line index (spec §4.4): the start offset of every line, kept in blocks of up to 4096 lines.
//! Each block holds a u64 base offset and u32 offsets relative to it, so ~4 bytes per line.

use crate::cancel::CancelToken;
use crate::error::CoreError;
use rayon::prelude::*;
use std::ops::Range;

pub const BLOCK_LINES: u64 = 4096;
/// Parallel build chunk size (spec: ~16 MB).
pub const DEFAULT_CHUNK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    block_first_line: Vec<u64>,
    block_base: Vec<u64>,
    rel: Vec<u32>,
    data_len: u64,
    ends_with_newline: bool,
    longest_line: u64,
    max_block_span: u64,
}

impl Default for LineIndex {
    fn default() -> Self {
        Self::with_max_block_span(u32::MAX as u64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexProgress {
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl LineIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Caps the bytes a block may span. Production uses u32::MAX; tests use tiny values to hit the overflow path.
    pub fn with_max_block_span(max_block_span: u64) -> Self {
        Self {
            block_first_line: Vec::new(),
            block_base: Vec::new(),
            rel: Vec::new(),
            data_len: 0,
            ends_with_newline: false,
            longest_line: 0,
            max_block_span,
        }
    }

    /// Serial build (reference implementation; used for small inputs).
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut index = Self::new();
        index.extend_to(data);
        index
    }

    pub fn line_count(&self) -> u64 {
        self.rel.len() as u64
    }

    pub fn data_len(&self) -> u64 {
        self.data_len
    }

    /// Longest line in bytes, excluding its `\n`.
    pub fn longest_line(&self) -> u64 {
        self.longest_line
    }

    /// Byte offset where `line` starts. `line` must be `< line_count()`.
    pub fn line_start(&self, line: u64) -> u64 {
        let block = self.block_first_line.partition_point(|&first| first <= line) - 1;
        self.block_base[block] + self.rel[line as usize] as u64
    }

    /// Bytes of `line` without its `\n` (a trailing `\r` is kept). `line` must be `< line_count()`.
    pub fn line_range(&self, line: u64) -> Range<u64> {
        let start = self.line_start(line);
        let end = if line + 1 < self.line_count() {
            self.line_start(line + 1) - 1
        } else {
            self.data_len - self.ends_with_newline as u64
        };
        start..end
    }

    pub fn heap_bytes(&self) -> usize {
        self.block_first_line.capacity() * 8 + self.block_base.capacity() * 8 + self.rel.capacity() * 4
    }

    pub fn shrink_to_fit(&mut self) {
        self.block_first_line.shrink_to_fit();
        self.block_base.shrink_to_fit();
        self.rel.shrink_to_fit();
    }

    /// Indexes `data[data_len()..]`. `data` must begin with the bytes already indexed (incremental append).
    pub fn extend_to(&mut self, data: &[u8]) {
        let from = self.data_len as usize;
        if data.len() <= from {
            return;
        }
        let newlines: Vec<u64> = memchr::memchr_iter(b'\n', &data[from..]).map(|p| (from + p) as u64).collect();
        self.append_scanned(data.len() as u64, &newlines);
    }

    fn push_start(&mut self, start: u64) {
        let line = self.line_count();
        let new_block = match (self.block_first_line.last(), self.block_base.last()) {
            (Some(&first), Some(&base)) => line - first >= BLOCK_LINES || start - base > self.max_block_span,
            _ => true,
        };
        if new_block {
            self.block_first_line.push(line);
            self.block_base.push(start);
        }
        let base = *self.block_base.last().expect("a block exists after push");
        self.rel.push((start - base) as u32);
    }

    /// Adds the lines of bytes `data_len()..new_len`, given every `\n` position in that range (ascending).
    fn append_scanned(&mut self, new_len: u64, newlines: &[u64]) {
        if new_len <= self.data_len {
            return;
        }
        if self.data_len == 0 || self.ends_with_newline {
            self.push_start(self.data_len);
        }
        for &nl in newlines {
            let last = self.line_count() - 1;
            self.longest_line = self.longest_line.max(nl - self.line_start(last));
            if nl + 1 < new_len {
                self.push_start(nl + 1);
            }
        }
        self.data_len = new_len;
        self.ends_with_newline = newlines.last() == Some(&(new_len - 1));
        if !self.ends_with_newline {
            let last = self.line_count() - 1;
            self.longest_line = self.longest_line.max(new_len - self.line_start(last));
        }
    }
}

/// Builds the index of `data` on all cores. Chunks are scanned for `\n` in parallel waves and stitched
/// in order. `on_wave` sees the growing index after each wave, so a UI can scroll the indexed prefix.
pub fn build_index(
    data: &[u8],
    chunk_bytes: usize,
    cancel: &CancelToken,
    on_wave: impl FnMut(&LineIndex, IndexProgress),
) -> Result<LineIndex, CoreError> {
    build_index_into(LineIndex::new(), data, chunk_bytes, cancel, on_wave)
}

/// Like `build_index`, continuing from an existing (possibly partially built) index.
pub fn build_index_into(
    mut index: LineIndex,
    data: &[u8],
    chunk_bytes: usize,
    cancel: &CancelToken,
    mut on_wave: impl FnMut(&LineIndex, IndexProgress),
) -> Result<LineIndex, CoreError> {
    let chunk = chunk_bytes.max(1);
    let total = data.len() as u64;
    let ranges: Vec<Range<usize>> =
        (index.data_len as usize..data.len()).step_by(chunk).map(|s| s..(s + chunk).min(data.len())).collect();
    let wave = rayon::current_num_threads().max(1) * 2;
    for group in ranges.chunks(wave) {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let scanned: Vec<Vec<u64>> = group
            .par_iter()
            .map(|r| memchr::memchr_iter(b'\n', &data[r.clone()]).map(|p| (r.start + p) as u64).collect())
            .collect();
        for (r, newlines) in group.iter().zip(&scanned) {
            index.append_scanned(r.end as u64, newlines);
        }
        on_wave(&index, IndexProgress { bytes_done: index.data_len, bytes_total: total });
    }
    if cancel.is_cancelled() && ranges.is_empty() {
        return Err(CoreError::Cancelled);
    }
    Ok(index)
}
```

`Cancelled` is returned for an empty-but-cancelled build too, so callers always see cancellation consistently.

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p rogg-core --lib`
Expected: 6 tests pass.

- [ ] **Step 6: Write the property tests**

`crates/rogg-core/tests/index_props.rs`:
```rust
use proptest::prelude::*;
use rogg_core::CancelToken;
use rogg_core::index::{LineIndex, build_index, build_index_into};
use std::ops::Range;

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'x'), Just(b'\n'), Just(b'\r')], 0..400)
}

fn naive_lines(data: &[u8]) -> Vec<Range<u64>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' {
            out.push(start as u64..i as u64);
            start = i + 1;
        }
    }
    if start < data.len() {
        out.push(start as u64..data.len() as u64);
    }
    out
}

proptest! {
    #[test]
    fn serial_matches_naive_split(data in bytes()) {
        let idx = LineIndex::from_bytes(&data);
        let naive = naive_lines(&data);
        prop_assert_eq!(idx.line_count(), naive.len() as u64);
        for (i, r) in naive.iter().enumerate() {
            prop_assert_eq!(idx.line_range(i as u64), r.clone());
        }
        let longest = naive.iter().map(|r| r.end - r.start).max().unwrap_or(0);
        prop_assert_eq!(idx.longest_line(), longest);
    }

    #[test]
    fn parallel_equals_serial(data in bytes(), chunk in 1usize..64, span in prop_oneof![Just(u32::MAX as u64), 1u64..16]) {
        let par = build_index_into(LineIndex::with_max_block_span(span), &data, chunk, &CancelToken::new(), |_, _| {}).unwrap();
        let mut serial = LineIndex::with_max_block_span(span);
        serial.extend_to(&data);
        prop_assert_eq!(par, serial);
    }

    #[test]
    fn incremental_equals_full(data in bytes(), mut cuts in proptest::collection::vec(0usize..400, 0..6)) {
        cuts.sort_unstable();
        let mut idx = LineIndex::new();
        for c in cuts {
            idx.extend_to(&data[..c.min(data.len())]);
        }
        idx.extend_to(&data);
        prop_assert_eq!(idx, LineIndex::from_bytes(&data));
    }

    #[test]
    fn parallel_build_default_entry_point(data in bytes(), chunk in 1usize..64) {
        prop_assert_eq!(build_index(&data, chunk, &CancelToken::new(), |_, _| {}).unwrap(), LineIndex::from_bytes(&data));
    }
}
```

- [ ] **Step 7: Run all rogg-core tests**

Run: `cargo test -p rogg-core`
Expected: unit tests and 4 proptests pass.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock crates/rogg-core
git commit -m "feat(core): rogg-core crate with cancellation, errors and a parallel line index

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Encoding detection, line endings and UTF-16 transcoding

**Files:**
- Create: `crates/rogg-core/src/encoding.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod encoding;`)
- Test: unit tests in `encoding.rs`

**Interfaces:**
- Consumes: `CancelToken`, `CoreError` (Task 1).
- Produces:
  - `encoding::Encoding::{Utf8, Utf8Bom, Utf16Le, Utf16Be}` with `detect(&[u8])`, `bom_len()`, `is_utf16()`, `label()`
  - `encoding::LineEndings::{None, Lf, Crlf, Mixed}` with `detect(&[u8])`, `label()`
  - `encoding::{ENDINGS_SAMPLE_BYTES = 65536, status_label(Encoding, LineEndings) -> String}`
  - `encoding::transcode_to_utf8(src: &Path, dst: &Path, Encoding, &CancelToken, on_progress: FnMut(u64 done, u64 total)) -> Result<(), CoreError>`, which writes `<dst>.partial` and then renames it

- [ ] **Step 1: Write the failing tests**

Create `crates/rogg-core/src/encoding.rs` with:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str, le: bool) -> Vec<u8> {
        let mut out = if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] };
        for u in s.encode_utf16() {
            out.extend(if le { u.to_le_bytes() } else { u.to_be_bytes() });
        }
        out
    }

    #[test]
    fn detects_boms() {
        assert_eq!(Encoding::detect(b"\xEF\xBB\xBFabc"), Encoding::Utf8Bom);
        assert_eq!(Encoding::detect(b"\xFF\xFEa\0"), Encoding::Utf16Le);
        assert_eq!(Encoding::detect(b"\xFE\xFF\0a"), Encoding::Utf16Be);
        assert_eq!(Encoding::detect(b"plain"), Encoding::Utf8);
        assert_eq!(Encoding::detect(b""), Encoding::Utf8);
        assert_eq!(Encoding::detect(b"\xFF"), Encoding::Utf8, "a lone 0xFF is not a BOM");
        assert_eq!([Encoding::Utf8.bom_len(), Encoding::Utf8Bom.bom_len(), Encoding::Utf16Le.bom_len()], [0, 3, 2]);
    }

    #[test]
    fn detects_line_endings() {
        assert_eq!(LineEndings::detect(b"a\nb\n"), LineEndings::Lf);
        assert_eq!(LineEndings::detect(b"a\r\nb\r\n"), LineEndings::Crlf);
        assert_eq!(LineEndings::detect(b"a\r\nb\n"), LineEndings::Mixed);
        assert_eq!(LineEndings::detect(b"no breaks\r"), LineEndings::None);
        assert_eq!(LineEndings::detect(b"\n"), LineEndings::Lf);
    }

    #[test]
    fn status_labels_match_spec() {
        assert_eq!(status_label(Encoding::Utf8, LineEndings::Lf), "UTF-8 · LF");
        assert_eq!(status_label(Encoding::Utf8Bom, LineEndings::Lf), "UTF-8 · LF");
        assert_eq!(status_label(Encoding::Utf16Le, LineEndings::Crlf), "UTF-16LE → UTF-8 · CRLF");
        assert_eq!(status_label(Encoding::Utf16Be, LineEndings::Mixed), "UTF-16BE → UTF-8 · mixed line endings");
    }

    #[test]
    fn transcodes_le_and_be_with_progress() {
        let dir = tempfile::tempdir().unwrap();
        for le in [true, false] {
            let src = dir.path().join(if le { "le.log" } else { "be.log" });
            let dst = dir.path().join(if le { "le.utf8" } else { "be.utf8" });
            std::fs::write(&src, utf16("héllo\r\nwörld ✓\n", le)).unwrap();
            let enc = Encoding::detect(&std::fs::read(&src).unwrap());
            let mut last = (0, 0);
            transcode_to_utf8(&src, &dst, enc, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
            assert_eq!(std::fs::read_to_string(&dst).unwrap(), "héllo\r\nwörld ✓\n");
            let total = std::fs::metadata(&src).unwrap().len();
            assert_eq!(last, (total, total));
            assert!(!partial_path(&dst).exists());
        }
    }

    #[test]
    fn odd_trailing_byte_becomes_replacement_char() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("odd.log");
        let dst = dir.path().join("odd.utf8");
        let mut bytes = utf16("ab", true);
        bytes.push(0x41);
        std::fs::write(&src, bytes).unwrap();
        transcode_to_utf8(&src, &dst, Encoding::Utf16Le, &CancelToken::new(), |_, _| {}).unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "ab\u{FFFD}");
    }

    #[test]
    fn cancel_leaves_no_output() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("c.log");
        let dst = dir.path().join("c.utf8");
        std::fs::write(&src, utf16("abc\n", true)).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(transcode_to_utf8(&src, &dst, Encoding::Utf16Le, &cancel, |_, _| {}), Err(CoreError::Cancelled)));
        assert!(!dst.exists());
        assert!(!partial_path(&dst).exists());
    }
}
```
Add `tempfile` usage only under tests (already a dev-dependency). Add `pub mod encoding;` to `lib.rs`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core encoding`
Expected: compile errors, `Encoding`, `LineEndings` and `transcode_to_utf8` are not found.

- [ ] **Step 3: Implement**

Above the tests:
```rust
//! Encoding detection, line-ending detection and UTF-16 → UTF-8 transcoding (spec §4.3).

use crate::cancel::CancelToken;
use crate::error::CoreError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

impl Encoding {
    pub fn detect(head: &[u8]) -> Self {
        match head {
            [0xEF, 0xBB, 0xBF, ..] => Encoding::Utf8Bom,
            [0xFF, 0xFE, ..] => Encoding::Utf16Le,
            [0xFE, 0xFF, ..] => Encoding::Utf16Be,
            _ => Encoding::Utf8,
        }
    }

    pub fn bom_len(self) -> usize {
        match self {
            Encoding::Utf8 => 0,
            Encoding::Utf8Bom => 3,
            Encoding::Utf16Le | Encoding::Utf16Be => 2,
        }
    }

    pub fn is_utf16(self) -> bool {
        matches!(self, Encoding::Utf16Le | Encoding::Utf16Be)
    }

    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 | Encoding::Utf8Bom => "UTF-8",
            Encoding::Utf16Le => "UTF-16LE → UTF-8",
            Encoding::Utf16Be => "UTF-16BE → UTF-8",
        }
    }

    fn decoder(self) -> Option<encoding_rs::Decoder> {
        match self {
            Encoding::Utf16Le => Some(encoding_rs::UTF_16LE.new_decoder_without_bom_handling()),
            Encoding::Utf16Be => Some(encoding_rs::UTF_16BE.new_decoder_without_bom_handling()),
            _ => None,
        }
    }

    /// Decodes in-memory UTF-16 (BOM included) to UTF-8, replacing malformed units with U+FFFD.
    pub(crate) fn decode_utf16(self, bytes: &[u8]) -> String {
        let body = bytes.get(2..).unwrap_or(&[]);
        let codec = if self == Encoding::Utf16Le { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
        codec.decode_without_bom_handling(body).0.into_owned()
    }
}

/// Bytes sampled for line-ending detection (spec: first 64 KB).
pub const ENDINGS_SAMPLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LineEndings {
    None,
    Lf,
    Crlf,
    Mixed,
}

impl LineEndings {
    pub fn detect(sample: &[u8]) -> Self {
        let (mut lf, mut crlf) = (0usize, 0usize);
        for p in memchr::memchr_iter(b'\n', sample) {
            if p > 0 && sample[p - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
        match (lf, crlf) {
            (0, 0) => LineEndings::None,
            (_, 0) => LineEndings::Lf,
            (0, _) => LineEndings::Crlf,
            _ => LineEndings::Mixed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LineEndings::None => "no line breaks",
            LineEndings::Lf => "LF",
            LineEndings::Crlf => "CRLF",
            LineEndings::Mixed => "mixed line endings",
        }
    }
}

/// Status bar text, e.g. `UTF-8 · LF` or `UTF-16LE → UTF-8 · CRLF`.
pub fn status_label(encoding: Encoding, endings: LineEndings) -> String {
    format!("{} · {}", encoding.label(), endings.label())
}

pub(crate) fn partial_path(dst: &Path) -> PathBuf {
    let mut name = dst.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

/// Streams a UTF-16 file (BOM included) into UTF-8 at `dst`. Output goes to `<dst>.partial` and is renamed
/// on success, so cancellation or a crash never leaves a truncated `dst` (spec §4.2 atomic writes).
pub fn transcode_to_utf8(
    src: &Path,
    dst: &Path,
    encoding: Encoding,
    cancel: &CancelToken,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), CoreError> {
    let mut decoder = encoding.decoder().expect("transcode_to_utf8 requires a UTF-16 encoding");
    let partial = partial_path(dst);
    let result = (|| -> Result<(), CoreError> {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let mut input = File::open(src).map_err(CoreError::io(src))?;
        let total = input.metadata().map_err(CoreError::io(src))?.len();
        let mut out = BufWriter::new(File::create(&partial).map_err(CoreError::io(&partial))?);
        let mut bom = [0u8; 2];
        input.read_exact(&mut bom).map_err(CoreError::io(src))?;
        let mut done = 2u64;
        let mut buf = vec![0u8; 1 << 20];
        let mut utf8 = vec![0u8; decoder.max_utf8_buffer_length(buf.len()).unwrap_or(4 << 20)];
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let n = input.read(&mut buf).map_err(CoreError::io(src))?;
            let last = n == 0;
            let mut consumed = 0;
            loop {
                let (result, read, written, _) = decoder.decode_to_utf8(&buf[consumed..n], &mut utf8, last);
                out.write_all(&utf8[..written]).map_err(CoreError::io(&partial))?;
                consumed += read;
                if matches!(result, encoding_rs::CoderResult::InputEmpty) {
                    break;
                }
            }
            done += n as u64;
            on_progress(done.min(total), total);
            if last {
                break;
            }
        }
        out.flush().map_err(CoreError::io(&partial))?;
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&partial, dst).map_err(CoreError::io(dst)),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-core encoding`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): encoding and line-ending detection with atomic UTF-16 transcoding

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: LogText — opened, indexed log text

**Files:**
- Create: `crates/rogg-core/src/text.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod text;`)
- Test: `crates/rogg-core/tests/text.rs`

**Interfaces:**
- Consumes: `LineIndex`, `build_index`, `DEFAULT_CHUNK_BYTES` (Task 1); `Encoding`, `LineEndings`, `status_label`, `transcode_to_utf8`, `ENDINGS_SAMPLE_BYTES` (Task 2).
- Produces:
  - `text::OpenProgress::{Transcoding { done, total }, Indexing { done, total }}`
  - `text::LogText` (Send + Sync):
    - constructors: `open(path, cache_dir, &CancelToken, on_progress: FnMut(OpenProgress)) -> Result<LogText, CoreError>`, `from_bytes(Vec<u8>) -> LogText`
    - lines: `line_count() -> u64`, `line(u64) -> &[u8]` (no `\n`, no trailing `\r`, empty if out of range), `line_text(u64) -> Cow<str>` (lossy)
    - raw access: `bytes() -> &[u8]` (text after any UTF-8 BOM), `index() -> &LineIndex`, `span(Range<u64>) -> (&[u8], u64)` (contiguous bytes covering those lines, plus the offset of their first byte within `bytes()`)
    - metadata: `encoding()`, `endings()`, `status_label() -> String`, `source_path() -> Option<&Path>`

`open` memory-maps the file, or the transcoded copy in `cache_dir` for UTF-16. The cache file is named by a hash of (canonical path, size, mtime), so reopening an unchanged UTF-16 file reuses it. The full cache with eviction is M3. Empty files use an owned empty buffer, because mapping zero bytes fails on macOS.

**Safety note for the implementer:** the mapping assumes the file is not truncated while mapped. Live follow and truncation handling (spec §4.5) are M3 and will re-map on change. Document this on `LogText`.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-core/tests/text.rs`:
```rust
use rogg_core::CancelToken;
use rogg_core::encoding::{Encoding, LineEndings};
use rogg_core::text::{LogText, OpenProgress};
use std::path::Path;

fn open(path: &Path, cache: &Path) -> LogText {
    LogText::open(path, cache, &CancelToken::new(), |_| {}).unwrap()
}

fn lines(t: &LogText) -> Vec<String> {
    (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()
}

#[test]
fn plain_file_lines_and_labels() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.log");
    std::fs::write(&p, "one\ntwo\nthree").unwrap();
    let t = open(&p, &dir.path().join("cache"));
    assert_eq!(lines(&t), ["one", "two", "three"]);
    assert_eq!(t.status_label(), "UTF-8 · LF");
    assert_eq!(t.source_path(), Some(p.as_path()));
    assert_eq!(t.line(99), b"", "out of range is empty, not a panic");
}

#[test]
fn degenerate_files() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    for (name, content, expected) in [("empty", "", vec![]), ("nl", "\n", vec![""]), ("two", "\n\n", vec!["", ""])] {
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        assert_eq!(lines(&open(&p, &cache)), expected, "{name}");
    }
}

#[test]
fn crlf_is_stripped_from_lines_and_endings_reported() {
    let t = LogText::from_bytes(b"error\r\nok\r\nlast\r".to_vec());
    assert_eq!(t.line(0), b"error");
    assert_eq!(t.line(2), b"last");
    assert_eq!(t.endings(), LineEndings::Crlf);
    assert_eq!(LogText::from_bytes(b"a\r\nb\n".to_vec()).endings(), LineEndings::Mixed);
}

#[test]
fn utf8_bom_is_not_part_of_line_one() {
    let t = LogText::from_bytes(b"\xEF\xBB\xBFfirst\nsecond".to_vec());
    assert_eq!(t.line(0), b"first");
    assert_eq!(t.encoding(), Encoding::Utf8Bom);
    assert_eq!(t.bytes(), b"first\nsecond");
}

#[test]
fn utf16_file_is_transcoded_into_cache_and_reused() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let p = dir.path().join("w.log");
    let mut bytes = vec![0xFF, 0xFE];
    for u in "héllo\r\nwörld\r\n".encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    std::fs::write(&p, bytes).unwrap();
    let mut saw_transcode = false;
    let t = LogText::open(&p, &cache, &CancelToken::new(), |e| saw_transcode |= matches!(e, OpenProgress::Transcoding { .. })).unwrap();
    assert!(saw_transcode);
    assert_eq!(lines(&t), ["héllo", "wörld"]);
    assert_eq!(t.status_label(), "UTF-16LE → UTF-8 · CRLF");
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
    let mut transcoded_again = false;
    let again = LogText::open(&p, &cache, &CancelToken::new(), |e| transcoded_again |= matches!(e, OpenProgress::Transcoding { .. })).unwrap();
    assert!(!transcoded_again, "unchanged file reuses the cached transcode");
    assert_eq!(lines(&again), ["héllo", "wörld"]);
}

#[test]
fn invalid_utf8_is_lossy_for_display_but_raw_for_matching() {
    let t = LogText::from_bytes(b"ok \xFF\xFE error\n".to_vec());
    assert_eq!(t.line(0), b"ok \xFF\xFE error");
    assert_eq!(t.line_text(0), "ok \u{FFFD}\u{FFFD} error");
}

#[test]
fn span_covers_contiguous_lines() {
    let t = LogText::from_bytes(b"aa\nbbb\ncc\n".to_vec());
    let (bytes, base) = t.span(1..3);
    assert_eq!(bytes, b"bbb\ncc");
    assert_eq!(base, 3);
    assert_eq!(t.span(2..2), (&b""[..], 0));
}

#[test]
fn indexing_progress_reaches_total_and_cancel_errors() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("p.log");
    std::fs::write(&p, "x\n".repeat(10_000)).unwrap();
    let mut last = None;
    LogText::open(&p, dir.path(), &CancelToken::new(), |e| last = Some(e)).unwrap();
    assert_eq!(last, Some(OpenProgress::Indexing { done: 20_000, total: 20_000 }));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(LogText::open(&p, dir.path(), &cancel, |_| {}).is_err());
}

#[test]
fn missing_file_is_a_typed_io_error() {
    let err = LogText::open(Path::new("/definitely/not/here.log"), Path::new("/tmp"), &CancelToken::new(), |_| {}).err().unwrap();
    assert!(matches!(err, rogg_core::CoreError::Io { .. }));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core --test text`
Expected: compile error, `rogg_core::text` is not found.

- [ ] **Step 3: Implement `text.rs`**

```rust
//! Opened log text: UTF-8 bytes plus the line index (spec §4.1 plain files, §4.3, §4.4).
//!
//! Files are memory-mapped. The mapping assumes the file is not truncated while open; live follow and
//! truncation handling (spec §4.5, milestone M3) will re-map on change.

use crate::cancel::CancelToken;
use crate::encoding::{ENDINGS_SAMPLE_BYTES, Encoding, LineEndings, status_label, transcode_to_utf8};
use crate::error::CoreError;
use crate::index::{DEFAULT_CHUNK_BYTES, IndexProgress, LineIndex, build_index};
use memmap2::Mmap;
use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};

enum Data {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Mapped(m) => m,
            Data::Owned(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenProgress {
    Transcoding { done: u64, total: u64 },
    Indexing { done: u64, total: u64 },
}

/// A log's UTF-8 text and line index. Line content excludes `\n` and one trailing `\r`.
pub struct LogText {
    data: Data,
    start: usize,
    index: LineIndex,
    encoding: Encoding,
    endings: LineEndings,
    source_path: Option<PathBuf>,
}

fn map_file(path: &Path) -> Result<Data, CoreError> {
    let file = File::open(path).map_err(CoreError::io(path))?;
    let len = file.metadata().map_err(CoreError::io(path))?.len();
    if len == 0 {
        return Ok(Data::Owned(Vec::new()));
    }
    // SAFETY: the file is opened read-only; rogg does not write to it. External truncation while mapped is
    // handled by re-mapping in M3 (live follow), see module docs.
    let map = unsafe { Mmap::map(&file) }.map_err(CoreError::io(path))?;
    Ok(Data::Mapped(map))
}

fn fnv1a(parts: &[&[u8]]) -> u64 {
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

fn transcoded_path(cache_dir: &Path, path: &Path, meta: &std::fs::Metadata) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let key = fnv1a(&[canonical.as_os_str().as_encoded_bytes(), &meta.len().to_le_bytes(), &mtime.to_le_bytes()]);
    cache_dir.join(format!("{key:016x}.utf8"))
}

impl LogText {
    /// Opens a plain file: detects the encoding, transcodes UTF-16 into `cache_dir`, memory-maps and indexes it.
    pub fn open(
        path: &Path,
        cache_dir: &Path,
        cancel: &CancelToken,
        mut on_progress: impl FnMut(OpenProgress),
    ) -> Result<LogText, CoreError> {
        let mut file = File::open(path).map_err(CoreError::io(path))?;
        let meta = file.metadata().map_err(CoreError::io(path))?;
        let mut head = [0u8; 4];
        let mut head_len = 0;
        while head_len < head.len() {
            let n = file.read(&mut head[head_len..]).map_err(CoreError::io(path))?;
            if n == 0 {
                break;
            }
            head_len += n;
        }
        let encoding = Encoding::detect(&head[..head_len]);
        let (data, start) = if encoding.is_utf16() {
            let dst = transcoded_path(cache_dir, path, &meta);
            if !dst.exists() {
                std::fs::create_dir_all(cache_dir).map_err(CoreError::io(cache_dir))?;
                transcode_to_utf8(path, &dst, encoding, cancel, |done, total| {
                    on_progress(OpenProgress::Transcoding { done, total })
                })?;
            }
            (map_file(&dst)?, 0)
        } else {
            (map_file(path)?, encoding.bom_len())
        };
        let text = &data.bytes()[start.min(data.bytes().len())..];
        let total = text.len() as u64;
        let mut index = build_index(text, DEFAULT_CHUNK_BYTES, cancel, |_, p: IndexProgress| {
            on_progress(OpenProgress::Indexing { done: p.bytes_done, total })
        })?;
        index.shrink_to_fit();
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        Ok(LogText { data, start, index, encoding, endings, source_path: Some(path.to_path_buf()) })
    }

    /// Builds text from memory (tests, small inputs, stdin buffers). UTF-16 is decoded; a UTF-8 BOM is skipped.
    pub fn from_bytes(bytes: Vec<u8>) -> LogText {
        let encoding = Encoding::detect(&bytes);
        let (data, start) = if encoding.is_utf16() {
            (Data::Owned(encoding.decode_utf16(&bytes).into_bytes()), 0)
        } else {
            let start = encoding.bom_len();
            (Data::Owned(bytes), start)
        };
        let text = &data.bytes()[start..];
        let index = LineIndex::from_bytes(text);
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        LogText { data, start, index, encoding, endings, source_path: None }
    }

    /// UTF-8 text bytes (after any BOM).
    pub fn bytes(&self) -> &[u8] {
        &self.data.bytes()[self.start..]
    }

    pub fn index(&self) -> &LineIndex {
        &self.index
    }

    pub fn line_count(&self) -> u64 {
        self.index.line_count()
    }

    /// Line content without `\n` and one trailing `\r`; empty when `i` is out of range.
    pub fn line(&self, i: u64) -> &[u8] {
        if i >= self.line_count() {
            return &[];
        }
        let r = self.index.line_range(i);
        let line = &self.bytes()[r.start as usize..r.end as usize];
        line.strip_suffix(b"\r").unwrap_or(line)
    }

    /// Display text (invalid UTF-8 becomes U+FFFD).
    pub fn line_text(&self, i: u64) -> Cow<'_, str> {
        String::from_utf8_lossy(self.line(i))
    }

    /// Contiguous bytes covering `lines` and the offset of their first byte within `bytes()`.
    pub fn span(&self, lines: Range<u64>) -> (&[u8], u64) {
        let end_line = lines.end.min(self.line_count());
        if lines.start >= end_line {
            return (&[], 0);
        }
        let start = self.index.line_start(lines.start);
        let end = self.index.line_range(end_line - 1).end;
        (&self.bytes()[start as usize..end as usize], start)
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub fn endings(&self) -> LineEndings {
        self.endings
    }

    pub fn status_label(&self) -> String {
        status_label(self.encoding, self.endings)
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}
```
Add `pub mod text;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-core --test text`
Expected: 9 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): LogText opens, maps, transcodes and indexes plain files

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 4: Filter expression model

**Files:**
- Create: `crates/rogg-core/src/filter/mod.rs`, `crates/rogg-core/src/filter/spec.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod filter;`)
- Test: unit tests in `spec.rs`

**Interfaces:**
- Produces (re-exported from `rogg_core::filter`):
  - `TermState::{Include, Off, Exclude}` with `next()` (Include → Off → Exclude → Include)
  - `Term { text: String, state: TermState }` with `Term::include(impl Into<String>)` and `Term::new(text, state)`
  - `FilterSpec { terms: Vec<Term>, regex: bool, case_sensitive: bool, context: u32 }` (Default, Serialize, Deserialize, Hash, Eq), with:
    - `from_expression(&str)`, `with_expression(&self, &str) -> FilterSpec` (keeps the states of unchanged terms), `expression() -> String`
    - `cycle(usize)`, `set_context(u32)` (clamped to `MAX_CONTEXT = 10`)
    - `counts() -> TermCounts { include, exclude, off }`, `summary() -> String` (`"2 include · 1 exclude · 1 off"`), `is_active() -> bool`
  - `split_expression(&str) -> Vec<String>`, `join_expression(&[Term]) -> String`

- [ ] **Step 1: Write the failing tests**

`crates/rogg-core/src/filter/spec.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_double_pipe_trims_and_drops_empty() {
        assert_eq!(split_expression("error || timeout ||  healthcheck "), ["error", "timeout", "healthcheck"]);
        assert_eq!(split_expression(" || a ||  || b ||"), ["a", "b"]);
        assert_eq!(split_expression(""), Vec::<String>::new());
        assert_eq!(split_expression("a|b || c"), ["a|b", "c"], "a single pipe is literal");
    }

    #[test]
    fn escaped_double_pipe_is_literal() {
        assert_eq!(split_expression(r"a \|\| b || c"), ["a || b", "c"]);
        assert_eq!(split_expression(r"x\|\|y"), ["x||y"]);
        let terms = vec![Term::include("a || b"), Term::include("c")];
        assert_eq!(join_expression(&terms), r"a \|\| b || c");
        assert_eq!(split_expression(&join_expression(&terms)), ["a || b", "c"]);
    }

    #[test]
    fn editing_keeps_states_of_unchanged_terms() {
        let mut spec = FilterSpec::from_expression("error || timeout || healthcheck || retry");
        spec.cycle(2);
        spec.cycle(2); // healthcheck: Include → Off → Exclude
        spec.cycle(3); // retry: Off
        let edited = spec.with_expression("timeout || healthcheck || connection reset");
        assert_eq!(edited.terms, vec![
            Term::new("timeout", TermState::Include),
            Term::new("healthcheck", TermState::Exclude),
            Term::new("connection reset", TermState::Include),
        ]);
        assert_eq!(edited.regex, spec.regex);
    }

    #[test]
    fn state_cycle_counts_and_summary() {
        assert_eq!(TermState::Include.next(), TermState::Off);
        assert_eq!(TermState::Off.next(), TermState::Exclude);
        assert_eq!(TermState::Exclude.next(), TermState::Include);
        let mut spec = FilterSpec::from_expression("error || timeout || healthcheck || retry");
        spec.terms[2].state = TermState::Exclude;
        spec.terms[3].state = TermState::Off;
        assert_eq!(spec.counts(), TermCounts { include: 2, exclude: 1, off: 1 });
        assert_eq!(spec.summary(), "2 include · 1 exclude · 1 off");
        assert!(spec.is_active());
        spec.cycle(99); // out of range is ignored
        let all_off = FilterSpec { terms: vec![Term::new("a", TermState::Off)], ..Default::default() };
        assert!(!all_off.is_active());
    }

    #[test]
    fn context_is_clamped() {
        let mut spec = FilterSpec::default();
        spec.set_context(25);
        assert_eq!(spec.context, MAX_CONTEXT);
    }

    #[test]
    fn serde_round_trip() {
        let spec = FilterSpec { regex: true, case_sensitive: true, context: 2, ..FilterSpec::from_expression("a || b") };
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<FilterSpec>(&json).unwrap(), spec);
    }
}
```
`crates/rogg-core/src/filter/mod.rs`:
```rust
//! Filter expressions (spec §4.6) and compiled matchers (spec §4.8).
mod spec;

pub use spec::{FilterSpec, MAX_CONTEXT, Term, TermCounts, TermState, join_expression, split_expression};
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core filter`
Expected: compile errors for the missing items.

- [ ] **Step 3: Implement `spec.rs`**

Above the tests:
```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum TermState {
    Include,
    Off,
    Exclude,
}

impl TermState {
    /// Include → Off → Exclude → Include (spec §4.6).
    pub fn next(self) -> Self {
        match self {
            TermState::Include => TermState::Off,
            TermState::Off => TermState::Exclude,
            TermState::Exclude => TermState::Include,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Term {
    pub text: String,
    pub state: TermState,
}

impl Term {
    pub fn new(text: impl Into<String>, state: TermState) -> Self {
        Self { text: text.into(), state }
    }

    pub fn include(text: impl Into<String>) -> Self {
        Self::new(text, TermState::Include)
    }
}

/// Largest context radius (spec §4.7: UI presets ±3, up to 10 via settings).
pub const MAX_CONTEXT: u32 = 10;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct FilterSpec {
    pub terms: Vec<Term>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TermCounts {
    pub include: usize,
    pub exclude: usize,
    pub off: usize,
}

/// Splits on `||`, honouring `\|\|` as a literal `||`; trims terms and drops empty ones (spec §4.6).
pub fn split_expression(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("\\|\\|") {
            current.push_str("||");
            rest = r;
        } else if let Some(r) = rest.strip_prefix("||") {
            terms.push(std::mem::take(&mut current));
            rest = r;
        } else {
            current.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    terms.push(current);
    terms.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

/// Inverse of `split_expression` for display in the expression editor.
pub fn join_expression(terms: &[Term]) -> String {
    terms.iter().map(|t| t.text.replace("||", "\\|\\|")).collect::<Vec<_>>().join(" || ")
}

impl FilterSpec {
    pub fn from_expression(text: &str) -> Self {
        Self { terms: split_expression(text).into_iter().map(Term::include).collect(), ..Self::default() }
    }

    /// Re-parses an edited expression; a term whose text is unchanged keeps its state, new terms start as Include.
    pub fn with_expression(&self, text: &str) -> Self {
        let terms = split_expression(text)
            .into_iter()
            .map(|t| {
                let state = self.terms.iter().find(|old| old.text == t).map(|old| old.state).unwrap_or(TermState::Include);
                Term { text: t, state }
            })
            .collect();
        Self { terms, ..self.clone() }
    }

    pub fn expression(&self) -> String {
        join_expression(&self.terms)
    }

    pub fn cycle(&mut self, index: usize) {
        if let Some(term) = self.terms.get_mut(index) {
            term.state = term.state.next();
        }
    }

    pub fn set_context(&mut self, context: u32) {
        self.context = context.min(MAX_CONTEXT);
    }

    pub fn counts(&self) -> TermCounts {
        let mut c = TermCounts::default();
        for t in &self.terms {
            match t.state {
                TermState::Include => c.include += 1,
                TermState::Exclude => c.exclude += 1,
                TermState::Off => c.off += 1,
            }
        }
        c
    }

    pub fn summary(&self) -> String {
        let c = self.counts();
        format!("{} include · {} exclude · {} off", c.include, c.exclude, c.off)
    }

    /// True when at least one term is Include or Exclude.
    pub fn is_active(&self) -> bool {
        self.terms.iter().any(|t| t.state != TermState::Off)
    }
}
```
Add `pub mod filter;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-core filter`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): filter expression model with escaping, states and edit retention

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Compiled filter and chunk scanning

**Files:**
- Create: `crates/rogg-core/src/filter/compiled.rs`
- Modify: `crates/rogg-core/src/filter/mod.rs`
- Test: unit tests in `compiled.rs`; `crates/rogg-core/tests/filter_props.rs`

**Interfaces:**
- Consumes: `FilterSpec`, `TermState` (Task 4); `LogText` (`line`, `span`, `index().line_start`) (Task 3).
- Produces:
  - `filter::CompiledFilter`, with:
    - `compile(&FilterSpec) -> Result<CompiledFilter, CoreError>` (`InvalidRegex { term, .. }` names the failing term)
    - `spec() -> &FilterSpec`, `is_active() -> bool`
    - `line_matches(&[u8]) -> bool` (full §4.7 semantics on one line), `is_excluded(&[u8]) -> bool`
    - `scan(&LogText, Range<u64>) -> ScanResult`
  - `filter::ScanResult { matches: Vec<u64>, term_hits: Vec<u64> }`. `term_hits[i]` is the number of scanned lines containing term `i`; it is 0 for Off terms, and each line counts once per term.

How it works:
- Plain mode: one Aho-Corasick automaton (`MatchKind::Standard`, ASCII case folding unless case-sensitive). It runs overlapping over the chunk's contiguous bytes, and each match is mapped to its line by binary search over the chunk's line starts.
- Regex mode: one `RegexSet` checked per line, on the `\r`-stripped content (`is_match` first, then `matches` only on hit lines, for per-term counts).
- Exclude terms are compiled the same way. Exclusion follows §4.7 exactly.
- Plain terms cannot contain `\n`, so a byte-level match never spans lines.

- [ ] **Step 1: Write the failing unit tests**

`crates/rogg-core/src/filter/compiled.rs` (tests first):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::text::LogText;

    fn text(lines: &[&str]) -> LogText {
        LogText::from_bytes(lines.join("\n").into_bytes())
    }

    fn spec(terms: &[(&str, TermState)]) -> FilterSpec {
        FilterSpec { terms: terms.iter().map(|(t, s)| Term::new(*t, *s)).collect(), ..Default::default() }
    }

    fn scan_all(spec: &FilterSpec, t: &LogText) -> ScanResult {
        CompiledFilter::compile(spec).unwrap().scan(t, 0..t.line_count())
    }

    use TermState::{Exclude, Include, Off};

    #[test]
    fn semantics_follow_spec_4_7() {
        let t = text(&["error a", "timeout b", "error healthcheck", "ok", "retry"]);
        // include any, minus excludes
        assert_eq!(scan_all(&spec(&[("error", Include), ("timeout", Include), ("healthcheck", Exclude)]), &t).matches, vec![0, 1]);
        // only excludes: everything not excluded
        assert_eq!(scan_all(&spec(&[("healthcheck", Exclude)]), &t).matches, vec![0, 1, 3, 4]);
        // no active terms: nothing
        assert_eq!(scan_all(&spec(&[("error", Off)]), &t).matches, Vec::<u64>::new());
        assert_eq!(scan_all(&FilterSpec::default(), &t).matches, Vec::<u64>::new());
        // off terms are ignored entirely
        assert_eq!(scan_all(&spec(&[("error", Include), ("retry", Off)]), &t).matches, vec![0, 2]);
    }

    #[test]
    fn case_handling() {
        let t = text(&["ERROR one", "error two", "Error three"]);
        assert_eq!(scan_all(&spec(&[("error", Include)]), &t).matches, vec![0, 1, 2], "case-insensitive by default");
        let cs = FilterSpec { case_sensitive: true, ..spec(&[("error", Include)]) };
        assert_eq!(scan_all(&cs, &t).matches, vec![1]);
        let re = FilterSpec { regex: true, ..spec(&[("^error", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 1, 2]);
    }

    #[test]
    fn regex_terms_and_errors() {
        let t = text(&["request_id=req_8f29", "request_id=x", "duration=30241ms"]);
        let re = FilterSpec { regex: true, ..spec(&[(r"req_[0-9a-f]{4}", Include), (r"duration=\d{4,}ms", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 2]);
        let bad = FilterSpec { regex: true, ..spec(&[("ok", Include), ("(unclosed", Include)]) };
        match CompiledFilter::compile(&bad) {
            Err(CoreError::InvalidRegex { term, .. }) => assert_eq!(term, "(unclosed"),
            other => panic!("expected InvalidRegex, got {:?}", other.err()),
        }
    }

    #[test]
    fn empty_matching_regex_matches_every_line() {
        let t = text(&["a", "", "xyz"]);
        let re = FilterSpec { regex: true, ..spec(&[("x*", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 1, 2]);
    }

    #[test]
    fn trailing_cr_is_never_matched() {
        let t = LogText::from_bytes(b"error\r\nok\r\n".to_vec());
        let re = FilterSpec { regex: true, ..spec(&[("error$", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0]);
        let plain_cr = spec(&[("ok", Include)]);
        assert_eq!(scan_all(&plain_cr, &t).matches, vec![1]);
    }

    #[test]
    fn per_term_hits_count_lines_once_and_include_overlaps() {
        let t = text(&["timeout timeout", "time", "error timeout", "ok"]);
        let s = spec(&[("time", Include), ("timeout", Include), ("error", Exclude), ("ok", Off)]);
        let r = scan_all(&s, &t);
        assert_eq!(r.term_hits, vec![3, 2, 1, 0]);
        assert_eq!(r.matches, vec![0, 1]);
    }

    #[test]
    fn invalid_utf8_lines_still_match_raw_bytes() {
        let t = LogText::from_bytes(b"\xFF\xFE error\nok\n".to_vec());
        assert_eq!(scan_all(&spec(&[("error", Include)]), &t).matches, vec![0]);
    }

    #[test]
    fn partial_ranges_and_many_terms() {
        let lines: Vec<String> = (0..300).map(|i| format!("line {i} term{}", i % 200)).collect();
        let t = LogText::from_bytes(lines.join("\n").into_bytes());
        let terms: Vec<Term> = (0..200).map(|i| Term::include(format!("term{i}"))).collect();
        let compiled = CompiledFilter::compile(&FilterSpec { terms, ..Default::default() }).unwrap();
        let r = compiled.scan(&t, 100..150);
        assert_eq!(r.matches, (100..150).collect::<Vec<u64>>());
        // A line containing `term1XY` also contains the terms `term1` and `term1X` (overlapping matches count).
        assert_eq!(r.term_hits[1], 50);
        assert_eq!(r.term_hits[10], 10);
        assert_eq!(r.term_hits[123], 1);
        assert_eq!(r.term_hits.iter().sum::<u64>(), 150);
    }

    #[test]
    fn line_matches_agrees_with_scan() {
        let t = text(&["error a", "timeout b", "error healthcheck", "ok"]);
        let s = spec(&[("error", Include), ("healthcheck", Exclude)]);
        let c = CompiledFilter::compile(&s).unwrap();
        let by_line: Vec<u64> = (0..t.line_count()).filter(|&i| c.line_matches(t.line(i))).collect();
        assert_eq!(by_line, c.scan(&t, 0..t.line_count()).matches);
        assert!(c.is_excluded(b"x healthcheck"));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core compiled`
Expected: compile errors, `CompiledFilter` and `ScanResult` are not found.

- [ ] **Step 3: Implement `compiled.rs`**

Above the tests:
```rust
use super::spec::{FilterSpec, TermState};
use crate::error::CoreError;
use crate::text::LogText;
use aho_corasick::{AhoCorasick, MatchKind};
use regex::bytes::{RegexBuilder, RegexSet, RegexSetBuilder};
use std::ops::Range;

/// Lines matched in a scanned range plus per-term line hit counts (indexed like `FilterSpec::terms`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub matches: Vec<u64>,
    pub term_hits: Vec<u64>,
}

enum TermSet {
    Empty,
    Plain { ac: AhoCorasick, terms: Vec<usize> },
    Regex { set: RegexSet, terms: Vec<usize> },
}

impl TermSet {
    fn build(spec: &FilterSpec, state: TermState) -> Result<Self, CoreError> {
        let terms: Vec<usize> =
            spec.terms.iter().enumerate().filter(|(_, t)| t.state == state).map(|(i, _)| i).collect();
        if terms.is_empty() {
            return Ok(TermSet::Empty);
        }
        let patterns: Vec<&str> = terms.iter().map(|&i| spec.terms[i].text.as_str()).collect();
        if spec.regex {
            for pattern in &patterns {
                RegexBuilder::new(pattern)
                    .case_insensitive(!spec.case_sensitive)
                    .build()
                    .map_err(|e| CoreError::InvalidRegex { term: pattern.to_string(), message: e.to_string() })?;
            }
            let set = RegexSetBuilder::new(&patterns)
                .case_insensitive(!spec.case_sensitive)
                .build()
                .map_err(|e| CoreError::InvalidRegex { term: patterns.join(" || "), message: e.to_string() })?;
            Ok(TermSet::Regex { set, terms })
        } else {
            let ac = AhoCorasick::builder()
                .ascii_case_insensitive(!spec.case_sensitive)
                .match_kind(MatchKind::Standard)
                .build(&patterns)
                .map_err(|e| CoreError::InvalidRegex { term: patterns.join(" || "), message: e.to_string() })?;
            Ok(TermSet::Plain { ac, terms })
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, TermSet::Empty)
    }

    fn is_match(&self, line: &[u8]) -> bool {
        match self {
            TermSet::Empty => false,
            TermSet::Plain { ac, .. } => ac.is_match(line),
            TermSet::Regex { set, .. } => set.is_match(line),
        }
    }

    /// Sets `hit[k]` for every line `lines.start + k` containing a term and counts per-term line hits.
    fn mark(&self, text: &LogText, lines: &Range<u64>, starts: &[u64], hit: &mut [bool], hits: &mut [u64], last: &mut [u64]) {
        match self {
            TermSet::Empty => {}
            TermSet::Plain { ac, terms } => {
                let (region, base) = text.span(lines.clone());
                for m in ac.find_overlapping_iter(region) {
                    let pos = base + m.start() as u64;
                    let rel = starts.partition_point(|&s| s <= pos) - 1;
                    hit[rel] = true;
                    let line = lines.start + rel as u64;
                    let term = terms[m.pattern().as_usize()];
                    if last[term] != line {
                        last[term] = line;
                        hits[term] += 1;
                    }
                }
            }
            TermSet::Regex { set, terms } => {
                for (rel, line) in lines.clone().enumerate() {
                    let content = text.line(line);
                    if !set.is_match(content) {
                        continue;
                    }
                    hit[rel] = true;
                    for p in set.matches(content).iter() {
                        hits[terms[p]] += 1;
                    }
                }
            }
        }
    }
}

fn decide(has_include: bool, included: bool, has_exclude: bool, excluded: bool) -> bool {
    if has_include {
        included && !excluded
    } else if has_exclude {
        !excluded
    } else {
        false
    }
}

/// A `FilterSpec` compiled into matchers (spec §4.8).
pub struct CompiledFilter {
    spec: FilterSpec,
    include: TermSet,
    exclude: TermSet,
}

impl CompiledFilter {
    pub fn compile(spec: &FilterSpec) -> Result<Self, CoreError> {
        Ok(Self {
            spec: spec.clone(),
            include: TermSet::build(spec, TermState::Include)?,
            exclude: TermSet::build(spec, TermState::Exclude)?,
        })
    }

    pub fn spec(&self) -> &FilterSpec {
        &self.spec
    }

    pub fn is_active(&self) -> bool {
        !self.include.is_empty() || !self.exclude.is_empty()
    }

    pub fn is_excluded(&self, line: &[u8]) -> bool {
        self.exclude.is_match(line)
    }

    /// Spec §4.7 semantics for one line (`line` already has `\r` stripped).
    pub fn line_matches(&self, line: &[u8]) -> bool {
        let has_include = !self.include.is_empty();
        let included = has_include && self.include.is_match(line);
        if has_include && !included {
            return false;
        }
        decide(has_include, included, !self.exclude.is_empty(), self.exclude.is_match(line))
    }

    /// Scans `lines` of `text`: matched lines (ascending) and per-term hit counts.
    pub fn scan(&self, text: &LogText, lines: Range<u64>) -> ScanResult {
        let lines = lines.start..lines.end.min(text.line_count());
        let mut term_hits = vec![0u64; self.spec.terms.len()];
        if lines.start >= lines.end || !self.is_active() {
            return ScanResult { matches: Vec::new(), term_hits };
        }
        let n = (lines.end - lines.start) as usize;
        let starts: Vec<u64> = lines.clone().map(|l| text.index().line_start(l)).collect();
        let mut last = vec![u64::MAX; term_hits.len()];
        let mut included = vec![false; n];
        let mut excluded = vec![false; n];
        self.include.mark(text, &lines, &starts, &mut included, &mut term_hits, &mut last);
        self.exclude.mark(text, &lines, &starts, &mut excluded, &mut term_hits, &mut last);
        let (has_include, has_exclude) = (!self.include.is_empty(), !self.exclude.is_empty());
        let matches = (0..n)
            .filter(|&k| decide(has_include, included[k], has_exclude, excluded[k]))
            .map(|k| lines.start + k as u64)
            .collect();
        ScanResult { matches, term_hits }
    }
}
```
In `mod.rs` add `mod compiled;` and `pub use compiled::{CompiledFilter, ScanResult};`.

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p rogg-core compiled`
Expected: 9 tests pass.

- [ ] **Step 5: Write the property test against a naive evaluator**

`crates/rogg-core/tests/filter_props.rs`:
```rust
use proptest::prelude::*;
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::text::LogText;

const WORDS: &[&str] = &["error", "timeout", "retry", "ok", "ERROR", "Time", "req_1", "req_22", "x", "é"];
const TERMS: &[&str] = &["error", "time", "timeout", "retry", "req_", "ok", "é", r"req_\d+", "^ok", "error|retry", "t.me"];

fn naive(line: &str, spec: &FilterSpec) -> bool {
    let hit = |t: &Term| {
        if spec.regex {
            regex::RegexBuilder::new(&t.text).case_insensitive(!spec.case_sensitive).build().unwrap().is_match(line)
        } else if spec.case_sensitive {
            line.contains(&t.text)
        } else {
            line.to_ascii_lowercase().contains(&t.text.to_ascii_lowercase())
        }
    };
    let inc: Vec<&Term> = spec.terms.iter().filter(|t| t.state == TermState::Include).collect();
    let exc: Vec<&Term> = spec.terms.iter().filter(|t| t.state == TermState::Exclude).collect();
    if !inc.is_empty() {
        inc.iter().any(|t| hit(t)) && !exc.iter().any(|t| hit(t))
    } else if !exc.is_empty() {
        !exc.iter().any(|t| hit(t))
    } else {
        false
    }
}

fn lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        (proptest::collection::vec(proptest::sample::select(WORDS), 0..5), any::<bool>())
            .prop_map(|(words, cr)| format!("{}{}", words.join(" "), if cr { "\r" } else { "" })),
        0..60,
    )
}

fn spec() -> impl Strategy<Value = FilterSpec> {
    (
        proptest::collection::vec(
            (proptest::sample::select(TERMS), prop_oneof![Just(TermState::Include), Just(TermState::Off), Just(TermState::Exclude)]),
            0..4,
        ),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(terms, regex, case_sensitive)| FilterSpec {
            terms: terms.into_iter().map(|(t, s)| Term::new(t, s)).collect(),
            regex,
            case_sensitive,
            context: 0,
        })
}

proptest! {
    #[test]
    fn chunked_scan_equals_naive(lines in lines(), spec in spec(), chunk in 1u64..9) {
        let text = LogText::from_bytes(lines.join("\n").into_bytes());
        let compiled = CompiledFilter::compile(&spec).unwrap();
        let mut got = Vec::new();
        let mut start = 0;
        while start < text.line_count() {
            let end = (start + chunk).min(text.line_count());
            got.extend(compiled.scan(&text, start..end).matches);
            start = end;
        }
        let expected: Vec<u64> = (0..text.line_count()).filter(|&i| naive(&text.line_text(i), &spec)).collect();
        prop_assert_eq!(got, expected);
    }
}
```

- [ ] **Step 6: Run the property test**

Run: `cargo test -p rogg-core --test filter_props`
Expected: PASS (256 cases).

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): compiled filter with Aho-Corasick/RegexSet chunk scanning

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Parallel filter engine with progressive, cancellable results

**Files:**
- Create: `crates/rogg-core/src/engine.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod engine;`)
- Test: `crates/rogg-core/tests/engine.rs`

**Interfaces:**
- Consumes: `CompiledFilter::scan`, `ScanResult` (Task 5); `LogText` (Task 3); `CancelToken`, `CoreError` (Task 1).
- Produces:
  - `engine::DEFAULT_CHUNK_LINES = 65_536`
  - `engine::FilterEvent`:
    - `Chunk { lines: Range<u64>, matches: Vec<u64> }`, emitted in ascending, contiguous order
    - `Progress { lines_done, total_lines }`
    - `Finished { term_hits: Vec<u64>, total_lines: u64, duration: Duration }`
    - `Cancelled`
  - `engine::FilterResult { matches: Vec<u64>, term_hits: Vec<u64>, total_lines: u64, duration: Duration }`
  - `engine::scan_lines(&LogText, &CompiledFilter, Range<u64>, chunk_lines, &CancelToken, emit: FnMut(FilterEvent))`
  - `engine::run_filter(&LogText, &CompiledFilter, chunk_lines, &CancelToken) -> Result<FilterResult, CoreError>` (blocking; collects every chunk)
  - `engine::FilterJob::spawn(Arc<LogText>, Arc<CompiledFilter>, chunk_lines) -> FilterJob`, with `events() -> &Receiver<FilterEvent>`, `cancel()`, `wait(self) -> Result<FilterResult, CoreError>`. Dropping a job cancels it and joins its thread.

Chunks are processed in waves of `4 × rayon threads` and emitted in order after each wave. Cancellation is checked before and after each wave. Once `Cancelled` is emitted nothing follows it, and `Finished` is never emitted after a cancel.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-core/tests/engine.rs`:
```rust
use rogg_core::CancelToken;
use rogg_core::engine::{FilterEvent, FilterJob, run_filter, scan_lines};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::text::LogText;
use std::sync::Arc;

fn sample(lines: usize) -> LogText {
    let words = ["error", "timeout", "ok", "retry", "healthcheck"];
    let text: Vec<String> = (0..lines).map(|i| format!("{i} {} {}", words[i % 5], words[(i * 7) % 5])).collect();
    LogText::from_bytes(text.join("\n").into_bytes())
}

fn filter() -> CompiledFilter {
    CompiledFilter::compile(&FilterSpec {
        terms: vec![Term::include("error"), Term::include("retry"), Term::new("healthcheck", TermState::Exclude)],
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn run_filter_equals_per_line_evaluation() {
    let text = sample(1_000);
    let f = filter();
    let result = run_filter(&text, &f, 7, &CancelToken::new()).unwrap();
    let expected: Vec<u64> = (0..text.line_count()).filter(|&i| f.line_matches(text.line(i))).collect();
    assert_eq!(result.matches, expected);
    assert_eq!(result.total_lines, 1_000);
    assert_eq!(result.term_hits.len(), 3);
}

#[test]
fn events_are_ordered_contiguous_and_end_with_finished() {
    let text = sample(500);
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..500, 9, &CancelToken::new(), |e| events.push(e));
    let mut next = 0;
    let mut last_progress = 0;
    for e in &events[..events.len() - 1] {
        match e {
            FilterEvent::Chunk { lines, matches } => {
                assert_eq!(lines.start, next);
                assert!(matches.iter().all(|m| lines.contains(m)));
                next = lines.end;
            }
            FilterEvent::Progress { lines_done, total_lines } => {
                assert!(*lines_done >= last_progress);
                assert_eq!(*total_lines, 500);
                last_progress = *lines_done;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(next, 500);
    assert!(matches!(events.last(), Some(FilterEvent::Finished { total_lines: 500, .. })));
}

#[test]
fn pre_cancelled_scan_emits_only_cancelled() {
    let text = sample(100);
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..100, 3, &cancel, |e| events.push(e));
    assert_eq!(events, vec![FilterEvent::Cancelled]);
    assert!(run_filter(&text, &filter(), 3, &cancel).is_err());
}

#[test]
fn cancel_mid_scan_stops_without_finished() {
    let text = sample(20_000);
    let cancel = CancelToken::new();
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..20_000, 2, &cancel, |e| {
        if matches!(e, FilterEvent::Chunk { .. }) {
            cancel.cancel();
        }
        events.push(e);
    });
    assert_eq!(events.last(), Some(&FilterEvent::Cancelled));
    assert!(!events.iter().any(|e| matches!(e, FilterEvent::Finished { .. })));
    let chunks = events.iter().filter(|e| matches!(e, FilterEvent::Chunk { .. })).count();
    assert!(chunks < 10_000, "stopped early ({chunks} chunks)");
}

#[test]
fn inactive_filter_finishes_with_no_matches() {
    let text = sample(50);
    let none = CompiledFilter::compile(&FilterSpec::default()).unwrap();
    let r = run_filter(&text, &none, 8, &CancelToken::new()).unwrap();
    assert!(r.matches.is_empty());
    assert_eq!(r.total_lines, 50);
}

#[test]
fn job_wait_matches_run_filter_and_drop_joins() {
    let text = Arc::new(sample(3_000));
    let f = Arc::new(filter());
    let expected = run_filter(&text, &f, 64, &CancelToken::new()).unwrap();
    let job = FilterJob::spawn(text.clone(), f.clone(), 64);
    let got = job.wait().unwrap();
    assert_eq!(got.matches, expected.matches);
    assert_eq!(got.term_hits, expected.term_hits);
    let big = Arc::new(sample(200_000));
    let job = FilterJob::spawn(big, f, 16);
    drop(job); // must cancel and join without hanging
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core --test engine`
Expected: compile error, `rogg_core::engine` is not found.

- [ ] **Step 3: Implement `engine.rs`**

```rust
//! Parallel filter scanning with in-order progressive results and cancellation (spec §3.3, §4.8).

use crate::cancel::CancelToken;
use crate::error::CoreError;
use crate::filter::{CompiledFilter, ScanResult};
use crate::text::LogText;
use rayon::prelude::*;
use std::ops::Range;
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const DEFAULT_CHUNK_LINES: u64 = 65_536;

#[derive(Clone, Debug, PartialEq)]
pub enum FilterEvent {
    /// Matches within `lines`; chunks arrive in ascending, contiguous order.
    Chunk { lines: Range<u64>, matches: Vec<u64> },
    Progress { lines_done: u64, total_lines: u64 },
    Finished { term_hits: Vec<u64>, total_lines: u64, duration: Duration },
    Cancelled,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilterResult {
    pub matches: Vec<u64>,
    pub term_hits: Vec<u64>,
    pub total_lines: u64,
    pub duration: Duration,
}

/// Scans `lines` on all cores, emitting chunk results in order after each wave.
pub fn scan_lines(
    text: &LogText,
    filter: &CompiledFilter,
    lines: Range<u64>,
    chunk_lines: u64,
    cancel: &CancelToken,
    mut emit: impl FnMut(FilterEvent),
) {
    let started = Instant::now();
    let lines = lines.start..lines.end.min(text.line_count());
    let total = lines.end.saturating_sub(lines.start);
    let chunk = chunk_lines.max(1);
    let ranges: Vec<Range<u64>> = (0..total.div_ceil(chunk))
        .map(|k| {
            let s = lines.start + k * chunk;
            s..(s + chunk).min(lines.end)
        })
        .collect();
    let mut term_hits = vec![0u64; filter.spec().terms.len()];
    let wave = rayon::current_num_threads().max(1) * 4;
    let mut done = 0;
    for group in ranges.chunks(wave) {
        if cancel.is_cancelled() {
            emit(FilterEvent::Cancelled);
            return;
        }
        let results: Vec<ScanResult> = group
            .par_iter()
            .map(|r| if cancel.is_cancelled() { ScanResult::default() } else { filter.scan(text, r.clone()) })
            .collect();
        if cancel.is_cancelled() {
            emit(FilterEvent::Cancelled);
            return;
        }
        for (r, result) in group.iter().zip(results) {
            for (total_hits, hits) in term_hits.iter_mut().zip(&result.term_hits) {
                *total_hits += hits;
            }
            done += r.end - r.start;
            emit(FilterEvent::Chunk { lines: r.clone(), matches: result.matches });
        }
        emit(FilterEvent::Progress { lines_done: done, total_lines: total });
    }
    if cancel.is_cancelled() {
        emit(FilterEvent::Cancelled);
        return;
    }
    emit(FilterEvent::Finished { term_hits, total_lines: total, duration: started.elapsed() });
}

fn collect(events: impl IntoIterator<Item = FilterEvent>) -> Result<FilterResult, CoreError> {
    let mut result = FilterResult::default();
    for event in events {
        match event {
            FilterEvent::Chunk { matches, .. } => result.matches.extend(matches),
            FilterEvent::Progress { .. } => {}
            FilterEvent::Finished { term_hits, total_lines, duration } => {
                result.term_hits = term_hits;
                result.total_lines = total_lines;
                result.duration = duration;
                return Ok(result);
            }
            FilterEvent::Cancelled => return Err(CoreError::Cancelled),
        }
    }
    Err(CoreError::Cancelled)
}

/// Blocking scan of the whole text.
pub fn run_filter(text: &LogText, filter: &CompiledFilter, chunk_lines: u64, cancel: &CancelToken) -> Result<FilterResult, CoreError> {
    let mut events = Vec::new();
    scan_lines(text, filter, 0..text.line_count(), chunk_lines, cancel, |e| events.push(e));
    collect(events)
}

/// A filter scan on a background thread; the receiver yields `FilterEvent`s as chunks complete.
pub struct FilterJob {
    cancel: CancelToken,
    events: mpsc::Receiver<FilterEvent>,
    thread: Option<JoinHandle<()>>,
}

impl FilterJob {
    pub fn spawn(text: Arc<LogText>, filter: Arc<CompiledFilter>, chunk_lines: u64) -> Self {
        let cancel = CancelToken::new();
        let (tx, events) = mpsc::channel();
        let token = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("rogg-filter".into())
            .spawn(move || {
                scan_lines(&text, &filter, 0..text.line_count(), chunk_lines, &token, |e| {
                    let _ = tx.send(e);
                })
            })
            .expect("spawn filter thread");
        Self { cancel, events, thread: Some(thread) }
    }

    pub fn events(&self) -> &mpsc::Receiver<FilterEvent> {
        &self.events
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Blocks until the scan finishes and returns the collected result.
    pub fn wait(mut self) -> Result<FilterResult, CoreError> {
        let result = collect(self.events.iter());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        result
    }
}

impl Drop for FilterJob {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
```
Add `pub mod engine;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-core --test engine`
Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): parallel filter engine with ordered progressive results and cancellation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Filtered view — pins, context and gap rows

**Files:**
- Create: `crates/rogg-core/src/view.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod view;`)
- Test: unit tests in `view.rs`; `crates/rogg-core/tests/view_props.rs`

**Interfaces:**
- Produces:
  - `view::CHECKPOINT = 1024`
  - `view::ViewRow::{Line { number: u64, matched: bool, pinned: bool }, Gap { omitted: u64 }}` with `is_context()`, which is true for a `Line` that is neither matched nor pinned
  - `view::FilteredView::build(matches: Arc<Vec<u64>>, pins: &BTreeSet<u64>, context: u32, total_lines: u64, excluded: impl Fn(u64) -> bool) -> FilteredView`
  - `FilteredView` methods:
    - rows: `row_count() -> u64`, `row(u64) -> Option<ViewRow>`, `row_for_line(u64) -> Option<u64>`
    - lines: `shown_lines() -> &Arc<Vec<u64>>`, `matches() -> &Arc<Vec<u64>>`
    - counts: `context_lines() -> u64`, `pinned_count() -> u64`, `pinned_unmatched() -> u64`

Composition follows spec §4.7:
- Shown lines are matches ∪ pins, plus up to `context` lines on each side of every match or pin, except context lines for which `excluded(line)` is true.
- A gap row sits before every shown line that doesn't directly follow the previous shown line.
- With `context == 0` and no pins, `shown_lines` shares the `matches` Arc, so nothing is copied.
- The view stores only sorted line numbers plus a row checkpoint every 1024 shown lines, so 10 M matches cost ~80 MB, not one object per row.

- [ ] **Step 1: Write the failing unit tests**

The expected values below come from running the reference mockup's own JavaScript (`renderBottom`) on its 40 sample lines: matches `[12,15,20,23,28,34]`, pin `{7}`, and "healthcheck" lines `{5,6,18,26}` excluded.

`crates/rogg-core/src/view.rs` (tests first):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const EXCLUDED: [u64; 4] = [5, 6, 18, 26];

    fn reference(context: u32) -> FilteredView {
        FilteredView::build(Arc::new(vec![12, 15, 20, 23, 28, 34]), &BTreeSet::from([7]), context, 40, |l| EXCLUDED.contains(&l))
    }

    fn line(r: Option<ViewRow>) -> u64 {
        match r {
            Some(ViewRow::Line { number, .. }) => number,
            other => panic!("expected a line row, got {other:?}"),
        }
    }

    #[test]
    fn reference_without_context() {
        let v = reference(0);
        assert_eq!(**v.shown_lines(), vec![7, 12, 15, 20, 23, 28, 34]);
        assert_eq!(v.row_count(), 7 + 6);
        assert_eq!(v.row(0), Some(ViewRow::Line { number: 7, matched: false, pinned: true }));
        assert_eq!(v.row(1), Some(ViewRow::Gap { omitted: 4 }));
        assert_eq!(v.row(2), Some(ViewRow::Line { number: 12, matched: true, pinned: false }));
        assert_eq!(v.row(12), Some(ViewRow::Line { number: 34, matched: true, pinned: false }));
        assert_eq!(v.row(13), None);
        assert_eq!(v.row_for_line(15), Some(4));
        assert_eq!(v.row_for_line(8), None);
        assert_eq!((v.context_lines(), v.pinned_count(), v.pinned_unmatched()), (0, 1, 1));
    }

    #[test]
    fn reference_with_context_one_and_three() {
        let v1 = reference(1);
        assert_eq!(**v1.shown_lines(), vec![7, 8, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 27, 28, 29, 33, 34, 35]);
        assert_eq!(v1.context_lines(), 13);
        assert!(matches!(v1.row(1), Some(ViewRow::Line { number: 8, matched: false, pinned: false })));
        assert!(v1.row(1).unwrap().is_context());
        let v3 = reference(3);
        assert_eq!(v3.shown_lines().len(), 30);
        assert_eq!(&v3.shown_lines()[..3], &[4, 7, 8]);
        assert_eq!(v3.context_lines(), 23);
    }

    #[test]
    fn shares_matches_when_nothing_is_added() {
        let matches = Arc::new(vec![1, 2, 9]);
        let v = FilteredView::build(matches.clone(), &BTreeSet::new(), 0, 10, |_| false);
        assert!(Arc::ptr_eq(v.shown_lines(), &matches));
    }

    #[test]
    fn edges_and_empty_inputs() {
        let empty = FilteredView::build(Arc::new(vec![]), &BTreeSet::new(), 3, 0, |_| false);
        assert_eq!(empty.row_count(), 0);
        assert_eq!(empty.row(0), None);
        let edges = FilteredView::build(Arc::new(vec![0, 9]), &BTreeSet::from([42]), 2, 10, |_| false);
        assert_eq!(**edges.shown_lines(), vec![0, 1, 2, 7, 8, 9], "pins beyond the file are ignored; windows clamp");
        assert_eq!(edges.pinned_count(), 0);
        assert_eq!(edges.row(3), Some(ViewRow::Gap { omitted: 4 }));
    }

    #[test]
    fn rows_resolve_across_checkpoints() {
        let matches: Vec<u64> = (0..5_000).map(|i| i * 3).collect();
        let v = FilteredView::build(Arc::new(matches.clone()), &BTreeSet::new(), 0, 15_000, |_| false);
        assert_eq!(v.row_count(), 5_000 + 4_999);
        for (k, &m) in matches.iter().enumerate().step_by(997) {
            let r = v.row_for_line(m).unwrap();
            assert_eq!(r, (k as u64) * 2);
            assert_eq!(line(v.row(r)), m);
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core view`
Expected: compile errors, `FilteredView` and `ViewRow` are not found.

- [ ] **Step 3: Implement `view.rs`**

```rust
//! The filtered view: matches ∪ pins ∪ context, with gap rows (spec §4.7).

use std::collections::BTreeSet;
use std::sync::Arc;

/// A row checkpoint is stored every CHECKPOINT shown lines; row lookups walk at most this many lines.
pub const CHECKPOINT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRow {
    Line { number: u64, matched: bool, pinned: bool },
    Gap { omitted: u64 },
}

impl ViewRow {
    /// A context line: shown only because it is near a match or pin.
    pub fn is_context(&self) -> bool {
        matches!(self, ViewRow::Line { matched: false, pinned: false, .. })
    }
}

pub struct FilteredView {
    matches: Arc<Vec<u64>>,
    pins: BTreeSet<u64>,
    shown: Arc<Vec<u64>>,
    checkpoints: Vec<u64>,
    row_count: u64,
    context_lines: u64,
}

fn merge_anchors(matches: &[u64], pins: &BTreeSet<u64>) -> Vec<u64> {
    let mut out = Vec::with_capacity(matches.len() + pins.len());
    let mut pins = pins.iter().copied().peekable();
    for &m in matches {
        while let Some(&p) = pins.peek() {
            if p >= m {
                break;
            }
            out.push(p);
            pins.next();
        }
        if pins.peek() == Some(&m) {
            pins.next();
        }
        out.push(m);
    }
    out.extend(pins);
    out
}

fn compose(anchors: &[u64], radius: u64, total: u64, excluded: &impl Fn(u64) -> bool) -> Vec<u64> {
    if total == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(anchors.len() * (1 + 2 * radius as usize).min(8));
    let mut next_free = 0u64;
    for (k, &a) in anchors.iter().enumerate() {
        for j in a.saturating_sub(radius).max(next_free)..a {
            if !excluded(j) {
                out.push(j);
            }
        }
        out.push(a);
        let hi = (a + radius).min(total - 1);
        let next_anchor = anchors.get(k + 1).copied().unwrap_or(u64::MAX);
        let mut j = a + 1;
        while j <= hi && j < next_anchor {
            if !excluded(j) {
                out.push(j);
            }
            j += 1;
        }
        next_free = j;
    }
    out
}

fn layout(shown: &[u64]) -> (Vec<u64>, u64) {
    let mut checkpoints = Vec::with_capacity(shown.len() / CHECKPOINT + 1);
    let mut row = 0u64;
    for (k, &line) in shown.iter().enumerate() {
        if k > 0 && line > shown[k - 1] + 1 {
            row += 1;
        }
        if k % CHECKPOINT == 0 {
            checkpoints.push(row);
        }
        row += 1;
    }
    (checkpoints, row)
}

impl FilteredView {
    pub fn build(matches: Arc<Vec<u64>>, pins: &BTreeSet<u64>, context: u32, total_lines: u64, excluded: impl Fn(u64) -> bool) -> Self {
        let pins: BTreeSet<u64> = pins.iter().copied().filter(|&p| p < total_lines).collect();
        let shown = if context == 0 && pins.is_empty() {
            matches.clone()
        } else {
            let anchors = merge_anchors(&matches, &pins);
            Arc::new(compose(&anchors, context as u64, total_lines, &excluded))
        };
        let context_lines =
            shown.iter().filter(|n| !pins.contains(n) && matches.binary_search(n).is_err()).count() as u64;
        let (checkpoints, row_count) = layout(&shown);
        Self { matches, pins, shown, checkpoints, row_count, context_lines }
    }

    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    fn gap_before(&self, k: usize) -> bool {
        k > 0 && self.shown[k] > self.shown[k - 1] + 1
    }

    fn line_row(&self, number: u64) -> ViewRow {
        ViewRow::Line { number, matched: self.matches.binary_search(&number).is_ok(), pinned: self.pins.contains(&number) }
    }

    pub fn row(&self, r: u64) -> Option<ViewRow> {
        if r >= self.row_count {
            return None;
        }
        let c = self.checkpoints.partition_point(|&x| x <= r) - 1;
        let mut k = c * CHECKPOINT;
        let mut row = self.checkpoints[c];
        while row < r {
            k += 1;
            row += 1 + self.gap_before(k) as u64;
        }
        if row == r {
            Some(self.line_row(self.shown[k]))
        } else {
            Some(ViewRow::Gap { omitted: self.shown[k] - self.shown[k - 1] - 1 })
        }
    }

    pub fn row_for_line(&self, number: u64) -> Option<u64> {
        let k = self.shown.binary_search(&number).ok()?;
        let c = k / CHECKPOINT;
        let mut row = self.checkpoints[c];
        for j in c * CHECKPOINT + 1..=k {
            row += 1 + self.gap_before(j) as u64;
        }
        Some(row)
    }

    pub fn shown_lines(&self) -> &Arc<Vec<u64>> {
        &self.shown
    }

    pub fn matches(&self) -> &Arc<Vec<u64>> {
        &self.matches
    }

    pub fn context_lines(&self) -> u64 {
        self.context_lines
    }

    pub fn pinned_count(&self) -> u64 {
        self.pins.len() as u64
    }

    pub fn pinned_unmatched(&self) -> u64 {
        self.pins.iter().filter(|p| self.matches.binary_search(p).is_err()).count() as u64
    }
}
```
Add `pub mod view;` to `lib.rs`.

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p rogg-core view`
Expected: 5 tests pass.

- [ ] **Step 5: Write the property test against a naive row model**

`crates/rogg-core/tests/view_props.rs`:
```rust
use proptest::prelude::*;
use rogg_core::view::{FilteredView, ViewRow};
use std::collections::BTreeSet;
use std::sync::Arc;

fn naive_rows(matches: &BTreeSet<u64>, pins: &BTreeSet<u64>, n: u64, total: u64, excluded: &BTreeSet<u64>) -> Vec<ViewRow> {
    let anchors: BTreeSet<u64> = matches.union(pins).copied().filter(|&l| l < total).collect();
    let shown: Vec<u64> = (0..total)
        .filter(|&j| anchors.contains(&j) || (!excluded.contains(&j) && anchors.iter().any(|&a| a.abs_diff(j) <= n)))
        .collect();
    let mut rows = Vec::new();
    for (k, &l) in shown.iter().enumerate() {
        if k > 0 && l > shown[k - 1] + 1 {
            rows.push(ViewRow::Gap { omitted: l - shown[k - 1] - 1 });
        }
        rows.push(ViewRow::Line { number: l, matched: matches.contains(&l), pinned: pins.contains(&l) });
    }
    rows
}

proptest! {
    #[test]
    fn view_equals_naive(
        total in 0u64..3_000,
        matches in proptest::collection::btree_set(0u64..3_000, 0..300),
        pins in proptest::collection::btree_set(0u64..3_100, 0..10),
        excluded in proptest::collection::btree_set(0u64..3_000, 0..300),
        n in 0u32..4,
    ) {
        let matches: BTreeSet<u64> = matches.into_iter().filter(|&m| m < total && !excluded.contains(&m)).collect();
        let view = FilteredView::build(Arc::new(matches.iter().copied().collect()), &pins, n, total, |l| excluded.contains(&l));
        let expected = naive_rows(&matches, &pins, n as u64, total, &excluded);
        prop_assert_eq!(view.row_count(), expected.len() as u64);
        for (r, row) in expected.iter().enumerate() {
            prop_assert_eq!(view.row(r as u64), Some(*row));
            if let ViewRow::Line { number, .. } = row {
                prop_assert_eq!(view.row_for_line(*number), Some(r as u64));
            }
        }
    }
}
```
Matches are filtered to exclude excluded lines, mirroring the engine: a match can't be an excluded line. Pins may be excluded lines; pins always show.

- [ ] **Step 6: Run the property test**

Run: `cargo test -p rogg-core --test view_props`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): filtered view composing matches, pins, context and gap rows

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 8: Highlight spans and per-panel search

**Files:**
- Create: `crates/rogg-core/src/highlight.rs`, `crates/rogg-core/src/search.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod highlight; pub mod search;`)
- Test: unit tests in both files

**Interfaces:**
- Consumes: `FilterSpec`, `Term`, `TermState`, `CompiledFilter` (Tasks 4–5); `run_filter` (Task 6); `LogText` (Task 3).
- Produces:
  - `highlight::{MAX_SPANS_PER_TERM = 4096, Highlighter}`, with:
    - `Highlighter::new(&FilterSpec) -> Result<Highlighter, CoreError>` (Include terms only)
    - `spans(&self, display: &str, palette_len: usize) -> Vec<(Range<usize>, usize)>`: byte ranges on the display string, sorted and non-overlapping, always on char boundaries. The earlier term wins on overlap, the slot is `term index % palette_len`, and empty matches are skipped.
  - `search::SearchQuery { text: String, regex: bool, case_sensitive: bool }` (Default), with `is_empty()` and `compile() -> Result<CompiledSearch, CoreError>`
  - `search::SearchScope::{All, Lines(Arc<Vec<u64>>)}`
  - `search::CompiledSearch`, with:
    - `run(&LogText, &SearchScope, chunk_lines, &CancelToken) -> Result<SearchResult, CoreError>`
    - `spans(&str) -> Vec<Range<usize>>`
  - `search::SearchResult { lines: Vec<u64> }`, with:
    - `count() -> usize`
    - `next_after(Option<u64>) -> Option<u64>` and `prev_before(Option<u64>) -> Option<u64>`, both wrapping around

Spans are computed on the **display string** (`LogText::line_text`), not on raw bytes. Range ends therefore always fall on char boundaries even for lines with invalid UTF-8, which is what the M0 renderer needs. This pins Review Focus 3.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-core/src/highlight.rs` (tests first):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};

    fn spec(terms: &[(&str, TermState)], regex: bool) -> FilterSpec {
        FilterSpec { terms: terms.iter().map(|(t, s)| Term::new(*t, *s)).collect(), regex, ..Default::default() }
    }

    #[test]
    fn earlier_term_wins_and_slots_follow_term_index() {
        let h = Highlighter::new(&spec(&[("timeout", TermState::Include), ("out", TermState::Include)], false)).unwrap();
        assert_eq!(h.spans("a timeout, out", 4), vec![(2..9, 0), (11..14, 1)]);
        let many: Vec<(String, TermState)> = (0..6).map(|i| (format!("t{i}"), TermState::Include)).collect();
        let refs: Vec<(&str, TermState)> = many.iter().map(|(t, s)| (t.as_str(), *s)).collect();
        let h = Highlighter::new(&spec(&refs, false)).unwrap();
        assert_eq!(h.spans("t5", 4), vec![(0..2, 1)], "slot = term index 5 % 4");
    }

    #[test]
    fn only_include_terms_are_highlighted_case_insensitively() {
        let h = Highlighter::new(&spec(&[("error", TermState::Include), ("ok", TermState::Exclude), ("x", TermState::Off)], false)).unwrap();
        assert_eq!(h.spans("ERROR ok x error", 4), vec![(0..5, 0), (11..16, 0)]);
    }

    #[test]
    fn spans_land_on_char_boundaries_for_lossy_text() {
        let display = String::from_utf8_lossy(b"\xFF\xFEtimeout \xC3\xA9 t\xFFimeout").into_owned();
        let h = Highlighter::new(&spec(&[("timeout", TermState::Include), ("é", TermState::Include)], false)).unwrap();
        let spans = h.spans(&display, 4);
        assert!(!spans.is_empty());
        for (r, _) in &spans {
            assert!(display.is_char_boundary(r.start) && display.is_char_boundary(r.end), "{r:?} in {display:?}");
        }
    }

    #[test]
    fn empty_regex_matches_are_skipped() {
        let h = Highlighter::new(&spec(&[("x*", TermState::Include)], true)).unwrap();
        assert_eq!(h.spans("axxb", 4), vec![(1..3, 0)]);
        assert!(Highlighter::new(&spec(&[("(bad", TermState::Include)], true)).is_err());
    }

    #[test]
    fn spans_per_term_are_capped() {
        let h = Highlighter::new(&spec(&[("a", TermState::Include)], false)).unwrap();
        let line = "a ".repeat(10_000);
        let covered: usize = h.spans(&line, 4).iter().map(|(r, _)| r.len()).sum();
        assert_eq!(covered, MAX_SPANS_PER_TERM);
    }
}
```

`crates/rogg-core/src/search.rs` (tests first):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::LogText;

    fn text() -> LogText {
        LogText::from_bytes(b"alpha timeout\nbeta\ngamma TIMEOUT\ndelta\ntimeout again\n".to_vec())
    }

    #[test]
    fn counts_lines_in_whole_text_and_in_shown_lines() {
        let q = SearchQuery { text: "timeout".into(), ..Default::default() };
        let c = q.compile().unwrap();
        let all = c.run(&text(), &SearchScope::All, 2, &CancelToken::new()).unwrap();
        assert_eq!(all.lines, vec![0, 2, 4]);
        assert_eq!(all.count(), 3);
        let shown = c.run(&text(), &SearchScope::Lines(Arc::new(vec![1, 2, 3])), 2, &CancelToken::new()).unwrap();
        assert_eq!(shown.lines, vec![2]);
    }

    #[test]
    fn regex_case_and_errors() {
        let q = SearchQuery { text: "^[a-z]+ timeout$".into(), regex: true, case_sensitive: true };
        assert_eq!(q.compile().unwrap().run(&text(), &SearchScope::All, 8, &CancelToken::new()).unwrap().lines, vec![0]);
        assert!(SearchQuery { text: "(".into(), regex: true, ..Default::default() }.compile().is_err());
    }

    #[test]
    fn next_and_prev_wrap() {
        let r = SearchResult { lines: vec![3, 8, 20] };
        assert_eq!(r.next_after(None), Some(3));
        assert_eq!(r.next_after(Some(8)), Some(20));
        assert_eq!(r.next_after(Some(20)), Some(3));
        assert_eq!(r.prev_before(None), Some(20));
        assert_eq!(r.prev_before(Some(8)), Some(3));
        assert_eq!(r.prev_before(Some(3)), Some(20));
        assert_eq!(SearchResult::default().next_after(Some(1)), None);
    }

    #[test]
    fn empty_query_finds_nothing_and_cancel_errors() {
        let empty = SearchQuery::default().compile().unwrap();
        assert!(empty.run(&text(), &SearchScope::All, 8, &CancelToken::new()).unwrap().lines.is_empty());
        assert!(empty.spans("anything").is_empty());
        let cancel = CancelToken::new();
        cancel.cancel();
        let q = SearchQuery { text: "a".into(), ..Default::default() }.compile().unwrap();
        assert!(q.run(&text(), &SearchScope::All, 8, &cancel).is_err());
        assert!(q.run(&text(), &SearchScope::Lines(Arc::new(vec![0])), 8, &cancel).is_err());
    }

    #[test]
    fn spans_mark_every_hit() {
        let q = SearchQuery { text: "time".into(), ..Default::default() }.compile().unwrap();
        assert_eq!(q.spans("Time to timeout"), vec![0..4, 8..12]);
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core highlight search`
Expected: compile errors, `Highlighter`, `SearchQuery` and the rest are not found. (If cargo rejects two filters, run `cargo test -p rogg-core highlight` and `cargo test -p rogg-core search` separately.)

- [ ] **Step 3: Implement `highlight.rs`**

Above its tests:
```rust
//! Highlight spans for painted rows (spec §4.8): Include terms over the display string.

use crate::error::CoreError;
use crate::filter::{FilterSpec, TermState};
use aho_corasick::{AhoCorasick, MatchKind};
use std::ops::Range;

/// At most this many matches per term are considered per line (bounds work on hostile lines).
pub const MAX_SPANS_PER_TERM: usize = 4096;

enum Finder {
    Plain(AhoCorasick),
    Regex(regex::Regex),
}

impl Finder {
    fn build(text: &str, regex: bool, case_sensitive: bool) -> Result<Self, CoreError> {
        if regex {
            regex::RegexBuilder::new(text)
                .case_insensitive(!case_sensitive)
                .build()
                .map(Finder::Regex)
                .map_err(|e| CoreError::InvalidRegex { term: text.to_string(), message: e.to_string() })
        } else {
            AhoCorasick::builder()
                .ascii_case_insensitive(!case_sensitive)
                .match_kind(MatchKind::LeftmostFirst)
                .build([text])
                .map(Finder::Plain)
                .map_err(|e| CoreError::InvalidRegex { term: text.to_string(), message: e.to_string() })
        }
    }

    fn ranges<'a>(&'a self, display: &'a str) -> Box<dyn Iterator<Item = Range<usize>> + 'a> {
        match self {
            Finder::Plain(ac) => Box::new(ac.find_iter(display).map(|m| m.range())),
            Finder::Regex(re) => Box::new(re.find_iter(display).map(|m| m.range())),
        }
    }
}

pub struct Highlighter {
    finders: Vec<(usize, Finder)>,
}

impl Highlighter {
    /// Highlighter for the Include terms of `spec`, keeping each term's index for its color slot.
    pub fn new(spec: &FilterSpec) -> Result<Self, CoreError> {
        let mut finders = Vec::new();
        for (i, term) in spec.terms.iter().enumerate() {
            if term.state == TermState::Include && !term.text.is_empty() {
                finders.push((i, Finder::build(&term.text, spec.regex, spec.case_sensitive)?));
            }
        }
        Ok(Self { finders })
    }

    pub(crate) fn single(text: &str, regex: bool, case_sensitive: bool) -> Result<Self, CoreError> {
        if text.is_empty() {
            return Ok(Self { finders: Vec::new() });
        }
        Ok(Self { finders: vec![(0, Finder::build(text, regex, case_sensitive)?)] })
    }

    /// Byte ranges of `display` to highlight with their palette slot; earlier terms win on overlap.
    pub fn spans(&self, display: &str, palette_len: usize) -> Vec<(Range<usize>, usize)> {
        if self.finders.is_empty() || display.is_empty() {
            return Vec::new();
        }
        let mut owner = vec![u32::MAX; display.len()];
        for (term, finder) in &self.finders {
            for range in finder.ranges(display).filter(|r| !r.is_empty()).take(MAX_SPANS_PER_TERM) {
                for slot in &mut owner[range] {
                    if *slot == u32::MAX {
                        *slot = *term as u32;
                    }
                }
            }
        }
        let palette_len = palette_len.max(1);
        let mut out = Vec::new();
        let mut i = 0;
        while i < owner.len() {
            let term = owner[i];
            if term == u32::MAX {
                i += 1;
                continue;
            }
            let start = i;
            while i < owner.len() && owner[i] == term {
                i += 1;
            }
            out.push((start..i, term as usize % palette_len));
        }
        out
    }
}
```

The cap test expects exactly `MAX_SPANS_PER_TERM` covered bytes. Each match of `"a"` is 1 byte and adjacent matches are separated by spaces, so 4096 matches cover 4096 bytes.

- [ ] **Step 4: Implement `search.rs`**

Above its tests:
```rust
//! Per-panel search (spec §4.10): a query independent of the filters, over the whole text or the shown rows.

use crate::cancel::CancelToken;
use crate::engine::run_filter;
use crate::error::CoreError;
use crate::filter::{CompiledFilter, FilterSpec, Term};
use crate::highlight::Highlighter;
use crate::text::LogText;
use rayon::prelude::*;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct SearchQuery {
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
}

#[derive(Clone, Debug)]
pub enum SearchScope {
    All,
    Lines(Arc<Vec<u64>>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    /// Lines containing at least one hit, ascending.
    pub lines: Vec<u64>,
}

pub struct CompiledSearch {
    filter: CompiledFilter,
    highlighter: Highlighter,
}

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn compile(&self) -> Result<CompiledSearch, CoreError> {
        let terms = if self.is_empty() { Vec::new() } else { vec![Term::include(self.text.clone())] };
        let spec = FilterSpec { terms, regex: self.regex, case_sensitive: self.case_sensitive, context: 0 };
        Ok(CompiledSearch {
            filter: CompiledFilter::compile(&spec)?,
            highlighter: Highlighter::single(&self.text, self.regex, self.case_sensitive)?,
        })
    }
}

impl CompiledSearch {
    pub fn run(&self, text: &LogText, scope: &SearchScope, chunk_lines: u64, cancel: &CancelToken) -> Result<SearchResult, CoreError> {
        match scope {
            SearchScope::All => run_filter(text, &self.filter, chunk_lines, cancel).map(|r| SearchResult { lines: r.matches }),
            SearchScope::Lines(lines) => {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let parts: Vec<Option<Vec<u64>>> = lines
                    .par_chunks(chunk_lines.max(1) as usize)
                    .map(|chunk| {
                        if cancel.is_cancelled() {
                            return None;
                        }
                        Some(chunk.iter().copied().filter(|&n| self.filter.line_matches(text.line(n))).collect())
                    })
                    .collect();
                if cancel.is_cancelled() || parts.iter().any(Option::is_none) {
                    return Err(CoreError::Cancelled);
                }
                Ok(SearchResult { lines: parts.into_iter().flatten().flatten().collect() })
            }
        }
    }

    /// Hit ranges on a display string, for the accent outline.
    pub fn spans(&self, display: &str) -> Vec<Range<usize>> {
        self.highlighter.spans(display, 1).into_iter().map(|(r, _)| r).collect()
    }
}

impl SearchResult {
    pub fn count(&self) -> usize {
        self.lines.len()
    }

    /// The first hit after `line` (wrapping); the first hit when `line` is None.
    pub fn next_after(&self, line: Option<u64>) -> Option<u64> {
        let first = *self.lines.first()?;
        match line {
            None => Some(first),
            Some(l) => Some(self.lines.get(self.lines.partition_point(|&x| x <= l)).copied().unwrap_or(first)),
        }
    }

    /// The last hit before `line` (wrapping); the last hit when `line` is None.
    pub fn prev_before(&self, line: Option<u64>) -> Option<u64> {
        let last = *self.lines.last()?;
        match line {
            None => Some(last),
            Some(l) => {
                let i = self.lines.partition_point(|&x| x < l);
                Some(if i == 0 { last } else { self.lines[i - 1] })
            }
        }
    }
}
```
Add `pub mod highlight; pub mod search;` to `lib.rs`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rogg-core highlight` and `cargo test -p rogg-core search`
Expected: 5 + 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): highlight spans on display text and per-panel search

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Statistics, filter history and app paths

**Files:**
- Create: `crates/rogg-core/src/stats.rs`, `crates/rogg-core/src/history.rs`, `crates/rogg-core/src/paths.rs`
- Modify: `crates/rogg-core/src/lib.rs` (add `pub mod history; pub mod paths; pub mod stats;`)
- Test: unit tests in each file

**Interfaces:**
- Consumes: `FilterSpec`, `Term`, `TermState` (Task 4); `FilterResult` (Task 6); `FilteredView` (Task 7).
- Produces:
  - `stats::TermHits { text: String, state: TermState, lines: u64 }`
  - `stats::FilterStats { total_lines, matched, pinned, pinned_unmatched, context_lines, term_hits: Vec<TermHits>, duration }`, with:
    - `compute(&FilterSpec, &FilterResult, &FilteredView)`
    - `summary() -> String`, giving e.g. `"6 matches / 40 lines · 1 pinned (1 added) · 0 context"`
  - `stats::group_digits(u64) -> String` (`1204 → "1,204"`)
  - `history::{HISTORY_LIMIT = 100, HistoryEntry { terms, regex, case_sensitive, context }, History, LoadOutcome { history, recovered: bool }}`
  - `HistoryEntry` methods: `from_spec(&FilterSpec)`, `to_spec()`, `expression()`
  - `History` methods:
    - `record(HistoryEntry)`: deduplicates, moves an existing entry to the front, caps at 100, ignores entries with no terms
    - `entries() -> &[HistoryEntry]`
    - `load(&Path) -> Result<LoadOutcome, CoreError>`: a missing file gives an empty history; an unparseable file is renamed to `<file>.bak` and gives an empty history with `recovered: true`
    - `save(&Path) -> Result<(), CoreError>`: atomic, via `<file>.partial` and rename
  - `paths::{home_dir, app_support_dir, cache_dir, history_path}`, giving `$HOME/Library/Application Support/rogg`, `$HOME/Library/Caches/rogg` and `…/history.json`

- [ ] **Step 1: Write the failing tests**

`stats.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::FilterResult;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::view::FilteredView;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    #[test]
    fn computes_reference_numbers_and_summary() {
        let spec = FilterSpec {
            terms: vec![Term::include("error"), Term::include("timeout"), Term::new("healthcheck", TermState::Exclude), Term::new("retry", TermState::Off)],
            ..Default::default()
        };
        let result = FilterResult { matches: vec![12, 15, 20, 23, 28, 34], term_hits: vec![7, 7, 4, 0], total_lines: 40, duration: Duration::from_millis(3) };
        let view = FilteredView::build(Arc::new(result.matches.clone()), &BTreeSet::from([7]), 0, 40, |_| false);
        let stats = FilterStats::compute(&spec, &result, &view);
        assert_eq!((stats.matched, stats.total_lines, stats.pinned, stats.pinned_unmatched, stats.context_lines), (6, 40, 1, 1, 0));
        assert_eq!(stats.term_hits[2], TermHits { text: "healthcheck".into(), state: TermState::Exclude, lines: 4 });
        assert_eq!(stats.summary(), "6 matches / 40 lines · 1 pinned (1 added) · 0 context");
    }

    #[test]
    fn summary_omits_added_and_groups_digits() {
        let stats = FilterStats { total_lines: 10_000_000, matched: 1_204, pinned: 0, pinned_unmatched: 0, context_lines: 12, term_hits: vec![], duration: Duration::ZERO };
        assert_eq!(stats.summary(), "1,204 matches / 10,000,000 lines · 0 pinned · 12 context");
        assert_eq!([group_digits(0), group_digits(999), group_digits(1000), group_digits(1234567)], ["0", "999", "1,000", "1,234,567"]);
    }
}
```

`history.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};

    fn entry(expr: &str) -> HistoryEntry {
        HistoryEntry::from_spec(&FilterSpec::from_expression(expr))
    }

    #[test]
    fn record_dedups_moves_to_front_and_caps() {
        let mut h = History::default();
        h.record(entry("a"));
        h.record(entry("b"));
        h.record(entry("a"));
        assert_eq!(h.entries().iter().map(|e| e.expression()).collect::<Vec<_>>(), ["a", "b"]);
        for i in 0..150 {
            h.record(entry(&format!("t{i}")));
        }
        assert_eq!(h.entries().len(), HISTORY_LIMIT);
        assert_eq!(h.entries()[0].expression(), "t149");
        h.record(HistoryEntry::from_spec(&FilterSpec::default()));
        assert_eq!(h.entries()[0].expression(), "t149", "empty filters are not recorded");
    }

    #[test]
    fn entries_restore_states_and_flags() {
        let spec = FilterSpec { terms: vec![Term::include("a"), Term::new("b", TermState::Exclude)], regex: true, case_sensitive: true, context: 2 };
        assert_eq!(HistoryEntry::from_spec(&spec).to_spec(), spec);
        let same_text_other_state = HistoryEntry::from_spec(&FilterSpec { terms: vec![Term::include("a"), Term::include("b")], ..spec.clone() });
        let mut h = History::default();
        h.record(HistoryEntry::from_spec(&spec));
        h.record(same_text_other_state);
        assert_eq!(h.entries().len(), 2, "different term states are different entries");
    }

    #[test]
    fn save_load_round_trip_missing_and_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/history.json");
        let missing = History::load(&path).unwrap();
        assert!(missing.history.entries().is_empty() && !missing.recovered);
        let mut h = History::default();
        h.record(entry("error || timeout"));
        h.save(&path).unwrap();
        assert_eq!(History::load(&path).unwrap().history, h);
        std::fs::write(&path, "{ not json").unwrap();
        let recovered = History::load(&path).unwrap();
        assert!(recovered.recovered && recovered.history.entries().is_empty());
        assert!(dir.path().join("nested/history.json.bak").exists());
    }
}
```

`paths.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_locations() {
        assert!(app_support_dir().ends_with("Library/Application Support/rogg"));
        assert!(cache_dir().ends_with("Library/Caches/rogg"));
        assert!(history_path().ends_with("Library/Application Support/rogg/history.json"));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-core stats` (and `history`, `paths`)
Expected: compile errors for the missing items.

- [ ] **Step 3: Implement the three modules**

`stats.rs`:
```rust
//! Filter statistics (spec §4.11).

use crate::engine::FilterResult;
use crate::filter::{FilterSpec, TermState};
use crate::view::FilteredView;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermHits {
    pub text: String,
    pub state: TermState,
    pub lines: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterStats {
    pub total_lines: u64,
    pub matched: u64,
    pub pinned: u64,
    pub pinned_unmatched: u64,
    pub context_lines: u64,
    pub term_hits: Vec<TermHits>,
    pub duration: Duration,
}

/// `1204` → `"1,204"`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl FilterStats {
    pub fn compute(spec: &FilterSpec, result: &FilterResult, view: &FilteredView) -> Self {
        let term_hits = spec
            .terms
            .iter()
            .enumerate()
            .map(|(i, t)| TermHits { text: t.text.clone(), state: t.state, lines: result.term_hits.get(i).copied().unwrap_or(0) })
            .collect();
        FilterStats {
            total_lines: result.total_lines,
            matched: result.matches.len() as u64,
            pinned: view.pinned_count(),
            pinned_unmatched: view.pinned_unmatched(),
            context_lines: view.context_lines(),
            term_hits,
            duration: result.duration,
        }
    }

    /// Footer text: `6 matches / 40 lines · 1 pinned (1 added) · 0 context`.
    pub fn summary(&self) -> String {
        let added = if self.pinned_unmatched > 0 { format!(" ({} added)", group_digits(self.pinned_unmatched)) } else { String::new() };
        format!(
            "{} matches / {} lines · {} pinned{} · {} context",
            group_digits(self.matched),
            group_digits(self.total_lines),
            group_digits(self.pinned),
            added,
            group_digits(self.context_lines)
        )
    }
}
```

`history.rs`:
```rust
//! Global filter history (spec §4.12), persisted as JSON with corruption recovery (spec §6.6).

use crate::error::CoreError;
use crate::filter::{FilterSpec, Term, join_expression};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HISTORY_LIMIT: usize = 100;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub terms: Vec<Term>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context: u32,
}

impl HistoryEntry {
    pub fn from_spec(spec: &FilterSpec) -> Self {
        Self { terms: spec.terms.clone(), regex: spec.regex, case_sensitive: spec.case_sensitive, context: spec.context }
    }

    pub fn to_spec(&self) -> FilterSpec {
        FilterSpec { terms: self.terms.clone(), regex: self.regex, case_sensitive: self.case_sensitive, context: self.context }
    }

    pub fn expression(&self) -> String {
        join_expression(&self.terms)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadOutcome {
    pub history: History,
    /// True when the file was unparseable and was moved aside to `<file>.bak`.
    pub recovered: bool,
}

#[derive(Serialize, Deserialize)]
struct HistoryFile {
    version: u32,
    entries: Vec<HistoryEntry>,
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

impl History {
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Adds `entry` at the front; an identical existing entry moves to the front; keeps the newest 100.
    pub fn record(&mut self, entry: HistoryEntry) {
        if entry.terms.is_empty() {
            return;
        }
        self.entries.retain(|e| e != &entry);
        self.entries.insert(0, entry);
        self.entries.truncate(HISTORY_LIMIT);
    }

    pub fn load(path: &Path) -> Result<LoadOutcome, CoreError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadOutcome { history: History::default(), recovered: false });
            }
            Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
        };
        match serde_json::from_str::<HistoryFile>(&text) {
            Ok(file) => {
                let mut history = History { entries: file.entries };
                history.entries.truncate(HISTORY_LIMIT);
                Ok(LoadOutcome { history, recovered: false })
            }
            Err(_) => {
                let backup = with_suffix(path, ".bak");
                std::fs::rename(path, &backup).map_err(CoreError::io(path))?;
                Ok(LoadOutcome { history: History::default(), recovered: true })
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
        }
        let json = serde_json::to_string_pretty(&HistoryFile { version: 1, entries: self.entries.clone() })
            .expect("history serializes");
        let partial = with_suffix(path, ".partial");
        std::fs::write(&partial, json).map_err(CoreError::io(&partial))?;
        std::fs::rename(&partial, path).map_err(CoreError::io(path))
    }
}
```

`paths.rs`:
```rust
//! macOS locations for rogg's data (spec §4.2, §4.12, §6).

use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `~/Library/Application Support/rogg`
pub fn app_support_dir() -> PathBuf {
    home_dir().join("Library/Application Support/rogg")
}

/// `~/Library/Caches/rogg`
pub fn cache_dir() -> PathBuf {
    home_dir().join("Library/Caches/rogg")
}

pub fn history_path() -> PathBuf {
    app_support_dir().join("history.json")
}
```
Add `pub mod history; pub mod paths; pub mod stats;` to `lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-core`
Expected: all rogg-core tests pass, including 2 stats, 3 history and 1 paths test.

- [ ] **Step 5: Commit**

```bash
git add crates/rogg-core
git commit -m "feat(core): filter statistics, persistent filter history and app paths

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: genlog — deterministic synthetic log generator

**Files:**
- Modify: `Cargo.toml` (add `"crates/rogg-genlog"` to members), `justfile`
- Create: `crates/rogg-genlog/Cargo.toml`, `crates/rogg-genlog/src/lib.rs`, `crates/rogg-genlog/src/main.rs`
- Test: unit tests in `lib.rs`

**Interfaces:**
- Produces (crate `rogg_genlog`):
  - `GenEncoding::{Utf8, Utf16Le}`
  - `GenOptions { size_bytes: u64, seed: u64, crlf: bool, long_line_every: Option<u64>, encoding: GenEncoding }` (Default: 1 MiB, seed 42, LF, no long lines, UTF-8)
  - `GenStats { lines: u64, bytes: u64 }`
  - `generate(&GenOptions, &mut impl Write) -> io::Result<GenStats>`
  - `generate_file(&Path, &GenOptions) -> io::Result<GenStats>`, which creates parent directories
  - `parse_size(&str) -> Option<u64>` (`"1G"`, `"512M"`, `"64K"`, `"1024"`, binary units, case-insensitive)
  - Binary: `genlog --size 1G --out FILE [--seed N] [--crlf] [--utf16le] [--long-every N]`

Lines look like the reference mockup: `2026-10-05 14:32:08.142  INFO   server       request started …`. Levels are weighted INFO 60 / DEBUG 25 / WARN 10 / ERROR 5. The vocabulary includes `timeout`, `retry`, `healthcheck`, `connection reset`, `request_id=req_xxxx` and `duration=NNNNms`, so filter benchmarks have realistic hit rates. Randomness comes from a built-in SplitMix64, so output is identical for the same seed on any machine and the crate needs no third-party dependencies. Generating zipped bundles from these files (spec §8) arrives with archive support in M3.

- [ ] **Step 1: Write the failing tests**

`crates/rogg-genlog/Cargo.toml`:
```toml
[package]
name = "rogg-genlog"
version.workspace = true
edition.workspace = true
publish.workspace = true

[[bin]]
name = "genlog"
path = "src/main.rs"
```
Add `"crates/rogg-genlog"` to the workspace members.

`crates/rogg-genlog/src/lib.rs` (tests first):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn gen(opts: &GenOptions) -> (Vec<u8>, GenStats) {
        let mut out = Vec::new();
        let stats = generate(opts, &mut out).unwrap();
        (out, stats)
    }

    #[test]
    fn deterministic_per_seed_and_sized() {
        let opts = GenOptions { size_bytes: 64 * 1024, ..Default::default() };
        let (a, sa) = gen(&opts);
        let (b, _) = gen(&opts);
        let (c, _) = gen(&GenOptions { seed: 7, ..opts.clone() });
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.len() as u64 >= opts.size_bytes && a.len() < opts.size_bytes as usize + 512);
        assert_eq!(sa.bytes, a.len() as u64);
        assert_eq!(sa.lines, a.iter().filter(|&&x| x == b'\n').count() as u64);
    }

    #[test]
    fn lines_follow_the_reference_format() {
        let (out, _) = gen(&GenOptions { size_bytes: 16 * 1024, ..Default::default() });
        let text = String::from_utf8(out).unwrap();
        for line in text.lines() {
            assert!(line.starts_with("2026-10-05 "), "{line}");
            assert_eq!(&line[23..25], "  ");
            let level = &line[25..30];
            assert!(["INFO ", "DEBUG", "WARN ", "ERROR"].contains(&level), "{line}");
        }
        assert!(text.contains("timeout") && text.contains("request_id=req_") && text.contains("healthcheck"));
    }

    #[test]
    fn crlf_utf16_and_long_lines() {
        let (crlf, _) = gen(&GenOptions { size_bytes: 8 * 1024, crlf: true, ..Default::default() });
        assert!(crlf.windows(2).filter(|w| w[1] == b'\n').all(|w| w[0] == b'\r'));
        let (utf16, stats) = gen(&GenOptions { size_bytes: 8 * 1024, encoding: GenEncoding::Utf16Le, ..Default::default() });
        assert_eq!(&utf16[..2], &[0xFF, 0xFE]);
        assert_eq!(utf16.len() % 2, 0);
        let decoded = String::from_utf16(&utf16[2..].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()).unwrap();
        assert_eq!(decoded.lines().count() as u64, stats.lines);
        let (long, _) = gen(&GenOptions { size_bytes: 300 * 1024, long_line_every: Some(10), ..Default::default() });
        assert!(long.split(|&b| b == b'\n').any(|l| l.len() > 100_000));
    }

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("1G"), Some(1 << 30));
        assert_eq!(parse_size("512m"), Some(512 << 20));
        assert_eq!(parse_size("64K"), Some(64 << 10));
        assert_eq!(parse_size("2GB"), Some(2 << 30));
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("lots"), None);
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p rogg-genlog`
Expected: compile errors for the missing items. (`main.rs` must exist for the crate to build. Create it with `fn main() {}` for now; Step 3 replaces it.)

- [ ] **Step 3: Implement**

`lib.rs`, above the tests:
```rust
//! Deterministic synthetic logs for tests and benchmarks (spec §8). No third-party dependencies.

use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenEncoding {
    Utf8,
    Utf16Le,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenOptions {
    pub size_bytes: u64,
    pub seed: u64,
    pub crlf: bool,
    /// Emit a ~100 KB line every N lines (exercises long-line handling).
    pub long_line_every: Option<u64>,
    pub encoding: GenEncoding,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self { size_bytes: 1 << 20, seed: 42, crlf: false, long_line_every: None, encoding: GenEncoding::Utf8 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenStats {
    pub lines: u64,
    pub bytes: u64,
}

/// SplitMix64: tiny, fast, and identical on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
}

const SOURCES: [&str; 12] =
    ["server", "router", "auth", "pool", "client", "healthcheck", "config", "queue", "worker", "circuit", "payments", "database"];
const TARGETS: [&str; 4] = ["payments-primary", "payments-secondary", "cache", "metrics"];
const METHODS: [&str; 3] = ["GET", "POST", "PUT"];
const PATHS: [&str; 4] = ["/v1/payments", "/v1/accounts", "/v1/authorize", "/status"];

fn level(rng: &mut Rng) -> &'static str {
    match rng.below(100) {
        0..60 => "INFO ",
        60..85 => "DEBUG",
        85..95 => "WARN ",
        _ => "ERROR",
    }
}

fn message(rng: &mut Rng) -> String {
    let req = rng.below(0x10000);
    match rng.below(10) {
        0 => format!("request started method={} path={} request_id=req_{req:04x}", rng.pick(&METHODS), rng.pick(&PATHS)),
        1 => format!("request completed status={} duration={}ms request_id=req_{req:04x}", [200, 200, 404, 502, 503][rng.below(5) as usize], rng.below(40_000)),
        2 => format!("retry attempt={} backoff={}ms target={}", 1 + rng.below(5), 250 << rng.below(4), rng.pick(&TARGETS)),
        3 => format!("upstream timeout after {}ms request_id=req_{req:04x} host={}.internal:8443", 1000 + rng.below(30_000), rng.pick(&TARGETS)),
        4 => format!("connection reset by peer upstream={} request_id=req_{req:04x}", rng.pick(&TARGETS)),
        5 => format!("healthcheck upstream={} status=ok latency={}ms", rng.pick(&TARGETS), rng.below(50)),
        6 => format!("job failed error=DEADLINE_EXCEEDED job_id=job_{} attempt={}", rng.below(10_000), 1 + rng.below(4)),
        7 => format!("acquired connection pool=primary active={} idle={}", rng.below(50), rng.below(20)),
        8 => format!("query completed rows={} duration={}ms", rng.below(1_000), rng.below(200)),
        _ => format!("flush completed counters={} histograms={} duration={}ms", rng.below(200), rng.below(40), rng.below(10)),
    }
}

/// Writes synthetic log lines until at least `size_bytes` bytes have been written.
pub fn generate(opts: &GenOptions, out: &mut impl Write) -> io::Result<GenStats> {
    let mut rng = Rng(opts.seed);
    let mut stats = GenStats::default();
    let mut clock_ms: u64 = (14 * 3600 + 32 * 60 + 8) * 1000 + 142;
    let mut line = String::with_capacity(256);
    let mut encoded = Vec::with_capacity(512);
    if opts.encoding == GenEncoding::Utf16Le {
        out.write_all(&[0xFF, 0xFE])?;
        stats.bytes += 2;
    }
    while stats.bytes < opts.size_bytes {
        clock_ms += rng.below(250);
        let day_ms = clock_ms % (24 * 3600 * 1000);
        let (h, m, s, ms) = (day_ms / 3_600_000, day_ms / 60_000 % 60, day_ms / 1000 % 60, day_ms % 1000);
        line.clear();
        use std::fmt::Write as _;
        let _ = write!(line, "2026-10-05 {h:02}:{m:02}:{s:02}.{ms:03}  {}  {:<12} {}", level(&mut rng), rng.pick(&SOURCES), message(&mut rng));
        if opts.long_line_every.is_some_and(|n| n > 0 && (stats.lines + 1) % n == 0) {
            line.push_str(" payload=");
            line.extend(std::iter::repeat_n('x', 100_000));
        }
        line.push_str(if opts.crlf { "\r\n" } else { "\n" });
        encoded.clear();
        match opts.encoding {
            GenEncoding::Utf8 => encoded.extend_from_slice(line.as_bytes()),
            GenEncoding::Utf16Le => encoded.extend(line.encode_utf16().flat_map(u16::to_le_bytes)),
        }
        out.write_all(&encoded)?;
        stats.bytes += encoded.len() as u64;
        stats.lines += 1;
    }
    Ok(stats)
}

pub fn generate_file(path: &Path, opts: &GenOptions) -> io::Result<GenStats> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = BufWriter::with_capacity(1 << 20, std::fs::File::create(path)?);
    let stats = generate(opts, &mut out)?;
    out.flush()?;
    Ok(stats)
}

/// `"1G"`, `"512M"`, `"64K"`, `"2GB"`, `"1024"` (binary units, case-insensitive).
pub fn parse_size(text: &str) -> Option<u64> {
    let t = text.trim().to_ascii_uppercase();
    let t = t.strip_suffix('B').unwrap_or(&t);
    let (digits, shift) = match t.chars().last()? {
        'K' => (&t[..t.len() - 1], 10),
        'M' => (&t[..t.len() - 1], 20),
        'G' => (&t[..t.len() - 1], 30),
        _ => (t, 0),
    };
    digits.parse::<u64>().ok().map(|n| n << shift)
}
```

`main.rs`:
```rust
use rogg_genlog::{GenEncoding, GenOptions, generate_file, parse_size};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let usage = "usage: genlog --size 1G --out FILE [--seed N] [--crlf] [--utf16le] [--long-every N]";
    let Some(out) = value("--out").map(PathBuf::from) else {
        eprintln!("{usage}");
        std::process::exit(2);
    };
    let size = value("--size").map(|s| parse_size(&s)).unwrap_or(Some(1 << 20));
    let Some(size_bytes) = size else {
        eprintln!("bad --size; {usage}");
        std::process::exit(2);
    };
    let opts = GenOptions {
        size_bytes,
        seed: value("--seed").and_then(|s| s.parse().ok()).unwrap_or(42),
        crlf: args.iter().any(|a| a == "--crlf"),
        long_line_every: value("--long-every").and_then(|s| s.parse().ok()),
        encoding: if args.iter().any(|a| a == "--utf16le") { GenEncoding::Utf16Le } else { GenEncoding::Utf8 },
    };
    match generate_file(&out, &opts) {
        Ok(stats) => println!("wrote {} lines, {} bytes to {}", stats.lines, stats.bytes, out.display()),
        Err(e) => {
            eprintln!("genlog: {e}");
            std::process::exit(1);
        }
    }
}
```

Append to `justfile`:
```just
genlog size="64M" out="target/genlog/sample.log":
    cargo run --release -p rogg-genlog -- --size {{size}} --out {{out}}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rogg-genlog && cargo run -q -p rogg-genlog -- --size 1M --out target/genlog/smoke.log`
Expected: 4 tests pass, and the binary prints `wrote N lines, 104857x bytes to target/genlog/smoke.log`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/rogg-genlog justfile
git commit -m "feat(genlog): deterministic synthetic log generator (lib + CLI)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Benchmarks against the spec targets

**Files:**
- Modify: `crates/rogg-core/Cargo.toml`, `justfile`
- Create: `crates/rogg-core/benches/engine.rs`, `crates/rogg-core/tests/memory.rs`, `docs/superpowers/reviews/2026-10-07-m1-benchmarks.md`

**Interfaces:**
- Consumes: `rogg_genlog::{generate_file, GenOptions, parse_size}` (Task 10); `LogText`, `CompiledFilter`, `run_filter`, `DEFAULT_CHUNK_LINES`, `CancelToken` (Tasks 1–6).
- Produces:
  - `just bench`, which runs criterion `engine/index`, `engine/filter_plain_4` and `engine/filter_regex_2` on a 1 GiB genlog file (cached under `target/bench-data/`), plus the ignored memory-budget test
  - a results document comparing each benchmark with its spec target

Targets (spec §8, this Mac):
- index 1 GB < 2 s
- 4 plain terms < 1.5 s
- 2 regex terms < 3 s
- index heap < 100 MB for the 1 GB file (`LineIndex::heap_bytes`, measured deterministically instead of RSS)

- [ ] **Step 1: Add bench and memory-test targets**

In `crates/rogg-core/Cargo.toml`, extend the existing `[dev-dependencies]` section and add a bench target:
```toml
[dev-dependencies]
proptest.workspace = true
tempfile.workspace = true
criterion.workspace = true
rogg-genlog = { path = "../rogg-genlog" }

[[bench]]
name = "engine"
harness = false
```

`crates/rogg-core/benches/engine.rs`:
```rust
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rogg_core::CancelToken;
use rogg_core::engine::{DEFAULT_CHUNK_LINES, run_filter};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term};
use rogg_core::text::LogText;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn data_file() -> PathBuf {
    let size = std::env::var("ROGG_BENCH_SIZE").unwrap_or_else(|_| "1G".into());
    let bytes = rogg_genlog::parse_size(&size).expect("ROGG_BENCH_SIZE like 1G or 256M");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/bench-data/genlog-{size}-seed42.log"));
    if !path.exists() {
        rogg_genlog::generate_file(&path, &rogg_genlog::GenOptions { size_bytes: bytes, seed: 42, ..Default::default() })
            .expect("generate bench data");
    }
    path
}

fn open(path: &Path) -> LogText {
    LogText::open(path, &std::env::temp_dir().join("rogg-bench-cache"), &CancelToken::new(), |_| {}).expect("open bench file")
}

fn engine(c: &mut Criterion) {
    let path = data_file();
    let len = std::fs::metadata(&path).unwrap().len();
    let mut group = c.benchmark_group("engine");
    group.sample_size(10).measurement_time(Duration::from_secs(30)).throughput(Throughput::Bytes(len));

    group.bench_function("index", |b| b.iter(|| open(&path)));

    let text = open(&path);
    let plain = CompiledFilter::compile(&FilterSpec {
        terms: ["timeout", "retry", "connection reset", "healthcheck"].map(Term::include).to_vec(),
        ..Default::default()
    })
    .unwrap();
    group.bench_function("filter_plain_4", |b| b.iter(|| run_filter(&text, &plain, DEFAULT_CHUNK_LINES, &CancelToken::new()).unwrap()));

    let regex = CompiledFilter::compile(&FilterSpec {
        terms: [r"request_id=req_[0-9a-f]{3}7", r"duration=\d{5,}ms"].map(Term::include).to_vec(),
        regex: true,
        ..Default::default()
    })
    .unwrap();
    group.bench_function("filter_regex_2", |b| b.iter(|| run_filter(&text, &regex, DEFAULT_CHUNK_LINES, &CancelToken::new()).unwrap()));
    group.finish();
}

criterion_group!(benches, engine);
criterion_main!(benches);
```

`crates/rogg-core/tests/memory.rs`:
```rust
use rogg_core::CancelToken;
use rogg_core::text::LogText;
use std::path::Path;

#[test]
#[ignore = "generates/uses a 1 GiB file; run via `just bench`"]
fn index_of_one_gigabyte_fits_the_memory_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/bench-data/genlog-1G-seed42.log");
    if !path.exists() {
        rogg_genlog::generate_file(&path, &rogg_genlog::GenOptions { size_bytes: 1 << 30, seed: 42, ..Default::default() }).unwrap();
    }
    let text = LogText::open(&path, &std::env::temp_dir(), &CancelToken::new(), |_| {}).unwrap();
    let heap = text.index().heap_bytes();
    println!("lines: {}, index heap: {:.1} MB", text.line_count(), heap as f64 / 1e6);
    assert!(text.line_count() > 5_000_000);
    assert!(heap < 100 * 1024 * 1024, "index heap {heap} bytes exceeds the 100 MB budget");
}
```

Append to `justfile`:
```just
bench:
    cargo bench -p rogg-core --bench engine
    cargo test -p rogg-core --release --test memory -- --ignored --nocapture
```

- [ ] **Step 2: Smoke-run on a small file**

Run: `ROGG_BENCH_SIZE=64M cargo bench -p rogg-core --bench engine -- --warm-up-time 1 --measurement-time 5`
Expected: three benchmarks report times, with no panic. This checks the harness before the slow 1 GiB run.

- [ ] **Step 3: Run the real benchmarks**

Run: `just bench`
Expected: the first run generates the 1 GiB file, which takes about a minute. Criterion prints mean times for `engine/index`, `engine/filter_plain_4` and `engine/filter_regex_2`, and the memory test prints the line count and index heap, then passes.

- [ ] **Step 4: Meet any missed target**

If any mean exceeds its target, optimize the responsible code and re-run until it passes. Commit each optimization separately, with its before/after numbers in the commit message, and keep every existing test and proptest green. Proven options in order of expected payoff:

**`index` over 2 s**
1. Try chunk sizes of 4, 8 and 32 MiB (`DEFAULT_CHUNK_BYTES`).
2. Avoid the per-chunk `Vec<u64>` of newline positions: count newlines first (`memchr::memchr_iter(..).count()`), then reserve `rel` once and write offsets straight into the index during stitching.
3. Make sure `shrink_to_fit` runs once, not per wave.

**`filter_plain_4` over 1.5 s**
1. Profile with `cargo bench … -- --profile-time 10` plus `samply`, if installed via `brew install samply`.
2. In `TermSet::mark` (Plain), replace `find_overlapping_iter` with leftmost-first `find_iter` for the *matched-line* decision. Count per-term hits only on matched lines, by running a second `find_overlapping_iter` over just those lines' bytes.
3. Replace the per-match `partition_point` with a forward cursor over `starts`, since match starts are almost sorted.

**`filter_regex_2` over 3 s**
1. In `TermSet::mark` (Regex), run each regex's `find_iter` over the whole chunk region instead of per line. Map match starts to lines with the cursor. Discard matches that contain a `\n`, and re-check those lines individually so semantics stay per-line.
2. Gate on a combined `RegexSet::is_match` over the region first, to skip chunks with no hits.

**Memory over 100 MB:** the index layout is 4 bytes per line plus 16 bytes per 4096 lines, about 40 MB for 10 M lines. Exceeding the budget means capacity wasn't shrunk; check that `shrink_to_fit` runs in `LogText::open`.

- [ ] **Step 5: Record the results**

`docs/superpowers/reviews/2026-10-07-m1-benchmarks.md`:
```markdown
# M1 benchmark results

Machine: <output of `sysctl -n machdep.cpu.brand_string`>, <`sysctl -n hw.ncpu`> cores, <`sysctl -n hw.memsize` in GB> GB RAM
Data: genlog 1 GiB, seed 42 (<lines> lines)
Command: `just bench`

| Benchmark | Mean | Throughput | Spec target | Pass |
|---|---|---|---|---|
| engine/index | <mean> | <GiB/s> | < 2 s | <✓/✗> |
| engine/filter_plain_4 | <mean> | <GiB/s> | < 1.5 s | <✓/✗> |
| engine/filter_regex_2 | <mean> | <GiB/s> | < 3 s | <✓/✗> |
| index heap (memory test) | <MB> | — | < 100 MB | <✓/✗> |

## Optimizations applied
<one line per optimization commit with before → after, or "none needed">
```
Fill every `<…>` with the measured values from the final `just bench` run. Commit only when every row passes.

- [ ] **Step 6: Run the whole workspace suite and commit**

Run: `cargo test --workspace`
Expected: all tests pass, including rogg-ui and rogg-gallery, unchanged by M1.

```bash
git add crates/rogg-core justfile docs/superpowers/reviews/2026-10-07-m1-benchmarks.md
git commit -m "test(core): criterion benchmarks and memory budget meeting spec targets

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

M1 is complete when `cargo test --workspace` and `just bench` pass and the results document shows every target met. M2 (main window) will connect `LogText`, `FilterJob`, `FilteredView`, `Highlighter` and `CompiledSearch` to the M0 components.
