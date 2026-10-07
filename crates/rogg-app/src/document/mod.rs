//! One open log: its source, its text once loaded, the cache entries it reads and the background jobs working on it.
//! Inputs up to `EngineConfig::sync_bytes` are handled on the UI thread; larger ones on background threads whose
//! channels one task polls every `POLL_INTERVAL` (spec §3.3).

mod filter;
mod load;
mod search;

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

pub use filter::FilterState;
pub use search::{Pane, PaneSearch};

/// How often background job channels are polled: at most one UI update per frame.
pub const POLL_INTERVAL: Duration = Duration::from_millis(16);

/// A `.gz` being decompressed is checked this often for new bytes.
pub const INFLATE_POLL: Duration = Duration::from_millis(100);

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

#[derive(Clone)]
pub enum LoadState {
    Loading { fraction: f32, label: SharedString },
    Ready(Arc<LogText>),
    Failed { message: SharedString },
}

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

impl Document {
    pub fn new(source: Source, init: DocumentInit, cx: &mut Context<Self>) -> Self {
        let mut doc = Self {
            source,
            load: LoadState::Loading { fraction: 0., label: "Opening".into() },
            filter: FilterState::new(init.spec, init.pins),
            search: Default::default(),
            number_base: init.number_base,
            text_revision: 0,
            load_revision: 0,
            open_job: None,
            leases: Vec::new(),
            inflating: None,
            inflated_len: None,
            follower: None,
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

    /// Bumped every time a text finishes loading and when it grows; tabs rebuild their rows when it changes.
    pub fn text_revision(&self) -> u64 {
        self.text_revision
    }

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
                self.finish_load(*result, cx);
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
}

impl Drop for Document {
    fn drop(&mut self) {
        if let Some(job) = self.filter.job.take() {
            filter::retire(job);
        }
    }
}
