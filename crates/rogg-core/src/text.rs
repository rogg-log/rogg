//! Opened log text: UTF-8 bytes plus the line index (spec §4.1 plain files, §4.3, §4.4).
//!
//! Files are memory-mapped through `mapping::SafeMap`, so a file truncated while open reads zeros past its new end
//! instead of crashing; live follow (`LogText::refresh`, spec §4.5) notices the shrink and reloads.

use crate::cancel::CancelToken;
use crate::encoding::{ENDINGS_SAMPLE_BYTES, Encoding, LineEndings, status_label, transcode_to_utf8};
use crate::error::CoreError;
use crate::index::{DEFAULT_CHUNK_BYTES, IndexProgress, LineIndex, build_index, build_index_into};
use crate::mapping::SafeMap;
use std::borrow::Cow;
use std::fs::File;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Bytes of a file's start remembered to detect in-place rewrites.
const HEAD_SAMPLE: usize = 64;

enum Data {
    Mapped(SafeMap),
    Owned(Vec<u8>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Mapped(m) => m.bytes(),
            Data::Owned(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenProgress {
    Transcoding { done: u64, total: u64 },
    Indexing { done: u64, total: u64 },
}

/// A log's UTF-8 text and line index. Line content excludes `\n` and one trailing `\r`.
pub struct LogText {
    data: Data,
    start: usize,
    index: LineIndex,
    encoding: Encoding,
    endings: LineEndings,
    source_path: Option<PathBuf>,
    /// The file whose bytes are mapped: the source itself, or its UTF-8 conversion in the cache.
    backing: Option<PathBuf>,
    /// The mapped file, kept open so growth can re-map it (spec §4.5); None for in-memory and converted texts.
    file: Option<Arc<File>>,
    /// A copy of the file's first bytes when it was mapped: a file rewritten in place (emptied and written again, as
    /// `logrotate copytruncate` does) is told apart from one that only grew, even when it is as long as before.
    head: Vec<u8>,
}

/// What `LogText::refresh` found.
#[allow(clippy::large_enum_variant)] // short-lived return value, moved straight into an `Arc`; boxing would only add an allocation
pub enum Refresh {
    Unchanged,
    /// The file grew: a new snapshot whose index extends this one's (only the appended bytes were scanned).
    Grew(LogText),
    /// The file is shorter than what is mapped, or its start changed: it was truncated or rewritten (reopen it).
    Truncated,
}

fn map_file(file: &File, path: &Path) -> Result<Data, CoreError> {
    let len = file.metadata().map_err(CoreError::io(path))?.len();
    if len == 0 {
        return Ok(Data::Owned(Vec::new()));
    }
    SafeMap::map(file).map(Data::Mapped).map_err(CoreError::io(path))
}

/// Where the UTF-8 conversion of the UTF-16 file at `path` lives in the cache (spec §4.2 key). A cache entry is
/// immutable and already origin-keyed (leasing touches its mtime, which would change an `origin_key`), so a top-level
/// entry's conversion is keyed by its file stem. Anything deeper (a capture in `streams/`, named by pid and counter,
/// which a later run can reuse for other data) keeps the origin key.
fn transcoded_path(cache_dir: &Path, path: &Path) -> Result<PathBuf, CoreError> {
    if path.parent() == Some(cache_dir)
        && let Some(stem) = path.file_stem()
    {
        return Ok(cache_dir.join(format!("{}.utf8", stem.to_string_lossy())));
    }
    let key = crate::cache::origin_key(path, &[], "utf8").map_err(CoreError::io(path))?;
    Ok(cache_dir.join(format!("{key:016x}.utf8")))
}

impl LogText {
    /// Opens a plain file: detects the encoding, transcodes UTF-16 into `cache_dir`, memory-maps and indexes it.
    pub fn open(path: &Path, cache_dir: &Path, cancel: &CancelToken, on_progress: impl FnMut(OpenProgress)) -> Result<LogText, CoreError> {
        let file = File::open(path).map_err(CoreError::io(path))?;
        Self::open_handle(file, path, cache_dir, cancel, on_progress)
    }

    /// `open` for a file that is already open: a `.gz` being decompressed is read through a handle taken before its
    /// partial file is renamed. `path` names the file for errors, the cache key and `source_path`.
    pub fn open_handle(
        file: File,
        path: &Path,
        cache_dir: &Path,
        cancel: &CancelToken,
        mut on_progress: impl FnMut(OpenProgress),
    ) -> Result<LogText, CoreError> {
        let mut head = [0u8; 4];
        let mut head_len = 0;
        while head_len < head.len() {
            let n = crate::fsx::read_at(&file, &mut head[head_len..], head_len as u64).map_err(CoreError::io(path))?;
            if n == 0 {
                break;
            }
            head_len += n;
        }
        let encoding = Encoding::detect(&head[..head_len]);
        let (data, start, backing, handle) = if encoding.is_utf16() {
            let dst = transcoded_path(cache_dir, path)?;
            if !dst.exists() {
                std::fs::create_dir_all(cache_dir).map_err(CoreError::io(cache_dir))?;
                transcode_to_utf8(path, &dst, encoding, cancel, |done, total| on_progress(OpenProgress::Transcoding { done, total }))?;
            }
            let converted = File::open(&dst).map_err(CoreError::io(&dst))?;
            (map_file(&converted, &dst)?, 0, dst, None)
        } else {
            (map_file(&file, path)?, encoding.bom_len(), path.to_path_buf(), Some(Arc::new(file)))
        };
        let text = &data.bytes()[start.min(data.bytes().len())..];
        let total = text.len() as u64;
        let mut index = build_index(text, DEFAULT_CHUNK_BYTES, cancel, |_, p: IndexProgress| on_progress(OpenProgress::Indexing { done: p.bytes_done, total }))?;
        index.shrink_to_fit();
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        let head = data.bytes()[..data.bytes().len().min(HEAD_SAMPLE)].to_vec();
        Ok(LogText { data, start, index, encoding, endings, source_path: Some(path.to_path_buf()), backing: Some(backing), file: handle, head })
    }

    /// Builds text from memory (tests, small inputs, stdin buffers). UTF-16 is decoded; a UTF-8 BOM is skipped.
    pub fn from_bytes(bytes: Vec<u8>) -> LogText {
        let encoding = Encoding::detect(&bytes);
        let (data, start) = if encoding.is_utf16() {
            (Data::Owned(encoding.decode_utf16(&bytes).into_bytes()), 0)
        } else {
            let start = encoding.bom_len();
            (Data::Owned(bytes), start)
        };
        let text = &data.bytes()[start..];
        let index = LineIndex::from_bytes(text);
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        LogText { data, start, index, encoding, endings, source_path: None, backing: None, file: None, head: Vec::new() }
    }

    /// Lines an append cannot change: every line, or all but the last when the text does not end with `\n`.
    pub fn stable_lines(&self) -> u64 {
        let n = self.line_count();
        if n == 0 || self.index.ends_with_newline() { n } else { n - 1 }
    }

    /// (device, inode) of the mapped file; live follow compares it with the path's to detect replacement.
    pub fn file_identity(&self) -> Option<(u64, u64)> {
        crate::fsx::file_identity(self.file.as_ref()?)
    }

    /// Checks the mapped file (spec §4.5). A longer file whose start is unchanged gives a new snapshot that maps the
    /// whole file and indexes only the appended bytes (an unfinished last line continues). A shorter file, or one whose
    /// first bytes changed, is `Truncated`. Texts without a file handle (in memory, UTF-16 conversions) are always
    /// `Unchanged`. A rewrite that keeps the first 64 bytes is indistinguishable from an append.
    pub fn refresh(&self, cancel: &CancelToken) -> Result<Refresh, CoreError> {
        let Some(file) = &self.file else { return Ok(Refresh::Unchanged) };
        let path = self.backing.clone().unwrap_or_default();
        let len = file.metadata().map_err(CoreError::io(&path))?.len();
        let have = self.mapped_len();
        if len < have {
            return Ok(Refresh::Truncated);
        }
        let mut head = vec![0u8; self.head.len()];
        if !head.is_empty() && (crate::fsx::read_exact_at(file, &mut head, 0).is_err() || head != self.head) {
            return Ok(Refresh::Truncated);
        }
        if len == have {
            return Ok(Refresh::Unchanged);
        }
        // The file may have changed since the size check above: an emptied file cannot be mapped, and the map covers
        // the file's length at this moment, so `grow_into` checks the map itself.
        let map = match SafeMap::map(file) {
            Ok(map) => map,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => return Ok(Refresh::Truncated),
            Err(e) => return Err(CoreError::io(&path)(e)),
        };
        self.grow_into(map, cancel)
    }

    /// The part of `refresh` that trusts only `map` (a fresh mapping of the file): it must still hold everything this
    /// text maps and start with the same bytes, otherwise the file was truncated after it was measured.
    fn grow_into(&self, map: SafeMap, cancel: &CancelToken) -> Result<Refresh, CoreError> {
        let all = map.bytes();
        if all.len() < self.data.bytes().len() || !all.starts_with(&self.head) {
            return Ok(Refresh::Truncated);
        }
        let bytes = &all[self.start.min(all.len())..];
        let index = build_index_into(self.index.clone(), bytes, DEFAULT_CHUNK_BYTES, cancel, |_, _| {})?;
        let endings = if self.endings == LineEndings::None { LineEndings::detect(&bytes[..bytes.len().min(ENDINGS_SAMPLE_BYTES)]) } else { self.endings };
        let head = all[..all.len().min(HEAD_SAMPLE)].to_vec();
        Ok(Refresh::Grew(LogText {
            data: Data::Mapped(map),
            start: self.start,
            index,
            encoding: self.encoding,
            endings,
            source_path: self.source_path.clone(),
            backing: self.backing.clone(),
            file: self.file.clone(),
            head,
        }))
    }

    /// UTF-8 text bytes (after any BOM).
    pub fn bytes(&self) -> &[u8] {
        &self.data.bytes()[self.start..]
    }

    pub fn index(&self) -> &LineIndex {
        &self.index
    }

    pub fn line_count(&self) -> u64 {
        self.index.line_count()
    }

    /// Line content without `\n` and one trailing `\r`; empty when `i` is out of range.
    pub fn line(&self, i: u64) -> &[u8] {
        if i >= self.line_count() {
            return &[];
        }
        let r = self.index.line_range(i);
        let line = &self.bytes()[r.start as usize..r.end as usize];
        line.strip_suffix(b"\r").unwrap_or(line)
    }

    /// Display text (invalid UTF-8 becomes U+FFFD).
    pub fn line_text(&self, i: u64) -> Cow<'_, str> {
        String::from_utf8_lossy(self.line(i))
    }

    /// Contiguous bytes covering `lines` and the offset of their first byte within `bytes()`.
    pub fn span(&self, lines: Range<u64>) -> (&[u8], u64) {
        let end_line = lines.end.min(self.line_count());
        if lines.start >= end_line {
            return (&[], 0);
        }
        let start = self.index.line_start(lines.start);
        let end = self.index.line_range(end_line - 1).end;
        (&self.bytes()[start as usize..end as usize], start)
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub fn endings(&self) -> LineEndings {
        self.endings
    }

    pub fn status_label(&self) -> String {
        status_label(self.encoding, self.endings)
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// The file whose bytes are mapped (the source, or its UTF-8 conversion in the cache).
    pub fn backing_path(&self) -> Option<&Path> {
        self.backing.as_deref()
    }

    /// Bytes mapped, a UTF-8 BOM included (compare with the backing file's size to see whether it grew).
    pub fn mapped_len(&self) -> u64 {
        self.data.bytes().len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(path: &Path) -> LogText {
        LogText::open(path, &path.with_extension("cache"), &CancelToken::new(), |_| {}).unwrap()
    }

    /// The file shrank between `refresh`'s size check and its mapping: the new map is shorter than what was mapped
    /// before, so the snapshot must not be built (its index would point past the map).
    #[test]
    fn a_map_shorter_than_before_is_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let text = open(&path);
        std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(8).unwrap();
        let shorter = SafeMap::map(&File::open(&path).unwrap()).unwrap();
        assert!(matches!(text.grow_into(shorter, &CancelToken::new()).unwrap(), Refresh::Truncated));
    }

    #[test]
    fn an_emptied_file_is_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("z.log");
        std::fs::write(&path, "one\n").unwrap();
        let text = open(&path);
        std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
        assert!(matches!(text.refresh(&CancelToken::new()).unwrap(), Refresh::Truncated));
    }
}
