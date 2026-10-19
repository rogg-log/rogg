mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext, px};
use rogg_app::args::Args;
use rogg_app::recent::RecentStore;
use rogg_app::workspace::Workspace;
use rogg_ui::layout::MainWindowParts;

fn open_two(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let a = common::write_log(dir.path(), "a.log", &["alpha one", "beta two"]);
    let b = common::write_log(dir.path(), "b.log", &["gamma"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![a, b], ..Default::default() });
    (dir, w, ws)
}

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> (usize, usize) {
    ws.read_with(cx, |ws, _| (ws.tabs().len(), ws.active_index()))
}

#[gpui_kit::test]
fn tabs_switch_close_and_reopen(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_two(cx);
    assert_eq!(active(&ws, cx), (2, 1));
    common::click(cx, w, "tab-0");
    assert_eq!(active(&ws, cx), (2, 0));
    common::press(cx, w, "cmd-2");
    assert_eq!(active(&ws, cx), (2, 1));
    common::press(cx, w, "ctrl-tab");
    assert_eq!(active(&ws, cx), (2, 0), "next wraps around");
    let b = dir.path().join("b.log");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![b], window, cx))).unwrap();
    assert_eq!(active(&ws, cx), (2, 1), "an open file activates its tab");
    common::click(cx, w, "tab-close-1");
    assert_eq!(active(&ws, cx), (1, 0));
    common::press(cx, w, "cmd-w");
    assert_eq!(active(&ws, cx), (0, 0));
    assert!(common::exists(cx, w, "welcome"));
}

#[gpui_kit::test]
fn sidebar_lists_open_files_by_folder(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    assert!(common::exists(cx, w, "sidebar"));
    assert!(common::exists(cx, w, "tree-row-0"), "the folder row");
    assert!(common::exists(cx, w, "tree-row-2") && !common::exists(cx, w, "tree-row-3"));
    common::click(cx, w, "tree-row-1");
    assert_eq!(active(&ws, cx), (2, 0));
    common::click(cx, w, "tree-row-0");
    assert!(!common::exists(cx, w, "tree-row-1"), "the folder collapses");
    common::press(cx, w, "cmd-\\");
    assert!(!common::exists(cx, w, "sidebar"));
}

#[gpui_kit::test]
fn recent_files_are_recorded_persisted_and_pruned(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_two(cx);
    cx.update(|cx| {
        let entries = cx.global::<RecentStore>().entries();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].path.ends_with("b.log"), "newest first");
    });
    assert!(dir.path().join("recent.json").exists());
    assert!(common::exists(cx, w, "recent-1"));
    std::fs::remove_file(dir.path().join("a.log")).unwrap();
    common::click(cx, w, "recent-1");
    cx.update(|cx| assert_eq!(cx.global::<RecentStore>().entries().len(), 1));
    assert!(ws.read_with(cx, |ws, _| !ws.toasts.is_empty()), "the removal is announced");
}

#[gpui_kit::test]
fn rendering_recent_never_stats_the_filesystem(cx: &mut TestAppContext) {
    let (dir, w, _ws) = open_two(cx);
    std::fs::remove_file(dir.path().join("a.log")).unwrap();
    common::render(cx, w);
    let missing: Vec<bool> = cx.update(|cx| rogg_app::workspace::chrome::recent_items(cx).iter().map(|r| r.missing).collect());
    assert_eq!(missing, [false, false], "the render path reads the cache, so a file deleted since is not noticed yet");
    common::click(cx, w, "recent-1");
    cx.update(|cx| {
        let store = cx.global::<RecentStore>();
        assert_eq!(store.entries().len(), 1, "the click-time stat catches it and removes the entry");
        assert!(store.entries()[0].path.ends_with("b.log"));
    });
}

#[gpui_kit::test]
fn drag_handles_resize_panes_and_sidebar(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let band = common::bounds(cx, w, "filter-band");
    ws.update(cx, |ws, cx| ws.drag_band(band.top() - px(100.), cx));
    let split = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).split);
    assert!(split < MainWindowParts::DEFAULT_SPLIT - 0.05, "split {split}");
    ws.update(cx, |ws, cx| ws.drag_sidebar(px(1000.), cx));
    assert_eq!(ws.read_with(cx, |ws, _| ws.sidebar_width), px(360.));
    ws.update(cx, |ws, cx| ws.drag_sidebar(px(10.), cx));
    assert_eq!(ws.read_with(cx, |ws, _| ws.sidebar_width), px(155.));
}

#[gpui_kit::test]
fn history_dialog_restores_an_older_filter(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("gamma", cx)).unwrap();
    common::click(cx, w, "history");
    assert!(common::exists(cx, w, "dialog"));
    common::click(cx, w, "history-pick-1");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "alpha");
    assert!(!common::exists(cx, w, "dialog"));
}

#[gpui_kit::test]
fn restoring_history_closes_the_expression_editor(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("gamma", cx)).unwrap();
    cx.update_window(w, |_, window, cx| tab.update(cx, |t, cx| t.start_edit(window, cx))).unwrap();
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()));
    common::click(cx, w, "history");
    common::click(cx, w, "history-pick-1");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "alpha");
    assert!(tab.read_with(cx, |t, _| t.editing.is_none()), "the editor showing the old expression is closed");
}

#[gpui_kit::test]
fn all_tabs_dialog_activates_a_tab(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "all-tabs");
    assert!(common::exists(cx, w, "dialog"));
    common::click(cx, w, "picker-0");
    assert_eq!(active(&ws, cx), (2, 0));
    assert!(!common::exists(cx, w, "dialog"));
}

#[gpui_kit::test]
fn unreadable_recent_and_history_files_toast_at_start(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("recent.json"), "{ not json").unwrap();
    std::fs::write(dir.path().join("history.json"), "[[[").unwrap();
    common::init(cx, rogg_app::paths::AppPaths::under(dir.path()));
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![], ..Default::default() });
    let toasts: Vec<String> = ws.read_with(cx, |ws, _| ws.toasts.iter().map(|(_, m)| m.to_string()).collect());
    assert_eq!(toasts.len(), 2, "{toasts:?}");
    assert!(toasts.iter().any(|m| m.contains("history.json.bak")) && toasts.iter().any(|m| m.contains("recent.json.bak")));
    assert!(dir.path().join("recent.json.bak").exists());
}

#[gpui_kit::test]
fn one_tab_per_file_whatever_the_path_spelling(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let raw = dir.path().join("sub/../late.log");
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![raw], ..Default::default() });
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1, "a missing file opens an error tab");
    common::write_log(dir.path(), "late.log", &["here"]);
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    doc.update(cx, |d, cx| d.retry(cx));
    let canonical = dir.path().join("late.log").canonicalize().unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![canonical], window, cx))).unwrap();
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1, "the same file activates its tab");
}
