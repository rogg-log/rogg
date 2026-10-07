//! "Open filtered as tab" (spec §4.9, §5.6): a tab whose text is a frozen copy of another tab's filtered rows, numbered
//! from 1 like an ordinary file. Built on a background thread (`materialize`, or `rebuild` from a session).

use rogg_core::cache::{Cache, CacheLease};
use rogg_core::derived::{Derived, materialize, rebuild};
use rogg_core::session::{DerivedSpec, SessionSource};
use rogg_core::text::LogText;
use rogg_core::{CancelToken, CoreError};
use gpui_kit::SharedString;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub enum DerivedOrigin {
    /// From an open tab: its text and the lines its filtered log showed.
    Lines { parent: Arc<LogText>, lines: Arc<Vec<u64>> },
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
                DerivedOrigin::Lines { parent, lines } => materialize(parent, lines, &cache, &token, report).map(|d| (d, Vec::new())),
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
