//! Derived sources (spec §4.9): a filtered view frozen into a text of its own. The shown lines are copied into a cache
//! file (`streams/derived-<pid>-<n>.log`) that the tab maps like any log and numbers from 1, as an ordinary file
//! (spec §4.9). Copying lets scans, search, export, nesting and session rebuilds work unchanged, at the cost of disk
//! space for the shown lines.

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
/// Progress reports lines copied of the total.
pub fn materialize(
    text: &LogText,
    lines: &[u64],
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
    Ok(Derived { path, text })
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
    rebuild_level(spec, cache, cancel, &mut progress)
}

/// `rebuild` over a `dyn` progress callback: a generic one would instantiate itself once per nesting level.
fn rebuild_level(spec: &DerivedSpec, cache: &Arc<Cache>, cancel: &CancelToken, progress: &mut dyn FnMut(u64, u64)) -> Result<Rebuilt, CoreError> {
    let (parent, leases, temp) = match &spec.parent {
        SessionSource::File { path } => {
            let (text, leases) = open_full(&SourceRef::File(path.clone()), cache, cancel)?;
            (text, leases, None)
        }
        SessionSource::Member { source } => {
            let (text, leases) = open_full(source, cache, cancel)?;
            (text, leases, None)
        }
        SessionSource::Derived(inner) => {
            let r = rebuild_level(inner, cache, cancel, &mut |_, _| {})?;
            (r.derived.text, r.leases, Some(r.derived.path))
        }
    };
    let _temp = temp.map(TempFile);
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
    Ok(Rebuilt { derived: materialize(&parent, &lines, cache, cancel, progress)?, leases })
}

/// An intermediate level's file, removed on every exit of the rebuild (success, error or cancellation).
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
