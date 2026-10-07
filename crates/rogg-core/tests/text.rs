use rogg_core::CancelToken;
use rogg_core::encoding::{Encoding, LineEndings};
use rogg_core::text::{LogText, OpenProgress};
use std::path::Path;

fn open(path: &Path, cache: &Path) -> LogText {
    LogText::open(path, cache, &CancelToken::new(), |_| {}).unwrap()
}

fn lines(t: &LogText) -> Vec<String> {
    (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()
}

#[test]
fn plain_file_lines_and_labels() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.log");
    std::fs::write(&p, "one\ntwo\nthree").unwrap();
    let t = open(&p, &dir.path().join("cache"));
    assert_eq!(lines(&t), ["one", "two", "three"]);
    assert_eq!(t.status_label(), "UTF-8 · LF");
    assert_eq!(t.source_path(), Some(p.as_path()));
    assert_eq!(t.line(99), b"", "out of range is empty, not a panic");
}

#[test]
fn degenerate_files() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    for (name, content, expected) in [("empty", "", vec![]), ("nl", "\n", vec![""]), ("two", "\n\n", vec!["", ""])] {
        let p = dir.path().join(name);
        std::fs::write(&p, content).unwrap();
        assert_eq!(lines(&open(&p, &cache)), expected, "{name}");
    }
}

#[test]
fn crlf_is_stripped_from_lines_and_endings_reported() {
    let t = LogText::from_bytes(b"error\r\nok\r\nlast\r".to_vec());
    assert_eq!(t.line(0), b"error");
    assert_eq!(t.line(2), b"last");
    assert_eq!(t.endings(), LineEndings::Crlf);
    assert_eq!(LogText::from_bytes(b"a\r\nb\n".to_vec()).endings(), LineEndings::Mixed);
}

#[test]
fn utf8_bom_is_not_part_of_line_one() {
    let t = LogText::from_bytes(b"\xEF\xBB\xBFfirst\nsecond".to_vec());
    assert_eq!(t.line(0), b"first");
    assert_eq!(t.encoding(), Encoding::Utf8Bom);
    assert_eq!(t.bytes(), b"first\nsecond");
}

#[test]
fn utf16_file_is_transcoded_into_cache_and_reused() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let p = dir.path().join("w.log");
    let mut bytes = vec![0xFF, 0xFE];
    for u in "héllo\r\nwörld\r\n".encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    std::fs::write(&p, bytes).unwrap();
    let mut saw_transcode = false;
    let t = LogText::open(&p, &cache, &CancelToken::new(), |e| saw_transcode |= matches!(e, OpenProgress::Transcoding { .. })).unwrap();
    assert!(saw_transcode);
    assert_eq!(lines(&t), ["héllo", "wörld"]);
    assert_eq!(t.status_label(), "UTF-16LE → UTF-8 · CRLF");
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
    let mut transcoded_again = false;
    let again = LogText::open(&p, &cache, &CancelToken::new(), |e| transcoded_again |= matches!(e, OpenProgress::Transcoding { .. })).unwrap();
    assert!(!transcoded_again, "unchanged file reuses the cached transcode");
    assert_eq!(lines(&again), ["héllo", "wörld"]);
}

#[test]
fn invalid_utf8_is_lossy_for_display_but_raw_for_matching() {
    let t = LogText::from_bytes(b"ok \xFF\xFE error\n".to_vec());
    assert_eq!(t.line(0), b"ok \xFF\xFE error");
    assert_eq!(t.line_text(0), "ok \u{FFFD}\u{FFFD} error");
}

#[test]
fn span_covers_contiguous_lines() {
    let t = LogText::from_bytes(b"aa\nbbb\ncc\n".to_vec());
    let (bytes, base) = t.span(1..3);
    assert_eq!(bytes, b"bbb\ncc");
    assert_eq!(base, 3);
    assert_eq!(t.span(2..2), (&b""[..], 0));
}

#[test]
fn indexing_progress_reaches_total_and_cancel_errors() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("p.log");
    std::fs::write(&p, "x\n".repeat(10_000)).unwrap();
    let mut last = None;
    LogText::open(&p, dir.path(), &CancelToken::new(), |e| last = Some(e)).unwrap();
    assert_eq!(last, Some(OpenProgress::Indexing { done: 20_000, total: 20_000 }));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(LogText::open(&p, dir.path(), &cancel, |_| {}).is_err());
}

#[test]
fn missing_file_is_a_typed_io_error() {
    let err = LogText::open(Path::new("/definitely/not/here.log"), Path::new("/tmp"), &CancelToken::new(), |_| {}).err().unwrap();
    assert!(matches!(err, rogg_core::CoreError::Io { .. }));
}

#[test]
fn log_text_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LogText>();
}

#[test]
fn span_clamps_out_of_range_lines() {
    let t = LogText::from_bytes(b"aa\nbbb\ncc\n".to_vec());
    assert_eq!(t.span(1..99), (&b"bbb\ncc"[..], 3));
    assert_eq!(t.span(50..99), (&b""[..], 0));
    let (start, end) = (2, 1);
    assert_eq!(t.span(start..end), (&b""[..], 0), "reversed range is empty");
    assert_eq!(t.span(0..3).0, b"aa\nbbb\ncc");
}

#[test]
fn from_bytes_decodes_utf16() {
    let mut bytes = vec![0xFE, 0xFF];
    for u in "ab\ncd".encode_utf16() {
        bytes.extend(u.to_be_bytes());
    }
    let t = LogText::from_bytes(bytes);
    assert_eq!(t.encoding(), Encoding::Utf16Be);
    assert_eq!(t.status_label(), "UTF-16BE → UTF-8 · LF");
    assert_eq!(lines(&t), ["ab", "cd"]);
}

#[test]
fn opened_utf8_bom_file_skips_bom_and_bom_only_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let p = dir.path().join("bom.log");
    std::fs::write(&p, b"\xEF\xBB\xBFfirst\nsecond").unwrap();
    let t = open(&p, &cache);
    assert_eq!(t.line(0), b"first");
    assert_eq!(t.bytes(), b"first\nsecond");
    let q = dir.path().join("bom-only.log");
    std::fs::write(&q, b"\xEF\xBB\xBF").unwrap();
    let t = open(&q, &cache);
    assert_eq!(t.line_count(), 0);
    assert_eq!(t.bytes(), b"");
    assert!(!cache.exists(), "UTF-8 files never touch the cache dir");
}

#[test]
fn changed_utf16_file_gets_a_new_cache_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let p = dir.path().join("w.log");
    let utf16 = |s: &str| {
        let mut b = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            b.extend(u.to_le_bytes());
        }
        b
    };
    std::fs::write(&p, utf16("one\n")).unwrap();
    assert_eq!(lines(&open(&p, &cache)), ["one"]);
    std::fs::write(&p, utf16("one\ntwo\n")).unwrap();
    assert_eq!(lines(&open(&p, &cache)), ["one", "two"]);
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 2);
}

#[test]
fn only_top_level_cache_entries_are_converted_under_their_stem() {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    std::fs::create_dir_all(cache.join("streams")).unwrap();
    let utf16 = |s: &str| {
        let mut b = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            b.extend(u.to_le_bytes());
        }
        b
    };
    // A cache entry is immutable and origin-keyed already: its conversion is named after it.
    let entry = cache.join("0123456789abcdef.log");
    std::fs::write(&entry, utf16("entry\n")).unwrap();
    assert_eq!(lines(&open(&entry, &cache)), ["entry"]);
    assert!(cache.join("0123456789abcdef.utf8").exists());
    // A capture is named by pid and counter, which a later run can reuse for other data: it keeps the origin key.
    let capture = cache.join("streams").join("stream-7-0.log");
    std::fs::write(&capture, utf16("capture\n")).unwrap();
    assert_eq!(lines(&open(&capture, &cache)), ["capture"]);
    assert!(!cache.join("stream-7-0.utf8").exists(), "the capture's name is not a key");
    let converted: Vec<_> = std::fs::read_dir(&cache).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".utf8")).collect();
    assert_eq!(converted.len(), 2, "{converted:?}");
}
