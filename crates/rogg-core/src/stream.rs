//! stdin and FIFOs (spec §4.1): data that arrives over time is copied into a cache file as it comes, and the tab
//! follows that file like a growing log.

use crate::cache::Cache;
use crate::error::CoreError;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::RawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Whether `fd` carries data to read: a pipe, FIFO, socket or regular file — not a terminal, `/dev/null` or other
/// device (an app started from Finder has `/dev/null` on stdin).
pub fn fd_has_data(fd: RawFd) -> bool {
    // SAFETY: `fstat` only writes into `st`; an invalid fd returns an error.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return false;
    }
    matches!(st.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK | libc::S_IFREG)
}

/// stdin when something is piped or redirected into rogg.
pub fn stdin_has_data() -> bool {
    fd_has_data(libc::STDIN_FILENO)
}

/// A FIFO or socket on disk (`mkfifo`, `<(cmd)` → `/dev/fd/63`): read it as a stream, never map it.
pub fn is_stream_path(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.file_type().is_fifo() || m.file_type().is_socket())
}

pub struct StreamCapture {
    path: PathBuf,
    bytes: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

impl StreamCapture {
    /// Creates `streams/stream-<pid>-<n>.log` and copies the input opened by `open` into it on a background thread,
    /// writing each read through at once so a follower sees it. `open` runs on that thread: opening a FIFO blocks
    /// until a writer appears.
    pub fn start<R: Read + Send + 'static>(open: impl FnOnce() -> io::Result<R> + Send + 'static, cache: &Cache) -> Result<StreamCapture, CoreError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = cache.streams_dir();
        std::fs::create_dir_all(&dir).map_err(CoreError::io(&dir))?;
        let path = dir.join(format!("stream-{}-{}.log", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
        let mut out = File::create(&path).map_err(CoreError::io(&path))?;
        let capture = StreamCapture {
            path,
            bytes: Arc::new(AtomicU64::new(0)),
            done: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
        };
        let (bytes, done, error) = (capture.bytes.clone(), capture.done.clone(), capture.error.clone());
        let spawned = std::thread::Builder::new().name("rogg-stream".into()).spawn(move || {
            let result = (|| -> io::Result<()> {
                let mut input = open()?;
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let n = match input.read(&mut buf) {
                        Ok(0) => return Ok(()),
                        Ok(n) => n,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(e),
                    };
                    out.write_all(&buf[..n])?;
                    bytes.fetch_add(n as u64, Ordering::Relaxed);
                }
            })();
            if let Err(e) = result {
                *error.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
            }
            done.store(true, Ordering::Release);
        });
        spawned.map_err(|source| CoreError::Io { path: capture.path.clone(), source })?;
        Ok(capture)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bytes captured so far.
    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    /// The input ended (or failed).
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}
