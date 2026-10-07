mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext, point, px};
use rogg_app::args::Args;
use rogg_app::document::{DocumentInit, EngineConfig, Pane, Source};
use rogg_app::tab::{Tab, TabInit};
use rogg_core::filter::FilterSpec;
use rogg_app::workspace::Workspace;

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn open_one(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &LINES);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().expect("a tab"));
    (dir, w, ws, tab)
}

fn selected(tab: &Entity<Tab>, pane: Pane, cx: &mut TestAppContext) -> Option<u64> {
    tab.read_with(cx, |t, cx| t.state(pane).read(cx).selected())
}

fn click_at(cx: &mut TestAppContext, w: AnyWindowHandle, id: &'static str, x: f32, y: f32) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at(id, point(px(x), px(y)), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_file_opens_into_both_panes_with_the_reference_chrome(cx: &mut TestAppContext) {
    let (_dir, w, _, _) = open_one(cx);
    for id in ["titlebar", "tab-shell-0", "meta", "pane-top-head", "log-top", "filter-band", "band-handle", "pane-bottom-head", "statusbar"] {
        assert!(common::exists(cx, w, id), "missing {id}");
    }
    assert!(common::exists(cx, w, "empty-results"), "no filter yet: the filtered pane shows the no-matches state");
}

#[gpui_kit::test]
fn filter_follow_selection_and_pins(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    assert!(common::exists(cx, w, "log-bottom"));
    // filtered rows: line 2, gap, line 4, gap, line 6, line 7 (display numbers)
    assert_eq!(tab.read_with(cx, |t, cx| t.filtered.read(cx).rows().row_count()), 6);
    click_at(cx, w, "log-bottom", 300., 4. + 10.);
    assert_eq!(selected(&tab, Pane::Filtered, cx), Some(2));
    assert_eq!(selected(&tab, Pane::Full, cx), Some(2), "follow selection mirrors into the full log");
    // full-log row 4 is line 5 ("ok"), which the filter hides
    click_at(cx, w, "log-top", 300., 4. + 4. * 23. + 10.);
    assert_eq!(selected(&tab, Pane::Full, cx), Some(5));
    assert_eq!(selected(&tab, Pane::Filtered, cx), None, "a hidden line clears the filtered selection");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(5));
    // the gutter pins it, and the filtered view shows it without a rescan
    let scans = doc.read_with(cx, |d, _| d.filter.scans);
    click_at(cx, w, "log-top", 10., 4. + 4. * 23. + 10.);
    assert!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().contains(&4)));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.scans), scans);
    tab.update(cx, |t, cx| t.toggle_follow(cx));
    click_at(cx, w, "log-bottom", 300., 4. + 10.);
    assert_eq!(selected(&tab, Pane::Full, cx), Some(5), "with follow off the full log keeps its selection");
}

#[gpui_kit::test]
fn find_counts_and_enter_steps_through_hits(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let input = tab.read_with(cx, |t, _| t.find[0].clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        input.update(cx, |i, cx| i.focus(window, cx));
        window.input("error", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Full).label().map(|s| s.to_string())).as_deref(), Some("3 hits"));
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(2));
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(6));
    common::press(cx, w, "shift-enter");
    assert_eq!(tab.read_with(cx, |t, _| t.selection()), Some(2));
}

#[gpui_kit::test]
fn invalid_regex_keeps_results_and_editor(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| {
        d.set_expression("error", cx).unwrap();
        d.toggle_regex(cx);
    });
    cx.update_window(w, |_, window, cx| tab.update(cx, |t, cx| t.start_edit(window, cx))).unwrap();
    assert!(common::exists(cx, w, "expression"));
    let input = tab.read_with(cx, |t, _| t.editing.clone().expect("editing"));
    cx.update_window(w, |_, window, cx| input.update(cx, |i, cx| i.set_value("error || (", window, cx))).unwrap();
    common::click(cx, w, "apply");
    assert!(common::exists(cx, w, "filter-error"));
    assert!(common::exists(cx, w, "expression"), "the editor stays open");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone()), [1, 5, 6]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "error");
}

#[gpui_kit::test]
fn missing_file_shows_error_state_with_retry(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("later.log");
    let (w, _) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    assert!(common::exists(cx, w, "error"));
    assert!(!common::exists(cx, w, "log-top"));
    std::fs::write(&path, "now here").unwrap();
    common::click(cx, w, "retry");
    assert!(common::exists(cx, w, "log-top"));
}

#[gpui_kit::test]
fn a_selection_shown_after_a_background_scan_is_selected_in_the_filtered_pane(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..50_000).map(|i| if i % 100 == 0 { format!("needle {i}") } else { format!("hay {i}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let path = common::write_log(dir.path(), "big.log", &refs);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let init = TabInit { doc: DocumentInit { spec: FilterSpec::from_expression("needle"), ..Default::default() }, selection: Some(201), ..Default::default() };
    let tab = cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_source(Source::File(path), init, window, cx))).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).filter.progress.is_none() && t.doc.read(cx).text().is_some()));
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.filtered.read(cx).selected() == Some(201)));
}
