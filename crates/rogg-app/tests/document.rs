mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, LoadState, Source};

fn open(cx: &mut TestAppContext, source: Source) -> Entity<Document> {
    cx.new(|cx| Document::new(source, DocumentInit::default(), cx))
}

fn lines(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<u64> {
    doc.read_with(cx, |d, _| d.text().map(|t| t.line_count()))
}

fn failure(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| match &d.load {
        LoadState::Failed { message } => Some(message.to_string()),
        _ => None,
    })
}

#[gpui_kit::test]
fn small_files_open_synchronously(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "a.log", &["one", "two", "three"]);
    let doc = open(cx, Source::File(path));
    assert_eq!(lines(&doc, cx), Some(3));
    assert_eq!(doc.read_with(cx, |d, _| (d.title().to_string(), d.text_revision(), d.number_base())), ("a.log".to_string(), 1, 1));
}

#[gpui_kit::test]
fn large_files_open_in_the_background(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let path = common::write_log(dir.path(), "big.log", &["x"; 1000]);
    let doc = open(cx, Source::File(path));
    assert!(matches!(doc.read_with(cx, |d, _| d.load.clone()), LoadState::Loading { .. }));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    assert_eq!(lines(&doc, cx), Some(1000));
}

#[gpui_kit::test]
fn empty_and_missing_files(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let empty = common::write_log(dir.path(), "empty.log", &[]);
    assert_eq!(lines(&open(cx, Source::File(empty)), cx), Some(0));
    let later = dir.path().join("later.log");
    let doc = open(cx, Source::File(later.clone()));
    let message = failure(&doc, cx).expect("missing file fails");
    assert!(message.contains("No such file"), "{message}");
    std::fs::write(&later, "back").unwrap();
    doc.update(cx, |d, cx| d.retry(cx));
    assert_eq!(lines(&doc, cx), Some(1));
}

#[gpui_kit::test]
fn cancelling_a_background_open_fails_with_a_retry(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let path = common::write_log(dir.path(), "big.log", &["x"; 1000]);
    let doc = open(cx, Source::File(path));
    doc.update(cx, |d, cx| d.cancel_load(cx));
    assert_eq!(failure(&doc, cx).as_deref(), Some("Opening cancelled"));
    doc.update(cx, |d, cx| d.retry(cx));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
}

#[gpui_kit::test]
fn memory_sources_load_immediately(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let bytes = std::sync::Arc::new(b"a\nb".to_vec());
    let doc = cx.new(|cx| Document::new(Source::Memory { name: "mem".into(), bytes }, DocumentInit { number_base: 100 }, cx));
    assert_eq!(lines(&doc, cx), Some(2));
    assert_eq!(doc.read_with(cx, |d, _| (d.title().to_string(), d.number_base())), ("mem".to_string(), 100));
}
