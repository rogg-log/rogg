//! What a tab shows and where it comes from (spec §4.1): a file, or a member of an archive (possibly nested).

use crate::archive::{ArchiveEntry, Content, extract, gunzip, list, sniff};
use crate::cache::{Cache, CacheLease, origin_key, partial_path};
use crate::cancel::CancelToken;
use crate::encoding::Encoding;
use crate::error::CoreError;
use crate::text::{LogText, OpenProgress};
use std::fs::File;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceRef {
    File(PathBuf),
    /// `path` (`/`-separated) inside the archive `archive`, which is itself a file or a member.
    Member { archive: Box<SourceRef>, path: String },
}

impl SourceRef {
    pub fn member(archive: SourceRef, path: impl Into<String>) -> SourceRef {
        SourceRef::Member { archive: Box::new(archive), path: path.into() }
    }

    /// The file on disk everything comes from (the outermost archive of a member).
    pub fn root_path(&self) -> &Path {
        match self {
            SourceRef::File(path) => path,
            SourceRef::Member { archive, .. } => archive.root_path(),
        }
    }

    /// Member paths from the outermost archive inwards; empty for a file.
    pub fn member_chain(&self) -> Vec<&str> {
        match self {
            SourceRef::File(_) => Vec::new(),
            SourceRef::Member { archive, path } => {
                let mut chain = archive.member_chain();
                chain.push(path.as_str());
                chain
            }
        }
    }

    /// The file's or member's own name.
    pub fn name(&self) -> String {
        match self {
            SourceRef::File(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string()),
            SourceRef::Member { path, .. } => path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path.as_str()).to_string(),
        }
    }

    /// The info-bar breadcrumb (spec §5.3): a file's folder and name; for a member, the outer archive's name, each
    /// nested archive and folder, and the member's name (`incident.zip › logs › api.log`).
    pub fn crumbs(&self) -> Vec<String> {
        match self {
            SourceRef::File(path) => {
                let mut crumbs: Vec<String> = path.parent().and_then(Path::file_name).map(|d| d.to_string_lossy().into_owned()).into_iter().collect();
                crumbs.push(self.name());
                crumbs
            }
            SourceRef::Member { archive, path } => {
                let mut crumbs = match archive.as_ref() {
                    SourceRef::File(_) => vec![archive.name()],
                    member => member.crumbs(),
                };
                crumbs.extend(path.split('/').filter(|s| !s.is_empty()).map(str::to_string));
                crumbs
            }
        }
    }

    /// Tab tooltip: a file's full path; a member as `api.log · incident.zip` (the reference's tab title).
    pub fn tooltip(&self) -> String {
        match self {
            SourceRef::File(path) => path.display().to_string(),
            SourceRef::Member { .. } => {
                let root = self.root_path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                format!("{} · {root}", self.name())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenStage {
    Extracting,
    Decompressing,
    Transcoding,
    Indexing,
}

/// Progress of `open_source`: the stage, the file it concerns (Extracting / Decompressing) and bytes done of total.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenUpdate {
    pub stage: OpenStage,
    pub name: String,
    pub done: u64,
    pub total: u64,
}

pub struct OpenedSource {
    pub text: LogText,
    /// Cache entries the text reads from; keep them while the text is in use (eviction skips them).
    pub leases: Vec<CacheLease>,
    /// A `.gz` still decompressing into the file the text maps: the text can grow as it proceeds (Task 7 `refresh`).
    pub inflating: Option<Inflating>,
    /// Plain files on disk: the path live follow watches (spec §4.5).
    pub watch: Option<PathBuf>,
}

fn update(stage: OpenStage, name: &str, done: u64, total: u64) -> OpenUpdate {
    OpenUpdate { stage, name: name.to_string(), done, total }
}

/// Opens `src` for viewing (spec §4.1): extracts archive members into the cache (outer archives first), decompresses
/// `.gz` — progressively: the returned text covers what is decompressed so far and `inflating` keeps going —, converts
/// UTF-16, and indexes. Opening an archive itself is an error (its members open from the sidebar).
pub fn open_source(src: &SourceRef, cache: &Arc<Cache>, cancel: &CancelToken, mut progress: impl FnMut(OpenUpdate)) -> Result<OpenedSource, CoreError> {
    let mut leases = Vec::new();
    let raw = raw_file(src, cache, cancel, &mut progress, &mut leases)?;
    let name = src.name();
    match sniff(&raw).map_err(CoreError::io(&raw))? {
        Content::Archive(_) => Err(CoreError::Archive { path: raw, message: format!("{name} is an archive; open its members from the sidebar") }),
        Content::Gzip => open_gzip(src, &raw, &name, cache, cancel, &mut progress, leases),
        Content::Text | Content::Binary => {
            let text = open_text(File::open(&raw).map_err(CoreError::io(&raw))?, &raw, cache, cancel, &mut progress, &mut leases)?;
            let watch = matches!(src, SourceRef::File(_)).then(|| raw.clone());
            Ok(OpenedSource { text, leases, inflating: None, watch })
        }
    }
}

/// Lists the archive `src`, extracting it into the cache first when it is itself a member (nested archives).
pub fn list_archive(src: &SourceRef, cache: &Arc<Cache>, cancel: &CancelToken) -> Result<Vec<ArchiveEntry>, CoreError> {
    let mut leases = Vec::new();
    let path = raw_file(src, cache, cancel, &mut |_: OpenUpdate| {}, &mut leases)?;
    match sniff(&path).map_err(CoreError::io(&path))? {
        Content::Archive(kind) => list(&path, kind, cancel, |_, _| {}),
        _ => Err(CoreError::Archive { path, message: format!("{} is not an archive", src.name()) }),
    }
}

/// The file holding `src`'s raw bytes: the file itself, or the member extracted into the cache.
fn raw_file(
    src: &SourceRef,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    leases: &mut Vec<CacheLease>,
) -> Result<PathBuf, CoreError> {
    let SourceRef::Member { archive, path } = src else { return Ok(src.root_path().to_path_buf()) };
    let container = raw_file(archive, cache, cancel, progress, leases)?;
    let kind = match sniff(&container).map_err(CoreError::io(&container))? {
        Content::Archive(kind) => kind,
        _ => return Err(CoreError::Archive { path: container, message: format!("{} is not an archive", archive.name()) }),
    };
    let root = src.root_path();
    let dst = cache.entry(origin_key(root, &src.member_chain(), "member").map_err(CoreError::io(root))?, "member");
    leases.push(cache.lease(&dst));
    if !dst.exists() {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let name = src.name();
        cache.write_entry(&dst, |file| {
            extract(&container, kind, path, file, cancel, |done, total| progress(update(OpenStage::Extracting, &name, done, total))).map(drop)
        })?;
    }
    Ok(dst)
}

/// Opens a text file, leasing its UTF-8 conversion when one was made in the cache.
fn open_text(
    file: File,
    path: &Path,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    leases: &mut Vec<CacheLease>,
) -> Result<LogText, CoreError> {
    let text = LogText::open_handle(file, path, cache.dir(), cancel, |p| {
        progress(match p {
            OpenProgress::Transcoding { done, total } => update(OpenStage::Transcoding, "", done, total),
            OpenProgress::Indexing { done, total } => update(OpenStage::Indexing, "", done, total),
        })
    })?;
    if let Some(backing) = text.backing_path()
        && backing != path
    {
        leases.push(cache.lease(backing));
    }
    Ok(text)
}

fn open_gzip(
    src: &SourceRef,
    raw: &Path,
    name: &str,
    cache: &Arc<Cache>,
    cancel: &CancelToken,
    progress: &mut impl FnMut(OpenUpdate),
    mut leases: Vec<CacheLease>,
) -> Result<OpenedSource, CoreError> {
    let root = src.root_path();
    let dst = cache.entry(origin_key(root, &src.member_chain(), "gunzip").map_err(CoreError::io(root))?, "log");
    leases.push(cache.lease(&dst));
    if !dst.exists() {
        std::fs::create_dir_all(cache.dir()).map_err(CoreError::io(cache.dir()))?;
        let mut inflating = Inflating::start(raw.to_path_buf(), dst.clone())?;
        let mut report = |read: u64, total: u64| progress(update(OpenStage::Decompressing, name, read, total));
        // A UTF-16 file must be complete before it can be converted; UTF-8 is shown while it decompresses.
        let head = inflating.wait_for_head(4, cancel, &mut report)?;
        if Encoding::detect(&head).is_utf16() {
            inflating.wait_done(cancel, &mut report)?;
        } else if !inflating.finished {
            let reader = inflating.reader.take().expect("the reader is taken once");
            let text = open_text(reader, inflating.partial(), cache, cancel, progress, &mut leases)?;
            return Ok(OpenedSource { text, leases, inflating: Some(inflating), watch: None });
        }
    }
    let text = open_text(File::open(&dst).map_err(CoreError::io(&dst))?, &dst, cache, cancel, progress, &mut leases)?;
    Ok(OpenedSource { text, leases, inflating: None, watch: None })
}

enum InflateMsg {
    Progress(u64, u64),
    Done(Result<(), CoreError>),
}

pub enum InflatePoll {
    Running,
    /// The decompression ended; on success the complete file is at `Inflating::dst`.
    Done(Result<(), CoreError>),
}

/// A `.gz` decompressing into the cache on a background thread (spec §4.1: viewable while it proceeds). The output
/// goes to a partial file renamed to the entry when complete; the text reads it through a handle taken up front, so
/// the rename does not affect it. Dropping this cancels the decompression (its partial file is removed).
pub struct Inflating {
    cancel: CancelToken,
    rx: Receiver<InflateMsg>,
    reader: Option<File>,
    partial: PathBuf,
    dst: PathBuf,
    progress: (u64, u64),
    finished: bool,
}

impl Inflating {
    fn start(src: PathBuf, dst: PathBuf) -> Result<Inflating, CoreError> {
        let partial = partial_path(&dst);
        let mut writer = File::create(&partial).map_err(CoreError::io(&partial))?;
        let reader = match File::open(&partial) {
            Ok(reader) => reader,
            Err(source) => {
                let _ = std::fs::remove_file(&partial);
                return Err(CoreError::Io { path: partial, source });
            }
        };
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let (thread_partial, thread_dst) = (partial.clone(), dst.clone());
        let spawned = std::thread::Builder::new()
            .name("rogg-gunzip".into())
            .spawn(move || {
                let sender = tx.clone();
                let result = gunzip(&src, &mut writer, &token, |read, total| {
                    let _ = sender.send(InflateMsg::Progress(read, total));
                })
                .and_then(|_| std::fs::rename(&thread_partial, &thread_dst).map_err(CoreError::io(&thread_dst)));
                if result.is_err() {
                    let _ = std::fs::remove_file(&thread_partial);
                }
                let _ = tx.send(InflateMsg::Done(result));
            });
        if let Err(source) = spawned {
            let _ = std::fs::remove_file(&partial);
            return Err(CoreError::Io { path: dst, source });
        }
        Ok(Inflating { cancel, rx, reader: Some(reader), partial, dst, progress: (0, 0), finished: false })
    }

    pub fn dst(&self) -> &Path {
        &self.dst
    }

    pub fn partial(&self) -> &Path {
        &self.partial
    }

    /// Compressed bytes read and the compressed size, as last reported.
    pub fn progress(&self) -> (u64, u64) {
        self.progress
    }

    pub fn fraction(&self) -> f32 {
        let (read, total) = self.progress;
        if total == 0 { 0. } else { read as f32 / total as f32 }
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    fn died(&self) -> CoreError {
        CoreError::Archive { path: self.dst.clone(), message: "decompression stopped unexpectedly".into() }
    }

    /// Drains progress; reports `Done` once the decompression ends.
    pub fn poll(&mut self) -> InflatePoll {
        loop {
            match self.rx.try_recv() {
                Ok(InflateMsg::Progress(read, total)) => self.progress = (read, total),
                Ok(InflateMsg::Done(result)) => {
                    self.finished = true;
                    return InflatePoll::Done(result);
                }
                Err(TryRecvError::Empty) => return InflatePoll::Running,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    return InflatePoll::Done(Err(self.died()));
                }
            }
        }
    }

    /// Blocks until at least `n` bytes are decompressed (or the stream ended) and returns those first bytes.
    fn wait_for_head(&mut self, n: usize, cancel: &CancelToken, report: &mut impl FnMut(u64, u64)) -> Result<Vec<u8>, CoreError> {
        loop {
            if cancel.is_cancelled() {
                self.cancel();
                return Err(CoreError::Cancelled);
            }
            let reader = self.reader.as_ref().expect("the reader is held until the text opens");
            let len = reader.metadata().map_err(CoreError::io(&self.partial))?.len();
            if len >= n as u64 || self.finished {
                let mut head = vec![0u8; (len as usize).min(n)];
                crate::fsx::read_exact_at(reader, &mut head, 0).map_err(CoreError::io(&self.partial))?;
                return Ok(head);
            }
            self.wait_step(report)?;
        }
    }

    /// Blocks until the decompression ends.
    fn wait_done(&mut self, cancel: &CancelToken, report: &mut impl FnMut(u64, u64)) -> Result<(), CoreError> {
        while !self.finished {
            if cancel.is_cancelled() {
                self.cancel();
                return Err(CoreError::Cancelled);
            }
            self.wait_step(report)?;
        }
        Ok(())
    }

    fn wait_step(&mut self, report: &mut impl FnMut(u64, u64)) -> Result<(), CoreError> {
        match self.rx.recv_timeout(Duration::from_millis(10)) {
            Ok(InflateMsg::Progress(read, total)) => {
                self.progress = (read, total);
                report(read, total);
                Ok(())
            }
            Ok(InflateMsg::Done(Ok(()))) => {
                self.finished = true;
                Ok(())
            }
            Ok(InflateMsg::Done(Err(e))) => Err(e),
            Err(RecvTimeoutError::Timeout) => Ok(()),
            Err(RecvTimeoutError::Disconnected) => Err(self.died()),
        }
    }
}

impl Drop for Inflating {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
