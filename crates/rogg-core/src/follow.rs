//! Live follow (spec §4.5): a thread that watches a text's file and reports growth (a new snapshot indexing only the
//! appended bytes), truncation and replacement. FSEvents through `notify` wakes it at once; a timer covers file
//! systems that send no events (network mounts) and files rogg writes itself.

use crate::cancel::CancelToken;
use crate::error::CoreError;
use crate::text::{LogText, Refresh};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
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
    crate::fsx::path_identity(path).is_some_and(|now| now != identity)
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
