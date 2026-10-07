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
    /// The text `result` is complete for.
    covered: Option<Arc<LogText>>,
    /// The text the running count reads.
    job_text: Option<Arc<LogText>>,
    /// Lines found before a running tail count's first line.
    tail_base: Option<Vec<u64>>,
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
        s.tail_base = None;
        s.covered = None;
        s.revision += 1;
        let mut spawned = false;
        if let (Some(text), Some(search)) = (text, s.compiled.clone()) {
            if text.bytes().len() as u64 <= sync_bytes {
                s.result = search.run(&text, &scope, DEFAULT_CHUNK_LINES, &CancelToken::new()).ok();
                if s.result.is_some() {
                    s.covered = Some(text.clone());
                }
            } else {
                s.job_text = Some(text.clone());
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
                Err(TryRecvError::Empty) => active = true,
                Err(TryRecvError::Disconnected) => {
                    s.job = None;
                    s.job_text = None;
                    s.tail_base = None;
                    changed = true;
                }
            }
        }
        if changed {
            self.grow_search(cx);
            cx.notify();
        }
        active
    }

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

    /// The next (or previous) line with a hit after 0-based `from`, wrapping; the first hit when `from` is None.
    pub fn next_hit(&self, pane: Pane, from: Option<u64>, backwards: bool) -> Option<u64> {
        let result = self.search(pane).result.as_ref()?;
        if backwards { result.prev_before(from) } else { result.next_after(from) }
    }
}
