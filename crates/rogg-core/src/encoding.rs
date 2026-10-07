//! Encoding detection, line-ending detection and UTF-16 → UTF-8 transcoding (spec §4.3).

use crate::cancel::CancelToken;
use crate::error::CoreError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

impl Encoding {
    pub fn detect(head: &[u8]) -> Self {
        match head {
            [0xEF, 0xBB, 0xBF, ..] => Encoding::Utf8Bom,
            [0xFF, 0xFE, ..] => Encoding::Utf16Le,
            [0xFE, 0xFF, ..] => Encoding::Utf16Be,
            _ => Encoding::Utf8,
        }
    }

    pub fn bom_len(self) -> usize {
        match self {
            Encoding::Utf8 => 0,
            Encoding::Utf8Bom => 3,
            Encoding::Utf16Le | Encoding::Utf16Be => 2,
        }
    }

    pub fn is_utf16(self) -> bool {
        matches!(self, Encoding::Utf16Le | Encoding::Utf16Be)
    }

    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 | Encoding::Utf8Bom => "UTF-8",
            Encoding::Utf16Le => "UTF-16LE → UTF-8",
            Encoding::Utf16Be => "UTF-16BE → UTF-8",
        }
    }

    fn decoder(self) -> Option<encoding_rs::Decoder> {
        match self {
            Encoding::Utf16Le => Some(encoding_rs::UTF_16LE.new_decoder_without_bom_handling()),
            Encoding::Utf16Be => Some(encoding_rs::UTF_16BE.new_decoder_without_bom_handling()),
            _ => None,
        }
    }

    /// Decodes in-memory UTF-16 (BOM included) to UTF-8, replacing malformed units with U+FFFD.
    pub(crate) fn decode_utf16(self, bytes: &[u8]) -> String {
        let body = bytes.get(2..).unwrap_or(&[]);
        let codec = if self == Encoding::Utf16Le { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
        codec.decode_without_bom_handling(body).0.into_owned()
    }
}

/// Bytes sampled for line-ending detection (spec: first 64 KB).
pub const ENDINGS_SAMPLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LineEndings {
    None,
    Lf,
    Crlf,
    Mixed,
}

impl LineEndings {
    pub fn detect(sample: &[u8]) -> Self {
        let (mut lf, mut crlf) = (0usize, 0usize);
        for p in memchr::memchr_iter(b'\n', sample) {
            if p > 0 && sample[p - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
        match (lf, crlf) {
            (0, 0) => LineEndings::None,
            (_, 0) => LineEndings::Lf,
            (0, _) => LineEndings::Crlf,
            _ => LineEndings::Mixed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LineEndings::None => "no line breaks",
            LineEndings::Lf => "LF",
            LineEndings::Crlf => "CRLF",
            LineEndings::Mixed => "mixed line endings",
        }
    }
}

/// Status bar text, e.g. `UTF-8 · LF` or `UTF-16LE → UTF-8 · CRLF`.
pub fn status_label(encoding: Encoding, endings: LineEndings) -> String {
    format!("{} · {}", encoding.label(), endings.label())
}

/// A partial-file path unique to this writer (`<dst>.<pid>.<n>.partial`), so concurrent transcodes of
/// the same file never share or delete each other's partial output.
fn unique_partial_path(dst: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut name = dst.as_os_str().to_owned();
    name.push(format!(".{}.{}.partial", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    PathBuf::from(name)
}

/// Streams a UTF-16 file (BOM included) into UTF-8 at `dst`. Output goes to a writer-unique
/// `<dst>.<pid>.<n>.partial` and is renamed on success, so cancellation or a crash never leaves a truncated
/// `dst` (spec §4.2 atomic writes). Concurrent calls for the same `dst` are safe: each renames its own
/// complete output over `dst`.
pub fn transcode_to_utf8(
    src: &Path,
    dst: &Path,
    encoding: Encoding,
    cancel: &CancelToken,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), CoreError> {
    let mut decoder = encoding.decoder().expect("transcode_to_utf8 requires a UTF-16 encoding");
    let partial = unique_partial_path(dst);
    let result = (|| -> Result<(), CoreError> {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let mut input = File::open(src).map_err(CoreError::io(src))?;
        let total = input.metadata().map_err(CoreError::io(src))?.len();
        let mut out = BufWriter::new(File::create(&partial).map_err(CoreError::io(&partial))?);
        let mut bom = [0u8; 2];
        input.read_exact(&mut bom).map_err(CoreError::io(src))?;
        let mut done = 2u64;
        let mut buf = vec![0u8; 1 << 20];
        let mut utf8 = vec![0u8; decoder.max_utf8_buffer_length(buf.len()).unwrap_or(4 << 20)];
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let n = input.read(&mut buf).map_err(CoreError::io(src))?;
            let last = n == 0;
            let mut consumed = 0;
            loop {
                let (result, read, written, _) = decoder.decode_to_utf8(&buf[consumed..n], &mut utf8, last);
                out.write_all(&utf8[..written]).map_err(CoreError::io(&partial))?;
                consumed += read;
                if matches!(result, encoding_rs::CoderResult::InputEmpty) {
                    break;
                }
            }
            done += n as u64;
            on_progress(done.min(total), total);
            if last {
                break;
            }
        }
        out.flush().map_err(CoreError::io(&partial))?;
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&partial, dst).map_err(CoreError::io(dst)),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str, le: bool) -> Vec<u8> {
        let mut out = if le { vec![0xFF, 0xFE] } else { vec![0xFE, 0xFF] };
        for u in s.encode_utf16() {
            out.extend(if le { u.to_le_bytes() } else { u.to_be_bytes() });
        }
        out
    }

    fn assert_no_partials(dir: &Path) {
        let leftovers: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".partial"))
            .collect();
        assert!(leftovers.is_empty(), "leftover partial files: {leftovers:?}");
    }

    #[test]
    fn detects_boms() {
        assert_eq!(Encoding::detect(b"\xEF\xBB\xBFabc"), Encoding::Utf8Bom);
        assert_eq!(Encoding::detect(b"\xFF\xFEa\0"), Encoding::Utf16Le);
        assert_eq!(Encoding::detect(b"\xFE\xFF\0a"), Encoding::Utf16Be);
        assert_eq!(Encoding::detect(b"plain"), Encoding::Utf8);
        assert_eq!(Encoding::detect(b""), Encoding::Utf8);
        assert_eq!(Encoding::detect(b"\xFF"), Encoding::Utf8, "a lone 0xFF is not a BOM");
        assert_eq!([Encoding::Utf8.bom_len(), Encoding::Utf8Bom.bom_len(), Encoding::Utf16Le.bom_len()], [0, 3, 2]);
    }

    #[test]
    fn detects_line_endings() {
        assert_eq!(LineEndings::detect(b"a\nb\n"), LineEndings::Lf);
        assert_eq!(LineEndings::detect(b"a\r\nb\r\n"), LineEndings::Crlf);
        assert_eq!(LineEndings::detect(b"a\r\nb\n"), LineEndings::Mixed);
        assert_eq!(LineEndings::detect(b"no breaks\r"), LineEndings::None);
        assert_eq!(LineEndings::detect(b"\n"), LineEndings::Lf);
    }

    #[test]
    fn status_labels_match_spec() {
        assert_eq!(status_label(Encoding::Utf8, LineEndings::Lf), "UTF-8 · LF");
        assert_eq!(status_label(Encoding::Utf8Bom, LineEndings::Lf), "UTF-8 · LF");
        assert_eq!(status_label(Encoding::Utf16Le, LineEndings::Crlf), "UTF-16LE → UTF-8 · CRLF");
        assert_eq!(status_label(Encoding::Utf16Be, LineEndings::Mixed), "UTF-16BE → UTF-8 · mixed line endings");
    }

    #[test]
    fn transcodes_le_and_be_with_progress() {
        let dir = tempfile::tempdir().unwrap();
        for le in [true, false] {
            let src = dir.path().join(if le { "le.log" } else { "be.log" });
            let dst = dir.path().join(if le { "le.utf8" } else { "be.utf8" });
            std::fs::write(&src, utf16("héllo\r\nwörld ✓\n", le)).unwrap();
            let enc = Encoding::detect(&std::fs::read(&src).unwrap());
            let mut last = (0, 0);
            transcode_to_utf8(&src, &dst, enc, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
            assert_eq!(std::fs::read_to_string(&dst).unwrap(), "héllo\r\nwörld ✓\n");
            let total = std::fs::metadata(&src).unwrap().len();
            assert_eq!(last, (total, total));
            assert_no_partials(dir.path());
        }
    }

    #[test]
    fn odd_trailing_byte_becomes_replacement_char() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("odd.log");
        let dst = dir.path().join("odd.utf8");
        let mut bytes = utf16("ab", true);
        bytes.push(0x41);
        std::fs::write(&src, bytes).unwrap();
        transcode_to_utf8(&src, &dst, Encoding::Utf16Le, &CancelToken::new(), |_, _| {}).unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "ab\u{FFFD}");
    }

    #[test]
    fn cancel_leaves_no_output() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("c.log");
        let dst = dir.path().join("c.utf8");
        std::fs::write(&src, utf16("abc\n", true)).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(transcode_to_utf8(&src, &dst, Encoding::Utf16Le, &cancel, |_, _| {}), Err(CoreError::Cancelled)));
        assert!(!dst.exists());
        assert_no_partials(dir.path());
    }

    #[test]
    fn surrogate_pair_across_read_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let chunk_size = 1usize << 20; // 1 MiB

        for le in [true, false] {
            // Placement 1: Emoji (surrogate pair) starts at post-BOM offset chunk_size - 2
            // This spans the first read boundary, with the pair split across two chunks
            // Post-BOM offset X means file byte 2 + X
            // So emoji is at file bytes chunk_size to chunk_size + 3
            // First read: bytes 2 to 2 + chunk_size - 1 (includes first 2 bytes of emoji)
            // Second read starts at byte chunk_size + 2 (includes last 2 bytes of emoji)
            let padding_chars = (chunk_size - 2) / 2; // Each 'a' is 2 bytes in UTF-16
            let test_string = "a".repeat(padding_chars) + "😀";

            let src = dir.path().join(if le { "le_boundary.log" } else { "be_boundary.log" });
            let dst = dir.path().join(if le { "le_boundary.utf8" } else { "be_boundary.utf8" });

            std::fs::write(&src, utf16(&test_string, le)).unwrap();
            let enc = Encoding::detect(&std::fs::read(&src).unwrap());
            transcode_to_utf8(&src, &dst, enc, &CancelToken::new(), |_, _| {}).unwrap();

            let result = std::fs::read_to_string(&dst).unwrap();
            assert_eq!(result, test_string, "Placement 1 (pair at offset {}, LE={}) failed", chunk_size - 2, le);

            // Placement 2: Emoji ends exactly at first read boundary
            // Emoji is at post-BOM offsets chunk_size - 4 to chunk_size - 1
            // First 2 bytes in first chunk, last 2 bytes in second chunk
            let padding_chars2 = (chunk_size - 4) / 2;
            let test_string2 = "a".repeat(padding_chars2) + "😀";

            let src2 = dir.path().join(if le { "le_boundary2.log" } else { "be_boundary2.log" });
            let dst2 = dir.path().join(if le { "le_boundary2.utf8" } else { "be_boundary2.utf8" });

            std::fs::write(&src2, utf16(&test_string2, le)).unwrap();
            let enc2 = Encoding::detect(&std::fs::read(&src2).unwrap());
            transcode_to_utf8(&src2, &dst2, enc2, &CancelToken::new(), |_, _| {}).unwrap();

            let result2 = std::fs::read_to_string(&dst2).unwrap();
            assert_eq!(result2, test_string2, "Placement 2 (pair at offset {}, LE={}) failed", chunk_size - 4, le);
        }
    }

    #[test]
    fn source_shorter_than_bom_is_a_typed_error() {
        let dir = tempfile::tempdir().unwrap();

        // Test with 0 bytes
        let src0 = dir.path().join("zero.log");
        let dst0 = dir.path().join("zero.utf8");
        std::fs::write(&src0, vec![]).unwrap();

        let result = transcode_to_utf8(&src0, &dst0, Encoding::Utf16Le, &CancelToken::new(), |_, _| {});
        assert!(matches!(result, Err(CoreError::Io { .. })), "Expected Io error for 0-byte file");
        assert!(!dst0.exists(), "Destination should not exist after error");
        assert_no_partials(dir.path());

        // Test with 1 byte
        let src1 = dir.path().join("one.log");
        let dst1 = dir.path().join("one.utf8");
        std::fs::write(&src1, vec![0xFF]).unwrap();

        let result = transcode_to_utf8(&src1, &dst1, Encoding::Utf16Le, &CancelToken::new(), |_, _| {});
        assert!(matches!(result, Err(CoreError::Io { .. })), "Expected Io error for 1-byte file");
        assert!(!dst1.exists(), "Destination should not exist after error");
        assert_no_partials(dir.path());
    }

    #[test]
    fn concurrent_transcodes_of_the_same_file_both_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("race.log");
        let text = "line é ✓\r\n".repeat(50_000);
        std::fs::write(&src, utf16(&text, true)).unwrap();
        for round in 0..20 {
            let dst = dir.path().join(format!("race{round}.utf8"));
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..2)
                    .map(|_| s.spawn(|| transcode_to_utf8(&src, &dst, Encoding::Utf16Le, &CancelToken::new(), |_, _| {})))
                    .collect();
                for h in handles {
                    h.join().unwrap().unwrap();
                }
            });
            assert_eq!(std::fs::read_to_string(&dst).unwrap(), text);
        }
        assert_no_partials(dir.path());
    }
}
