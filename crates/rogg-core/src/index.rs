//! Line index (spec §4.4): the start offset of every line, kept in blocks of up to 4096 lines.
//! Each block holds a u64 base offset and u32 offsets relative to it, so ~4 bytes per line.

use crate::cancel::CancelToken;
use crate::error::CoreError;
use rayon::prelude::*;
use std::ops::Range;

pub const BLOCK_LINES: u64 = 4096;
/// Parallel build chunk size (spec: ~16 MB).
pub const DEFAULT_CHUNK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    block_first_line: Vec<u64>,
    block_base: Vec<u64>,
    rel: Vec<u32>,
    data_len: u64,
    ends_with_newline: bool,
    longest_line: u64,
    max_block_span: u64,
}

impl Default for LineIndex {
    fn default() -> Self {
        Self::with_max_block_span(u32::MAX as u64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexProgress {
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl LineIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Caps the bytes a block may span. Production uses u32::MAX; tests use tiny values to hit the overflow path.
    pub fn with_max_block_span(max_block_span: u64) -> Self {
        Self {
            block_first_line: Vec::new(),
            block_base: Vec::new(),
            rel: Vec::new(),
            data_len: 0,
            ends_with_newline: false,
            longest_line: 0,
            max_block_span,
        }
    }

    /// Serial build (reference implementation; used for small inputs).
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut index = Self::new();
        index.extend_to(data);
        index
    }

    pub fn line_count(&self) -> u64 {
        self.rel.len() as u64
    }

    pub fn data_len(&self) -> u64 {
        self.data_len
    }

    /// Whether the indexed bytes end with `\n` (otherwise the last line may still be extended by an append).
    pub fn ends_with_newline(&self) -> bool {
        self.ends_with_newline
    }

    /// Longest line in bytes, excluding its `\n`.
    pub fn longest_line(&self) -> u64 {
        self.longest_line
    }

    /// Byte offset where `line` starts. `line` must be `< line_count()`.
    pub fn line_start(&self, line: u64) -> u64 {
        let block = self.block_first_line.partition_point(|&first| first <= line) - 1;
        self.block_base[block] + self.rel[line as usize] as u64
    }

    /// Bytes of `line` without its `\n` (a trailing `\r` is kept). `line` must be `< line_count()`.
    pub fn line_range(&self, line: u64) -> Range<u64> {
        let start = self.line_start(line);
        let end = if line + 1 < self.line_count() {
            self.line_start(line + 1) - 1
        } else {
            self.data_len - self.ends_with_newline as u64
        };
        start..end
    }

    pub fn heap_bytes(&self) -> usize {
        self.block_first_line.capacity() * 8 + self.block_base.capacity() * 8 + self.rel.capacity() * 4
    }

    pub fn shrink_to_fit(&mut self) {
        self.block_first_line.shrink_to_fit();
        self.block_base.shrink_to_fit();
        self.rel.shrink_to_fit();
    }

    /// Indexes `data[data_len()..]`. `data` must begin with the bytes already indexed (incremental append).
    pub fn extend_to(&mut self, data: &[u8]) {
        let from = self.data_len as usize;
        if data.len() <= from {
            return;
        }
        let newlines: Vec<u64> = memchr::memchr_iter(b'\n', &data[from..]).map(|p| (from + p) as u64).collect();
        self.append_scanned(data.len() as u64, &newlines);
    }

    fn push_start(&mut self, start: u64) {
        let line = self.line_count();
        let new_block = match (self.block_first_line.last(), self.block_base.last()) {
            (Some(&first), Some(&base)) => line - first >= BLOCK_LINES || start - base > self.max_block_span,
            _ => true,
        };
        if new_block {
            self.block_first_line.push(line);
            self.block_base.push(start);
        }
        let base = *self.block_base.last().expect("a block exists after push");
        self.rel.push((start - base) as u32);
    }

    /// Adds the lines of bytes `data_len()..new_len`, given every `\n` position in that range (ascending).
    fn append_scanned(&mut self, new_len: u64, newlines: &[u64]) {
        if new_len <= self.data_len {
            return;
        }
        if self.data_len == 0 || self.ends_with_newline {
            self.push_start(self.data_len);
        }
        for &nl in newlines {
            let last = self.line_count() - 1;
            self.longest_line = self.longest_line.max(nl - self.line_start(last));
            if nl + 1 < new_len {
                self.push_start(nl + 1);
            }
        }
        self.data_len = new_len;
        self.ends_with_newline = newlines.last() == Some(&(new_len - 1));
        if !self.ends_with_newline {
            let last = self.line_count() - 1;
            self.longest_line = self.longest_line.max(new_len - self.line_start(last));
        }
    }
}

/// Builds the index of `data` on all cores. Chunks are scanned for `\n` in parallel waves and stitched
/// in order. `on_wave` sees the growing index after each wave, so a UI can scroll the indexed prefix.
pub fn build_index(
    data: &[u8],
    chunk_bytes: usize,
    cancel: &CancelToken,
    on_wave: impl FnMut(&LineIndex, IndexProgress),
) -> Result<LineIndex, CoreError> {
    build_index_into(LineIndex::new(), data, chunk_bytes, cancel, on_wave)
}

/// Like `build_index`, continuing from an existing (possibly partially built) index.
pub fn build_index_into(
    mut index: LineIndex,
    data: &[u8],
    chunk_bytes: usize,
    cancel: &CancelToken,
    mut on_wave: impl FnMut(&LineIndex, IndexProgress),
) -> Result<LineIndex, CoreError> {
    let chunk = chunk_bytes.max(1);
    let total = data.len() as u64;
    let ranges: Vec<Range<usize>> =
        (index.data_len as usize..data.len()).step_by(chunk).map(|s| s..(s + chunk).min(data.len())).collect();
    let wave = rayon::current_num_threads().max(1) * 2;
    for group in ranges.chunks(wave) {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let scanned: Vec<Vec<u64>> = group
            .par_iter()
            .map(|r| memchr::memchr_iter(b'\n', &data[r.clone()]).map(|p| (r.start + p) as u64).collect())
            .collect();
        for (r, newlines) in group.iter().zip(&scanned) {
            index.append_scanned(r.end as u64, newlines);
        }
        on_wave(&index, IndexProgress { bytes_done: index.data_len, bytes_total: total });
    }
    if cancel.is_cancelled() && ranges.is_empty() {
        return Err(CoreError::Cancelled);
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(idx: &LineIndex) -> Vec<Range<u64>> {
        (0..idx.line_count()).map(|i| idx.line_range(i)).collect()
    }

    #[test]
    fn degenerate_shapes() {
        assert_eq!(LineIndex::from_bytes(b"").line_count(), 0);
        let one_newline = LineIndex::from_bytes(b"\n");
        assert_eq!(ranges(&one_newline), vec![0..0]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"\n\n")), vec![0..0, 1..1]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\nbb\nccc")), vec![0..1, 2..4, 5..8]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\nbb\n")), vec![0..1, 2..4]);
        assert_eq!(ranges(&LineIndex::from_bytes(b"a\r\nb")), vec![0..2, 3..4], "\\r is kept; LogText strips it");
    }

    #[test]
    fn longest_line_ignores_newlines() {
        assert_eq!(LineIndex::from_bytes(b"a\nbb\nccc").longest_line(), 3);
        assert_eq!(LineIndex::from_bytes(b"abcd\n").longest_line(), 4);
        let huge = vec![b'x'; 1 << 20];
        let idx = LineIndex::from_bytes(&huge);
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.longest_line(), 1 << 20);
    }

    #[test]
    fn starts_survive_block_boundaries_and_span_overflow() {
        let data: Vec<u8> = (0..10_000).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
        let idx = LineIndex::from_bytes(&data);
        assert_eq!(idx.line_count(), 10_000);
        assert_eq!(&data[idx.line_range(4096).start as usize..idx.line_range(4096).end as usize], b"line 4096");
        assert_eq!(&data[idx.line_range(9999).start as usize..idx.line_range(9999).end as usize], b"line 9999");
        // A tiny span limit forces a new block every few lines, exercising the u32-overflow path.
        let mut small = LineIndex::with_max_block_span(10);
        small.extend_to(&data);
        assert_eq!(ranges(&small), ranges(&idx));
    }

    #[test]
    fn memory_is_about_four_bytes_per_line() {
        let data: Vec<u8> = (0..100_000).flat_map(|_| b"xy\n".to_vec()).collect();
        let mut idx = LineIndex::from_bytes(&data);
        idx.shrink_to_fit();
        assert!(idx.heap_bytes() <= 100_000 * 4 + 4096, "heap {}", idx.heap_bytes());
    }

    #[test]
    fn parallel_build_matches_serial_and_reports_progress() {
        let data: Vec<u8> = (0..5_000).flat_map(|i| format!("{i}\r\n").into_bytes()).collect();
        let mut waves = Vec::new();
        let idx = build_index(&data, 997, &CancelToken::new(), |i, p| waves.push((i.line_count(), p))).unwrap();
        assert_eq!(idx, LineIndex::from_bytes(&data));
        assert!(!waves.is_empty());
        assert!(waves.windows(2).all(|w| w[0].1.bytes_done <= w[1].1.bytes_done));
        assert_eq!(waves.last().unwrap().1, IndexProgress { bytes_done: data.len() as u64, bytes_total: data.len() as u64 });
    }

    #[test]
    fn cancelled_build_returns_cancelled() {
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(build_index(b"a\nb\n", 1, &cancel, |_, _| {}), Err(CoreError::Cancelled)));
    }
}
