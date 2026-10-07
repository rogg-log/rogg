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
/// Upper bound on the bytes in one chunk (a chunk always holds at least one line), so progress,
/// parallelism and cancellation stay responsive when lines are very long.
pub const MAX_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

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

/// Splits `lines` into ascending, contiguous chunks of at most `chunk_lines` lines and at most
/// `MAX_CHUNK_BYTES` bytes each (but never fewer than one line).
pub(crate) fn chunk_ranges(text: &LogText, lines: Range<u64>, chunk_lines: u64) -> Vec<Range<u64>> {
    let index = text.index();
    let offset = |line: u64| if line < index.line_count() { index.line_start(line) } else { index.data_len() };
    let mut ranges = Vec::new();
    let mut start = lines.start;
    while start < lines.end {
        let first = start + 1;
        let max_end = start.saturating_add(chunk_lines).min(lines.end).max(first);
        let base = offset(start);
        // Largest end in `first..=max_end` whose byte span fits; `first` is always accepted.
        let (mut lo, mut hi) = (first, max_end);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if offset(mid) - base <= MAX_CHUNK_BYTES {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        ranges.push(start..lo);
        start = lo;
    }
    ranges
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
    let ranges = chunk_ranges(text, lines.clone(), chunk_lines.max(1));
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

/// A filter scan on a background thread; the receiver yields `FilterEvent`s as chunks complete.
pub struct FilterJob {
    cancel: CancelToken,
    events: mpsc::Receiver<FilterEvent>,
    thread: Option<JoinHandle<()>>,
}

impl FilterJob {
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
