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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    #[test]
    fn dropping_the_job_cancels_it_and_leaves_no_file() {
        let n = 2_000_000u64;
        let text = Arc::new(LogText::from_bytes("line\n".repeat(n as usize).into_bytes()));
        let view = Arc::new(FilteredView::build(Arc::new((0..n).collect()), &BTreeSet::new(), 0, n, |_| false));
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("out.log");
        let options = ExportOptions { context: true, gaps: true, numbers: true };
        drop(ExportJob::spawn(text.clone(), view, 1, options, dst.clone()));
        // The worker holds the other reference to the text until it has finished.
        let deadline = Instant::now() + Duration::from_secs(30);
        while Arc::strong_count(&text) > 1 {
            assert!(Instant::now() < deadline, "the export did not stop");
            std::thread::sleep(Duration::from_millis(5));
        }
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert!(left.is_empty(), "neither the destination nor a partial file remains: {left:?}");
    }
}
