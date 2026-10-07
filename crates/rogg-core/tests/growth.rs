use proptest::prelude::*;
use rogg_core::CancelToken;
use rogg_core::text::{LogText, Refresh};
use std::io::Write;
use std::path::Path;

fn open(path: &Path) -> LogText {
    LogText::open(path, &path.with_extension("cache"), &CancelToken::new(), |_| {}).unwrap()
}

fn append(path: &Path, bytes: &[u8]) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(bytes).unwrap();
}

fn lines(t: &LogText) -> Vec<String> {
    (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()
}

fn grew(t: &LogText) -> LogText {
    match t.refresh(&CancelToken::new()).unwrap() {
        Refresh::Grew(next) => next,
        Refresh::Unchanged => panic!("expected growth, got Unchanged"),
        Refresh::Truncated => panic!("expected growth, got Truncated"),
    }
}

#[test]
fn refresh_reports_growth_truncation_and_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let text = open(&path);
    assert!(matches!(text.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    append(&path, b"three\n");
    let next = grew(&text);
    assert_eq!(lines(&next), ["one", "two", "three"]);
    assert_eq!(next.index(), open(&path).index(), "the extended index equals a fresh one");
    assert_eq!(lines(&text), ["one", "two"], "the old snapshot is unchanged");
    assert!(matches!(next.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(4).unwrap();
    assert!(matches!(next.refresh(&CancelToken::new()).unwrap(), Refresh::Truncated));
    // Emptied and written again to the same length (`copytruncate` + new lines): the start changed.
    let rewritten = dir.path().join("b.log");
    std::fs::write(&rewritten, "aaaa\n").unwrap();
    let before = open(&rewritten);
    std::fs::write(&rewritten, "bbbb\ncc\n").unwrap();
    assert!(matches!(before.refresh(&CancelToken::new()).unwrap(), Refresh::Truncated), "a rewrite is not growth");
    assert_eq!(text.file_identity(), next.file_identity());
    assert!(text.file_identity().is_some());
}

#[test]
fn an_unfinished_last_line_is_completed_by_an_append() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\nerr").unwrap();
    let text = open(&path);
    assert_eq!((text.line_count(), text.stable_lines()), (2, 1));
    append(&path, b"or here\nnext");
    let next = grew(&text);
    assert_eq!(lines(&next), ["a", "error here", "next"]);
    assert_eq!(next.stable_lines(), 2);
    append(&path, b"\n");
    assert_eq!(grew(&next).stable_lines(), 3);
}

#[test]
fn empty_files_grow_and_fixed_texts_never_do() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e.log");
    std::fs::write(&path, "").unwrap();
    let empty = open(&path);
    assert_eq!((empty.line_count(), empty.stable_lines()), (0, 0));
    append(&path, b"first\n");
    assert_eq!(lines(&grew(&empty)), ["first"]);
    let memory = LogText::from_bytes(b"a\n".to_vec());
    assert!(matches!(memory.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged));
    assert!(memory.file_identity().is_none());
    let utf16 = dir.path().join("w.log");
    std::fs::write(&utf16, [0xFF, 0xFE, b'h', 0, b'\n', 0]).unwrap();
    let converted = open(&utf16);
    append(&utf16, &[b'i', 0, b'\n', 0]);
    assert!(matches!(converted.refresh(&CancelToken::new()).unwrap(), Refresh::Unchanged), "UTF-16 conversions do not follow growth");
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'x'), Just(b'\n'), Just(b'\r')], 0..300)
}

proptest! {
    /// Spec §8: incremental append == full reindex, through the file and the mapping.
    #[test]
    fn appends_equal_a_fresh_open(data in bytes(), mut cuts in proptest::collection::vec(0usize..300, 0..5)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("g.log");
        cuts.push(data.len());
        cuts.sort_unstable();
        let first = cuts[0].min(data.len());
        std::fs::write(&path, &data[..first]).unwrap();
        let mut text = open(&path);
        let mut at = first;
        for cut in cuts {
            let cut = cut.min(data.len());
            if cut > at {
                append(&path, &data[at..cut]);
                at = cut;
                text = match text.refresh(&CancelToken::new()).unwrap() {
                    Refresh::Grew(next) => next,
                    _ => panic!("an append must grow the text"),
                };
            }
        }
        let fresh = LogText::from_bytes(data.clone());
        prop_assert_eq!(lines(&text), lines(&fresh));
        prop_assert_eq!(text.index().longest_line(), fresh.index().longest_line());
    }
}
