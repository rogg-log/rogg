//! Opened log text: UTF-8 bytes plus the line index (spec §4.1 plain files, §4.3, §4.4).
//!
//! Files are memory-mapped. The mapping assumes the file is not truncated while open; live follow and
//! truncation handling (spec §4.5, milestone M3) will re-map on change.

use crate::cancel::CancelToken;
use crate::encoding::{ENDINGS_SAMPLE_BYTES, Encoding, LineEndings, status_label, transcode_to_utf8};
use crate::error::CoreError;
use crate::index::{DEFAULT_CHUNK_BYTES, IndexProgress, LineIndex, build_index};
use memmap2::Mmap;
use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::{Path, PathBuf};

enum Data {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Mapped(m) => m,
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
///
/// Opened files are memory-mapped and the mapping assumes the file is not truncated while mapped; truncating
/// it externally is undefined behavior until live follow (M3) re-maps on change.
pub struct LogText {
    data: Data,
    start: usize,
    index: LineIndex,
    encoding: Encoding,
    endings: LineEndings,
    source_path: Option<PathBuf>,
}

fn map_file(path: &Path) -> Result<Data, CoreError> {
    let file = File::open(path).map_err(CoreError::io(path))?;
    let len = file.metadata().map_err(CoreError::io(path))?.len();
    if len == 0 {
        return Ok(Data::Owned(Vec::new()));
    }
    // SAFETY: the file is opened read-only; rogg does not write to it. External truncation while mapped is
    // handled by re-mapping in M3 (live follow), see module docs.
    let map = unsafe { Mmap::map(&file) }.map_err(CoreError::io(path))?;
    Ok(Data::Mapped(map))
}

fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for &b in *part {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn transcoded_path(cache_dir: &Path, path: &Path, meta: &std::fs::Metadata) -> PathBuf {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let key = fnv1a(&[canonical.as_os_str().as_encoded_bytes(), &meta.len().to_le_bytes(), &mtime.to_le_bytes()]);
    cache_dir.join(format!("{key:016x}.utf8"))
}

impl LogText {
    /// Opens a plain file: detects the encoding, transcodes UTF-16 into `cache_dir`, memory-maps and indexes it.
    pub fn open(
        path: &Path,
        cache_dir: &Path,
        cancel: &CancelToken,
        mut on_progress: impl FnMut(OpenProgress),
    ) -> Result<LogText, CoreError> {
        let mut file = File::open(path).map_err(CoreError::io(path))?;
        let meta = file.metadata().map_err(CoreError::io(path))?;
        let mut head = [0u8; 4];
        let mut head_len = 0;
        while head_len < head.len() {
            let n = file.read(&mut head[head_len..]).map_err(CoreError::io(path))?;
            if n == 0 {
                break;
            }
            head_len += n;
        }
        let encoding = Encoding::detect(&head[..head_len]);
        let (data, start) = if encoding.is_utf16() {
            let dst = transcoded_path(cache_dir, path, &meta);
            if !dst.exists() {
                std::fs::create_dir_all(cache_dir).map_err(CoreError::io(cache_dir))?;
                transcode_to_utf8(path, &dst, encoding, cancel, |done, total| {
                    on_progress(OpenProgress::Transcoding { done, total })
                })?;
            }
            (map_file(&dst)?, 0)
        } else {
            (map_file(path)?, encoding.bom_len())
        };
        let text = &data.bytes()[start.min(data.bytes().len())..];
        let total = text.len() as u64;
        let mut index = build_index(text, DEFAULT_CHUNK_BYTES, cancel, |_, p: IndexProgress| {
            on_progress(OpenProgress::Indexing { done: p.bytes_done, total })
        })?;
        index.shrink_to_fit();
        let endings = LineEndings::detect(&text[..text.len().min(ENDINGS_SAMPLE_BYTES)]);
        Ok(LogText { data, start, index, encoding, endings, source_path: Some(path.to_path_buf()) })
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
        LogText { data, start, index, encoding, endings, source_path: None }
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
}
