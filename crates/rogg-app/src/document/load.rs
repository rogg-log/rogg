//! Opening a source on a background thread — extraction, decompression, conversion, indexing; the document polls the
//! channel.

use rogg_core::cache::Cache;
use rogg_core::source::{OpenUpdate, OpenedSource, SourceRef, open_source};
use rogg_core::{CancelToken, CoreError};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// The result of a finished open, boxed so the channel messages stay small.
pub(crate) type OpenResult = Box<Result<OpenedSource, CoreError>>;

enum Message {
    Progress(OpenUpdate),
    Done(OpenResult),
}

pub(crate) enum OpenPoll {
    /// Still working; the latest progress since the last poll, if any.
    Pending(Option<OpenUpdate>),
    Done(OpenResult),
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
            let _ = tx.send(Message::Done(Box::new(result)));
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
