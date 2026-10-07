//! One open log: its source, its text once loaded, and the background jobs working on it. Inputs up to
//! `EngineConfig::sync_bytes` are handled on the UI thread; larger ones on background threads whose channels
//! one task polls every `POLL_INTERVAL` (spec §3.3).

mod filter;
mod load;

use crate::paths::AppPaths;
use gpui_kit::{App, Context, Global, SharedString};
use load::{OpenJob, OpenPoll};
use rogg_core::filter::FilterSpec;
use rogg_core::text::{LogText, OpenProgress};
use rogg_core::{CancelToken, CoreError};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub use filter::FilterState;

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

pub struct Document {
    pub source: Source,
    pub load: LoadState,
    pub filter: FilterState,
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
            filter: FilterState::new(init.spec, init.pins),
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
        if matches!(self.load, LoadState::Ready(_)) {
            self.start_filter(cx);
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
        let active = self.poll_open(cx) | self.poll_filter(cx);
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

impl Drop for Document {
    fn drop(&mut self) {
        if let Some(job) = self.filter.job.take() {
            filter::retire(job);
        }
    }
}
