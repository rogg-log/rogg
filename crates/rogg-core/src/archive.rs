//! Archives and compressed files (spec §4.1): what a file holds, cheap listing of zip and tar archives, extracting
//! one member, and streaming `.gz` decompression. Damage is an `Archive` error for the archive or member it concerns,
//! never a panic (spec §4.13).

use crate::cancel::CancelToken;
use crate::encoding::Encoding;
use crate::error::CoreError;
use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::rc::Rc;

/// Bytes sniffed to tell text from binary (spec §4.1: the first 8 KB).
pub const SNIFF_BYTES: usize = 8192;
const COPY_CHUNK: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
}

/// What a file holds, judged from its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    Archive(ArchiveKind),
    /// A single gzip-compressed file (`app.log.1.gz`).
    Gzip,
    Text,
    Binary,
}

/// A regular file inside an archive: its `/`-separated path and uncompressed size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

fn is_tar_block(block: &[u8]) -> bool {
    block.len() >= 512 && &block[257..262] == b"ustar"
}

/// Reads until `buf` is full or the input ends.
fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// Classifies a file: zip, tar, gzip (a tar.gz when the decompressed start is a tar header), text (no NUL in the
/// first 8 KB, or a UTF-16 BOM) or binary.
pub fn sniff(path: &Path) -> io::Result<Content> {
    let mut head = vec![0u8; SNIFF_BYTES];
    let n = read_full(&mut File::open(path)?, &mut head)?;
    head.truncate(n);
    if head.starts_with(&[0x1f, 0x8b]) {
        let mut inner = [0u8; 512];
        let got = read_full(&mut MultiGzDecoder::new(File::open(path)?), &mut inner).unwrap_or(0);
        return Ok(if is_tar_block(&inner[..got]) { Content::Archive(ArchiveKind::TarGz) } else { Content::Gzip });
    }
    Ok(sniff_head(&head))
}

/// `sniff` for bytes already read; a gzip start is reported as `Gzip` (no look inside).
pub fn sniff_head(head: &[u8]) -> Content {
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Content::Archive(ArchiveKind::Zip)
    } else if head.starts_with(&[0x1f, 0x8b]) {
        Content::Gzip
    } else if is_tar_block(head) {
        Content::Archive(ArchiveKind::Tar)
    } else if Encoding::detect(head).is_utf16() || !head[..head.len().min(SNIFF_BYTES)].contains(&0) {
        Content::Text
    } else {
        Content::Binary
    }
}

/// The archive kind a member's name suggests (members are only sniffed once extracted).
pub fn kind_from_name(name: &str) -> Option<ArchiveKind> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if lower.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else {
        None
    }
}

fn archive_error(path: &Path, message: impl std::fmt::Display) -> CoreError {
    CoreError::Archive { path: path.to_path_buf(), message: message.to_string() }
}

/// A member path as listed: without a leading `./` or `/`.
fn clean(name: &str) -> &str {
    name.trim_start_matches("./").trim_start_matches('/')
}

/// Counts the bytes read through it (tar progress is measured on the compressed file).
struct Counting<R> {
    inner: R,
    read: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.set(self.read.get() + n as u64);
        Ok(n)
    }
}

fn tar_reader(path: &Path, kind: ArchiveKind, read: Rc<Cell<u64>>) -> Result<(Box<dyn Read>, u64), CoreError> {
    let file = File::open(path).map_err(CoreError::io(path))?;
    let total = file.metadata().map_err(CoreError::io(path))?.len();
    let counted = Counting { inner: file, read };
    let reader: Box<dyn Read> = if kind == ArchiveKind::TarGz { Box::new(MultiGzDecoder::new(counted)) } else { Box::new(counted) };
    Ok((reader, total))
}

/// Lists the regular files of an archive in archive order. `progress(done, total)` counts entries (zip) or archive
/// bytes read (tar).
pub fn list(path: &Path, kind: ArchiveKind, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<Vec<ArchiveEntry>, CoreError> {
    let mut out = Vec::new();
    match kind {
        ArchiveKind::Zip => {
            let file = File::open(path).map_err(CoreError::io(path))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| archive_error(path, e))?;
            let total = zip.len() as u64;
            for i in 0..zip.len() {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let entry = zip.by_index_raw(i).map_err(|e| archive_error(path, e))?;
                if !entry.is_dir() {
                    out.push(ArchiveEntry { path: clean(entry.name()).to_string(), size: entry.size() });
                }
                progress(i as u64 + 1, total);
            }
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            let read = Rc::new(Cell::new(0));
            let (reader, total) = tar_reader(path, kind, read.clone())?;
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries().map_err(|e| archive_error(path, e))? {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let entry = entry.map_err(|e| archive_error(path, e))?;
                if entry.header().entry_type().is_file() {
                    let name = entry.path().map_err(|e| archive_error(path, e))?.to_string_lossy().into_owned();
                    out.push(ArchiveEntry { path: clean(&name).to_string(), size: entry.size() });
                }
            }
            progress(total, total);
        }
    }
    Ok(out)
}

enum CopyError {
    Read(io::Error),
    Write(io::Error),
    Cancelled,
}

/// Copies `from` to `out` in 1 MiB chunks, checking `cancel` between chunks.
fn copy(from: &mut impl Read, out: &mut impl Write, total: u64, cancel: &CancelToken, progress: &mut impl FnMut(u64, u64)) -> Result<u64, CopyError> {
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut written = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(CopyError::Cancelled);
        }
        let n = match from.read(&mut buf) {
            Ok(0) => return Ok(written),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(CopyError::Read(e)),
        };
        out.write_all(&buf[..n]).map_err(CopyError::Write)?;
        written += n as u64;
        progress(written, total.max(written));
    }
}

/// Copies archive member `member` to `out`; `progress(written, member size)`. A missing member, an unsupported
/// compression method or a damaged member is an `Archive` error naming the member.
pub fn extract(
    path: &Path,
    kind: ArchiveKind,
    member: &str,
    out: &mut impl Write,
    cancel: &CancelToken,
    mut progress: impl FnMut(u64, u64),
) -> Result<u64, CoreError> {
    let failed = |e: CopyError| match e {
        CopyError::Read(e) => archive_error(path, format!("{member}: {e}")),
        CopyError::Write(e) => CoreError::Io { path: path.to_path_buf(), source: e },
        CopyError::Cancelled => CoreError::Cancelled,
    };
    match kind {
        ArchiveKind::Zip => {
            let file = File::open(path).map_err(CoreError::io(path))?;
            let mut zip = zip::ZipArchive::new(file).map_err(|e| archive_error(path, e))?;
            let index = (0..zip.len())
                .find(|&i| zip.name_for_index(i).is_some_and(|n| clean(n) == member))
                .ok_or_else(|| archive_error(path, format!("no member `{member}`")))?;
            let mut entry = zip.by_index(index).map_err(|e| archive_error(path, format!("{member}: {e}")))?;
            let size = entry.size();
            copy(&mut entry, out, size, cancel, &mut progress).map_err(failed)
        }
        ArchiveKind::Tar | ArchiveKind::TarGz => {
            let (reader, _) = tar_reader(path, kind, Rc::new(Cell::new(0)))?;
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries().map_err(|e| archive_error(path, e))? {
                if cancel.is_cancelled() {
                    return Err(CoreError::Cancelled);
                }
                let mut entry = entry.map_err(|e| archive_error(path, e))?;
                let name = entry.path().map_err(|e| archive_error(path, e))?.to_string_lossy().into_owned();
                if entry.header().entry_type().is_file() && clean(&name) == member {
                    let size = entry.size();
                    return copy(&mut entry, out, size, cancel, &mut progress).map_err(failed);
                }
            }
            Err(archive_error(path, format!("no member `{member}`")))
        }
    }
}

/// Decompresses the gzip file `src` (every member of a multi-member file) into `out`, writing as it goes so a reader
/// of `out` sees the data at once; `progress(compressed bytes read, compressed size)`.
pub fn gunzip(src: &Path, out: &mut impl Write, cancel: &CancelToken, mut progress: impl FnMut(u64, u64)) -> Result<u64, CoreError> {
    let file = File::open(src).map_err(CoreError::io(src))?;
    let total = file.metadata().map_err(CoreError::io(src))?.len();
    let read = Rc::new(Cell::new(0));
    let mut decoder = MultiGzDecoder::new(Counting { inner: file, read: read.clone() });
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut written = 0u64;
    loop {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let n = match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(archive_error(src, e)),
        };
        out.write_all(&buf[..n]).map_err(CoreError::io(src))?;
        written += n as u64;
        progress(read.get().min(total), total);
    }
    progress(total, total);
    Ok(written)
}
