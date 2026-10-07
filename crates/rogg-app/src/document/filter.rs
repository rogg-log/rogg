//! A document's filter pipeline (spec §4.6–4.8, §4.11): compiles the spec, scans (synchronously for small
//! texts, as a `FilterJob` otherwise), republishes progressive results, and composes the filtered view from
//! matches, pins and context without rescanning.

use super::{Document, EngineConfig};
use crate::history::HistoryStore;
use gpui_kit::{BorrowAppContext, Context, SharedString};
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

/// A publish rebuilds the view on the UI thread, so the next one waits at least this many times as long as the
/// last rebuild took: publishing never takes more than a fifth of the UI thread, however large the matches grow.
const REBUILD_COST_FACTOR: u32 = 4;

/// Whether partial results should be republished now: matches arrived since the last publish and both the
/// minimum interval and the cost-proportional pause have elapsed.
fn publish_due(dirty: bool, since_last_publish: Option<Duration>, last_rebuild: Duration) -> bool {
    dirty && since_last_publish.is_none_or(|elapsed| elapsed >= PUBLISH_INTERVAL.max(last_rebuild * REBUILD_COST_FACTOR))
}

pub struct FilterState {
    /// The applied filter: what the chips, flags and context select show.
    pub spec: FilterSpec,
    /// Why the last change was rejected (shown as `Previous results retained · …`).
    pub error: Option<SharedString>,
    compiled: Option<Arc<CompiledFilter>>,
    pub highlighter: Option<Arc<Highlighter>>,
    pub(super) job: Option<FilterJob>,
    partial: Vec<u64>,
    last_publish: Option<Instant>,
    /// Matches arrived since the last publish.
    dirty: bool,
    /// How long the last progressive rebuild took.
    last_rebuild: Duration,
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
            dirty: false,
            last_rebuild: Duration::ZERO,
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
            if spec.context != old.context && spec.is_active() && cx.has_global::<HistoryStore>() {
                cx.update_global::<HistoryStore, _>(|history, _| history.record_context(&spec));
            }
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
        self.filter.dirty = false;
        self.filter.last_rebuild = Duration::ZERO;
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
        let progress_before = self.filter.progress;
        loop {
            match job.events().try_recv() {
                Ok(FilterEvent::Chunk { matches, .. }) => {
                    self.filter.dirty |= !matches.is_empty();
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
        if publish_due(self.filter.dirty, self.filter.last_publish.map(|t| t.elapsed()), self.filter.last_rebuild) {
            let started = Instant::now();
            self.filter.matches = Arc::new(self.filter.partial.clone());
            self.rebuild_view(cx);
            self.filter.last_rebuild = started.elapsed();
            self.filter.last_publish = Some(Instant::now());
            self.filter.dirty = false;
        } else if self.filter.progress != progress_before {
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
        self.filter.dirty = false;
        self.rebuild_view(cx);
    }

    /// Recomposes the filtered view and statistics from the current matches, pins and context. While a scan runs
    /// the view is built without context (and without the exclusion check), so a progressive publish stays cheap;
    /// the final view of `finish_scan` applies the real context.
    pub(super) fn rebuild_view(&mut self, cx: &mut Context<Self>) {
        let total = self.text().map_or(0, |t| t.line_count());
        let f = &self.filter;
        let context = if f.progress.is_some() { 0 } else { f.spec.context };
        let view = match (self.text(), &f.compiled) {
            (Some(text), Some(filter)) if context > 0 => {
                FilteredView::build(f.matches.clone(), &f.pins, context, total, |n| filter.is_excluded(text.line(n)))
            }
            _ => FilteredView::build(f.matches.clone(), &f.pins, context, total, |_| false),
        };
        let stats = FilterStats::from_parts(&f.spec, f.matches.len() as u64, &f.term_hits, total, f.duration, &view);
        self.filter.view = Arc::new(view);
        self.filter.stats = Some(stats);
        self.filter.revision += 1;
        self.refresh_search(super::Pane::Filtered, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn publishing_waits_for_new_matches_the_interval_and_four_times_the_last_rebuild() {
        assert!(publish_due(true, None, MS(500)), "the first publish is immediate");
        assert!(!publish_due(false, Some(MS(10_000)), Duration::ZERO), "nothing new, nothing to publish");
        assert!(!publish_due(true, Some(MS(99)), Duration::ZERO));
        assert!(publish_due(true, Some(MS(100)), Duration::ZERO));
        // A 60 ms rebuild pushes the next publish to 240 ms, so slow publishes never run back to back.
        assert!(!publish_due(true, Some(MS(239)), MS(60)));
        assert!(publish_due(true, Some(MS(240)), MS(60)));
    }
}
