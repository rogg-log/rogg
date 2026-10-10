mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
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
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
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
    // The final view (and the statistics that follow it) is large, so it lands off the UI thread after the scan.
    common::wait_until(cx, |cx| {
        doc.read_with(cx, |d, _| {
            d.filter.progress.is_none()
                && d.filter.matches.len() == 1_501_001
                && d.filter.stats.as_ref().is_some_and(|s| s.term_hits[0].lines == 1_501_001)
        })
    });
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
fn a_reload_shows_no_stale_view_while_the_new_text_loads(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    // Every open runs in the background, so the reload has a window in which the new text is not there yet.
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let path = common::write_log(dir.path(), "stale.log", &["a", "a", "a", ""]);
    let doc = doc_for(cx, &path, "a");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.shown_lines().len() == 3));
    let after_reload = Rc::new(RefCell::new(None));
    let (seen, handle) = (after_reload.clone(), doc.clone());
    cx.update(|cx| {
        cx.subscribe(&doc, move |_, ev: &DocumentEvent, cx| {
            if *ev == DocumentEvent::Reloaded {
                let d = handle.read(cx);
                *seen.borrow_mut() = Some((d.filter.view.shown_lines().len(), d.filter.stats.is_some()));
            }
        })
        .detach()
    });
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    append(&path, "b\n");
    common::wait_until(cx, |_| after_reload.borrow().is_some());
    assert_eq!(*after_reload.borrow(), Some((0, false)), "the old view and stats are gone as soon as the reload starts");
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

#[gpui_kit::test]
fn an_idle_followed_file_polls_slowly(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    // sync_bytes 0 puts every search on a background thread, so the busy interval is observable.
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let log = common::write_log(dir.path(), "idle.log", &["a", "b"]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log], ..Default::default() });
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some() && d.filter.progress.is_none()));
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.poll_interval() == rogg_app::document::FOLLOW_POLL));
    doc.update(cx, |d, cx| d.set_search(Pane::Full, "a", cx));
    assert_eq!(doc.read_with(cx, |d, _| d.poll_interval()), rogg_app::document::POLL_INTERVAL, "a running search polls every frame");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.poll_interval() == rogg_app::document::FOLLOW_POLL));
}

#[gpui_kit::test]
fn truncating_a_large_file_mid_scan_reloads_and_renders(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..400_000).map(|i| format!("line {i} error padding padding padding")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &refs);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log.clone()], ..Default::default() });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    std::fs::OpenOptions::new().write(true).open(&log).unwrap().set_len(0).unwrap();
    std::fs::write(&log, "fresh\n").unwrap();
    for _ in 0..20 {
        common::render(cx, w);
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_count() == 1)));
    common::render(cx, w);
    assert!(ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains("truncated or replaced"))));
}
