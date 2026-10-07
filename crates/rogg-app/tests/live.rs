mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentEvent, DocumentInit, EngineConfig, Pane, Source};
use rogg_core::filter::FilterSpec;
use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

fn append(path: &Path, s: &str) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(s.as_bytes()).unwrap();
}

fn doc_for(cx: &mut TestAppContext, path: &Path, expr: &str) -> Entity<Document> {
    let init = DocumentInit { spec: FilterSpec::from_expression(expr), ..Default::default() };
    cx.new(|cx| Document::new(Source::File(path.to_path_buf()), init, cx))
}

fn line_count(doc: &Entity<Document>, cx: &mut TestAppContext) -> u64 {
    doc.read_with(cx, |d, _| d.text().map_or(0, |t| t.line_count()))
}

fn matched(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.matches.to_vec())
}

fn events(cx: &mut TestAppContext, doc: &Entity<Document>) -> Rc<RefCell<Vec<DocumentEvent>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    cx.update(|cx| cx.subscribe(doc, move |_, ev: &DocumentEvent, _| s.borrow_mut().push(ev.clone())).detach());
    seen
}

#[gpui_kit::test]
fn appended_lines_are_indexed_filtered_and_counted(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "live.log", &["error one", "ok", ""]);
    let doc = doc_for(cx, &path, "error");
    let seen = events(cx, &doc);
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "ok", cx));
    assert_eq!(matched(&doc, cx), [0]);
    assert!(!doc.read_with(cx, |d, _| d.is_live()));
    append(&path, "ok again\nerror two\n");
    common::wait_until(cx, |cx| line_count(&doc, cx) == 4);
    assert_eq!(matched(&doc, cx), [0, 3]);
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.scans, d.filter.tail_scans)), (1, 1), "only the appended lines were scanned");
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Full).result.as_ref().map(|r| r.lines.clone())), Some(vec![1, 2]));
    assert!(doc.read_with(cx, |d, _| d.is_live()));
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 1);
    assert!(seen.borrow().contains(&DocumentEvent::Grew { lines: 2 }));
}

#[gpui_kit::test]
fn an_unfinished_last_line_is_rescanned_when_completed(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("p.log");
    std::fs::write(&path, "a\nerr").unwrap();
    let doc = doc_for(cx, &path, "error");
    assert!(matched(&doc, cx).is_empty());
    append(&path, "or here\n");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_text(1) == "error here")));
    assert_eq!(matched(&doc, cx), [1]);
    let hits = doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().term_hits[0].lines);
    assert_eq!(hits, 1, "the completed line counts once");
}

#[gpui_kit::test]
fn appends_during_a_running_scan_are_scanned_once(cx: &mut TestAppContext) {
    // 3M lines take about a second to scan in a debug build, far longer than the follower needs to report growth
    // (debounce plus file-system latency), so the growth always lands while the first scan still runs.
    const BASE: u64 = 3_000_000;
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let mut body: String = (0..BASE).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    // An unfinished last line the append completes: it must be scanned again and match on its full text.
    body.push_str("alp");
    let path = dir.path().join("big.log");
    std::fs::write(&path, body).unwrap();
    let doc = doc_for(cx, &path, "alpha");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_some()));
    append(&path, &format!("ha tail\n{}", "alpha tail\n".repeat(1000)));
    let mut scan_running_at_growth = None;
    common::wait_until(cx, |cx| {
        doc.read_with(cx, |d, _| {
            let grown = d.text().is_some_and(|t| t.line_count() > BASE + 1);
            if grown {
                scan_running_at_growth = Some(d.filter.progress.is_some());
            }
            grown
        })
    });
    assert_eq!(scan_running_at_growth, Some(true), "the growth must arrive while the first scan runs");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && d.filter.matches.len() == 1_501_001));
    let expected: Vec<u64> = (0..BASE).step_by(2).chain(BASE..=BASE + 1000).collect();
    assert!(matched(&doc, cx) == expected, "exact matches: the even lines, the completed line and the appended ones");
    let hits = doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().term_hits[0].lines);
    assert_eq!(hits, 1_501_001);
    assert_eq!(line_count(&doc, cx), BASE + 1001);
    assert_eq!(
        doc.read_with(cx, |d, _| (d.filter.scans, d.filter.tail_scans)),
        (1, 1),
        "one full scan, then one tail scan of the appended lines"
    );
}

#[gpui_kit::test]
fn truncation_reloads_with_a_toast_and_drops_pins(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "t.log", &["a", "b", "c", "d", ""]);
    let doc = doc_for(cx, &path, "a");
    let seen = events(cx, &doc);
    doc.update(cx, |d, cx| d.toggle_pin(2, cx));
    // `logrotate copytruncate`: the file is emptied in place, then written again.
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    append(&path, "new one\n");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some()));
    assert!(seen.borrow().contains(&DocumentEvent::Reloaded));
    assert!(doc.read_with(cx, |d, _| d.filter.pins.is_empty()), "pins referred to the old content");
    common::wait_until(cx, |cx| line_count(&doc, cx) == 1);
}

#[gpui_kit::test]
fn a_replaced_file_is_reloaded(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "r.log", &["old", ""]);
    let doc = doc_for(cx, &path, "new");
    let fresh = common::write_log(dir.path(), "r.log.1", &["new one", "new two", ""]);
    std::fs::rename(&fresh, &path).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some_and(|t| t.line_count() == 2)));
    assert_eq!(matched(&doc, cx), [0, 1]);
}
