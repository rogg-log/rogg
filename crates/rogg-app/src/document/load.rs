//! Opening a large file on a background thread; the document polls the channel.

use rogg_core::text::{LogText, OpenProgress};
use rogg_core::{CancelToken, CoreError};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum Message {
    Progress(OpenProgress),
    Done(Result<LogText, CoreError>),
}

pub(crate) enum OpenPoll {
    /// Still working; the latest progress since the last poll, if any.
    Pending(Option<OpenProgress>),
    Done(Result<LogText, CoreError>),
    /// The thread ended without a result (it panicked or could not be spawned).
    Died,
}

pub(crate) struct OpenJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
}

impl OpenJob {
    pub(crate) fn spawn(path: PathBuf, cache: PathBuf) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        // Detached: on cancel, `LogText::open` returns at its next check and the thread ends.
        let _detached = std::thread::Builder::new().name("rogg-open".into()).spawn(move || {
            let progress = tx.clone();
            let result = LogText::open(&path, &cache, &token, |p| {
                let _ = progress.send(Message::Progress(p));
            });
            let _ = tx.send(Message::Done(result));
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
