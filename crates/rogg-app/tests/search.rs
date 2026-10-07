mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, Pane, Source};
use std::sync::Arc;

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn doc(cx: &mut TestAppContext, text: String) -> Entity<Document> {
    let bytes = Arc::new(text.into_bytes());
    cx.new(|cx| Document::new(Source::Memory { name: "mem.log".into(), bytes }, DocumentInit::default(), cx))
}

fn label(doc: &Entity<Document>, pane: Pane, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| d.search(pane).label().map(|s| s.to_string()))
}

#[gpui_kit::test]
fn full_log_search_counts_and_steps_through_hits(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    assert_eq!(label(&doc, Pane::Full, cx), None);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "error", cx));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("3 hits"));
    doc.read_with(cx, |d, _| {
        assert_eq!(d.next_hit(Pane::Full, None, false), Some(1));
        assert_eq!(d.next_hit(Pane::Full, Some(1), false), Some(5));
        assert_eq!(d.next_hit(Pane::Full, Some(6), false), Some(1), "wraps forward");
        assert_eq!(d.next_hit(Pane::Full, Some(1), true), Some(6), "wraps backward");
    });
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "", cx));
    assert_eq!(label(&doc, Pane::Full, cx), None);
}

#[gpui_kit::test]
fn filtered_search_follows_the_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    doc.update(cx, |d, cx| d.set_expression("timeout", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_search(Pane::Filtered, "error", cx));
    assert_eq!(label(&doc, Pane::Filtered, cx).as_deref(), Some("1 hit"));
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    assert_eq!(label(&doc, Pane::Filtered, cx).as_deref(), Some("3 hits"), "recounted for the new view");
}

#[gpui_kit::test]
fn rows_outline_the_query(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx, LINES.join("\n"));
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "disk", cx));
    let rows = doc.read_with(cx, |d, _| d.full_rows().unwrap());
    use rogg_ui::logview::{LogRow, LogRowSource, SpanStyle};
    let LogRow::Line(line) = rows.row(1) else { panic!("line") };
    assert!(line.spans.iter().any(|s| s.style == SpanStyle::SearchHit && s.range == (6..10)));
}

#[gpui_kit::test]
fn large_texts_search_in_the_background(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let text: String = (0..50_000).map(|i| if i % 10 == 0 { "find me\n" } else { "other\n" }).collect();
    let doc = doc(cx, text);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "find", cx));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("…"));
    common::wait_until(cx, |cx| label(&doc, Pane::Full, cx).as_deref() != Some("…"));
    assert_eq!(label(&doc, Pane::Full, cx).as_deref(), Some("5,000 hits"));
}
