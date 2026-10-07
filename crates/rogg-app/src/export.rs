//! Exporting filtered lines on a background thread (spec §6.5); the document polls it and reports the result.

use rogg_core::CancelToken;
use rogg_core::export::{ExportOptions, export_lines};
use rogg_core::text::LogText;
use rogg_core::view::FilteredView;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

enum Message {
    Progress(f32),
    Done(Result<u64, String>),
}

pub struct ExportJob {
    cancel: CancelToken,
    rx: Receiver<Message>,
    pub dst: PathBuf,
    pub progress: f32,
}

impl ExportJob {
    pub fn spawn(text: Arc<LogText>, view: Arc<FilteredView>, base: u64, options: ExportOptions, dst: PathBuf) -> Self {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let path = dst.clone();
        let _detached = std::thread::Builder::new().name("rogg-export".into()).spawn(move || {
            let progress = tx.clone();
            let result = export_lines(&text, &view, |l| base + l, options, &path, &token, |done, total| {
                let _ = progress.send(Message::Progress(if total == 0 { 1. } else { done as f32 / total as f32 }));
            });
            let _ = tx.send(Message::Done(result.map_err(|e| e.to_string())));
        });
        Self { cancel, rx, dst, progress: 0. }
    }

    /// None while running; the outcome once finished (a dead worker is an error).
    pub fn poll(&mut self) -> Option<Result<u64, String>> {
        loop {
            match self.rx.try_recv() {
                Ok(Message::Progress(f)) => self.progress = f,
                Ok(Message::Done(result)) => return Some(result),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => return Some(Err("export stopped unexpectedly".into())),
            }
        }
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
