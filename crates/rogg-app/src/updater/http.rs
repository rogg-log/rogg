//! Update traffic over ureq: the release manifest and the asset download. Blocking; callers run it off the UI
//! thread.

use rogg_core::CancelToken;
use rogg_core::update::Manifest;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://github.com/rogg-log/rogg/releases/latest/download";

/// A manifest check gives up after this long, end to end.
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(10);
/// A download fails after this long without receiving any data.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// The download thread's own cap on reading the whole body, so a read stuck after the caller gave up (a mid-body
/// stall, then a timeout or a cancel) ends eventually. The caller's 30 s idle limit is what fails a download; this
/// only bounds the abandoned thread. An hour is far more than a real download needs: the largest asset is about
/// 23 MB, which takes an hour only below about 6 KB/s, and the idle limit already fails a link that stops sending.
const BODY_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Progress is reported about every this many bytes.
const PROGRESS_STEP: u64 = 64 * 1024;
const CHUNK: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    Offline,
    TimedOut,
    Http(u16),
    BadManifest,
    Cancelled,
    Io(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Offline => f.write_str("no network connection"),
            NetError::TimedOut => f.write_str("timed out"),
            NetError::Http(status) => write!(f, "HTTP status {status}"),
            NetError::BadManifest => f.write_str("the release information couldn't be read"),
            NetError::Cancelled => f.write_str("cancelled"),
            NetError::Io(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for NetError {}

/// Where releases are fetched from: `ROGG_UPDATE_BASE` when set and non-empty, else GitHub's latest release.
pub fn base_url() -> String {
    match std::env::var("ROGG_UPDATE_BASE") {
        Ok(base) if !base.trim().is_empty() => base,
        _ => DEFAULT_BASE.to_string(),
    }
}

fn agent(configure: impl FnOnce(ureq::config::ConfigBuilder<ureq::typestate::AgentScope>) -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope>) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    let builder = ureq::Agent::config_builder().tls_config(tls);
    configure(builder).build().into()
}

fn map_ureq(e: &ureq::Error) -> NetError {
    match e {
        ureq::Error::StatusCode(status) => NetError::Http(*status),
        ureq::Error::Timeout(_) => NetError::TimedOut,
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => NetError::Offline,
        ureq::Error::Io(io) => map_io(io),
        other => NetError::Io(other.to_string()),
    }
}

fn map_io(e: &io::Error) -> NetError {
    if let Some(inner) = e.get_ref().and_then(|inner| inner.downcast_ref::<ureq::Error>()) {
        return map_ureq(inner);
    }
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => NetError::TimedOut,
        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound | io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable | io::ErrorKind::NetworkDown => {
            NetError::Offline
        }
        // The connection dropped mid-transfer (ureq reports an early close as `UnexpectedEof`): a network problem,
        // not a damaged download.
        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe => {
            NetError::Offline
        }
        _ => NetError::Io(e.to_string()),
    }
}

/// Fetches and parses `<base>/manifest.json`, giving up after 10 s.
pub fn fetch_manifest(base: &str) -> Result<Manifest, NetError> {
    let url = format!("{}/manifest.json", base.trim_end_matches('/'));
    let agent = agent(|b| b.timeout_global(Some(MANIFEST_TIMEOUT)));
    let mut response = agent.get(&url).call().map_err(|e| map_ureq(&e))?;
    let text = response.body_mut().read_to_string().map_err(|e| map_ureq(&e))?;
    Manifest::parse(&text).map_err(|_| NetError::BadManifest)
}

/// Downloads `url` to `dest` (through `dest` + `.part`, renamed on success). `on_progress(done, total)` is called
/// about every 64 KiB and once at the end. The download fails with `TimedOut` after 30 s without data.
pub fn download(url: &str, dest: &Path, on_progress: &mut dyn FnMut(u64, Option<u64>), cancel: &CancelToken) -> Result<(), NetError> {
    download_with_timeout(url, dest, on_progress, cancel, IDLE_TIMEOUT)
}

/// `download` with a chosen no-data timeout, so tests needn't wait 30 s.
///
/// ureq has no per-read timeout (`timeout_recv_body` caps the whole body and isn't restarted per read), so waiting
/// for the response headers uses `timeout_recv_response`, and the body is read on a helper thread whose chunks the
/// caller waits for with `idle` as the limit per chunk. A big download on a slow link therefore never times out
/// while data keeps flowing. The helper's body read is capped at an hour (`BODY_TIMEOUT`) only so that it ends.
#[doc(hidden)]
pub fn download_with_timeout(
    url: &str,
    dest: &Path,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: &CancelToken,
    idle: Duration,
) -> Result<(), NetError> {
    let part = part_path(dest);
    let result = fetch_to(url, &part, on_progress, cancel, idle);
    match result {
        Ok(()) => fs::rename(&part, dest).map_err(|e| {
            let _ = fs::remove_file(&part);
            NetError::Io(e.to_string())
        }),
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// What the helper thread reports: the response head, then body chunks.
enum Msg {
    Head(Option<u64>),
    Chunk(Vec<u8>),
}

/// How often the caller looks at the cancel token while waiting for the helper.
const POLL: Duration = Duration::from_millis(100);

fn fetch_to(url: &str, part: &Path, on_progress: &mut dyn FnMut(u64, Option<u64>), cancel: &CancelToken, idle: Duration) -> Result<(), NetError> {
    if cancel.is_cancelled() {
        return Err(NetError::Cancelled);
    }
    // The request and the body read both block, so they run on a helper thread; the caller polls in short slices
    // so a cancel takes effect quickly in either phase. If the caller gives up, the helper ends when its next send
    // fails or its read returns.
    let (tx, rx) = mpsc::sync_channel::<Result<Msg, NetError>>(4);
    let url = url.to_string();
    thread::spawn(move || {
        let agent = agent(|b| {
            b.timeout_connect(Some(MANIFEST_TIMEOUT)).timeout_recv_response(Some(idle)).timeout_recv_body(Some(BODY_TIMEOUT))
        });
        let response = match agent.get(&url).call() {
            Ok(response) => response,
            Err(e) => {
                let _ = tx.send(Err(map_ureq(&e)));
                return;
            }
        };
        let body = response.into_body();
        if tx.send(Ok(Msg::Head(body.content_length()))).is_err() {
            return;
        }
        let mut reader = body.into_reader();
        loop {
            let mut buf = vec![0u8; CHUNK];
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf.truncate(n);
                    if tx.send(Ok(Msg::Chunk(buf))).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(map_io(&e)));
                    return;
                }
            }
        }
    });

    let mut file: Option<File> = None;
    let mut total = None;
    let mut done = 0u64;
    let mut reported = 0u64;
    let mut waited = Duration::ZERO;
    loop {
        if cancel.is_cancelled() {
            return Err(NetError::Cancelled);
        }
        match rx.recv_timeout(POLL) {
            Ok(Ok(Msg::Head(length))) => {
                waited = Duration::ZERO;
                total = length;
                file = Some(File::create(part).map_err(|e| NetError::Io(e.to_string()))?);
            }
            Ok(Ok(Msg::Chunk(chunk))) => {
                waited = Duration::ZERO;
                let Some(file) = file.as_mut() else { return Err(NetError::Io("no response head".into())) };
                file.write_all(&chunk).map_err(|e| NetError::Io(e.to_string()))?;
                done += chunk.len() as u64;
                if done - reported >= PROGRESS_STEP {
                    reported = done;
                    on_progress(done, total);
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                waited += POLL;
                if waited >= idle {
                    return Err(NetError::TimedOut);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let Some(mut file) = file else { return Err(NetError::Io("no response".into())) };
    // The body ended short of its length: the connection closed early.
    if total.is_some_and(|total| done != total) {
        return Err(NetError::Offline);
    }
    file.flush().map_err(|e| NetError::Io(e.to_string()))?;
    if done != reported {
        on_progress(done, total);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_connection_is_offline() {
        for kind in [io::ErrorKind::ConnectionReset, io::ErrorKind::ConnectionAborted, io::ErrorKind::UnexpectedEof, io::ErrorKind::BrokenPipe] {
            assert_eq!(map_io(&io::Error::from(kind)), NetError::Offline, "{kind:?}");
        }
        // A local file error stays one.
        assert!(matches!(map_io(&io::Error::from(io::ErrorKind::PermissionDenied)), NetError::Io(_)));
    }
}
