mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::workspace::Workspace;
use rogg_core::session::Session;

fn open_filtered(cx: &mut TestAppContext, lines: &[&str], expr: &str) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression(expr, cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some() && d.filter.progress.is_none()));
    (dir, w, ws)
}

fn toasted(ws: &Entity<Workspace>, cx: &mut TestAppContext, needle: &str) -> bool {
    ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains(needle)))
}

#[gpui_kit::test]
fn exports_filtered_lines_in_the_background(cx: &mut TestAppContext) {
    let (dir, _w, ws) = open_filtered(cx, &["error a", "ok", "error b"], "error");
    let dst = dir.path().join("out.log");
    ws.update(cx, |ws, cx| ws.export_lines_to(dst.clone(), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Exported 2 lines to out.log"));
    assert_eq!(std::fs::read_to_string(&dst).unwrap(), "error a\n··· 1 line omitted\nerror b\n");
    ws.update(cx, |ws, cx| {
        ws.export_options.numbers = true;
        ws.export_options.gaps = false;
        ws.export_lines_to(dst.clone(), cx);
    });
    common::wait_until(cx, |_| std::fs::read_to_string(&dst).is_ok_and(|s| s.starts_with("1  ")));
    assert_eq!(std::fs::read_to_string(&dst).unwrap(), "1  error a\n3  error b\n");
}

#[gpui_kit::test]
fn export_errors_toast_and_closing_cancels(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let (dir, w, ws) = open_filtered(cx, &["error a"], "error");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    ws.update(cx, |ws, cx| ws.export_lines_to(locked.join("out.log"), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Could not export"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(std::fs::read_dir(&locked).unwrap().count(), 0);
    let big: Vec<String> = (0..300_000).map(|i| format!("error line {i}")).collect();
    let big_refs: Vec<&str> = big.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &big_refs);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![log], window, cx))).unwrap();
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some() && d.filter.progress.is_none()));
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    ws.update(cx, |ws, cx| ws.export_lines_to(out.join("big-filtered.log"), cx));
    // Handles held by the test would keep the document (and so its export) alive past the close.
    drop(doc);
    drop(tab);
    common::press(cx, w, "cmd-w");
    cx.run_until_parked();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let leftovers: Vec<_> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".partial")).collect();
    assert!(leftovers.is_empty(), "closing the tab cancels its export and removes the partial file: {leftovers:?}");}

#[gpui_kit::test]
fn a_failure_after_the_partial_was_written_leaves_nothing_behind(cx: &mut TestAppContext) {
    let (dir, _w, ws) = open_filtered(cx, &["error a"], "error");
    // Renaming the finished partial onto an existing directory fails.
    let taken = dir.path().join("taken");
    std::fs::create_dir(&taken).unwrap();
    ws.update(cx, |ws, cx| ws.export_lines_to(taken.clone(), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Could not export"));
    let partials: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".partial")).collect();
    assert!(partials.is_empty(), "the partial file is removed: {partials:?}");
    assert!(taken.is_dir() && std::fs::read_dir(&taken).unwrap().next().is_none(), "the directory is untouched");
}

#[gpui_kit::test]
fn export_waits_for_the_filtered_view_to_be_built(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    // 100 matches: the context-free view is built at once, a ±2 view off the UI thread.
    cx.update(|cx| cx.set_global(rogg_app::document::EngineConfig { view_sync_work: 150, ..Default::default() }));
    let lines: Vec<&str> = (0..1000).map(|i| if i % 10 == 5 { "match here" } else { "filler" }).collect();
    let log = common::write_log(dir.path(), "a.log", &lines);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some()));
    doc.update(cx, |d, cx| d.set_expression("match", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter_settled()));
    let dst = dir.path().join("out.log");
    ws.update(cx, |ws, cx| {
        ws.active_tab().unwrap().read(cx).doc.clone().update(cx, |d, cx| d.set_context(2, cx));
        ws.export_lines_to(dst.clone(), cx);
    });
    assert!(toasted(&ws, cx, "Filtering is still running — export when it finishes"), "the stale view is not exported");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter_settled()));
    cx.run_until_parked();
    assert!(!dst.exists(), "nothing was written");
}

#[gpui_kit::test]
fn the_export_dialog_sets_options_and_copies_the_expression(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_filtered(cx, &["error a", "timeout"], "error || timeout");
    common::press(cx, w, "cmd-e");
    assert!(common::exists(cx, w, "export-lines"));
    common::click(cx, w, "export-numbers");
    common::click(cx, w, "export-context");
    let options = ws.read_with(cx, |ws, _| ws.export_options);
    assert!(options.numbers && !options.context);
    common::click(cx, w, "export-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("error || timeout"));
    assert!(toasted(&ws, cx, "Filter expression copied"));
    common::click(cx, w, "export-scope-1");
    assert!(ws.read_with(cx, |ws, _| ws.export_all_tabs));
}

#[gpui_kit::test]
fn the_info_bar_export_button_opens_the_dialog(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_filtered(cx, &["error a"], "error");
    common::click(cx, w, "export");
    assert!(common::exists(cx, w, "export-lines"));
    assert!(ws.read_with(cx, |ws, _| ws.toasts.is_empty()), "no placeholder toast");
}

#[gpui_kit::test]
fn sessions_export_to_rogg_files(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_filtered(cx, &["error a"], "error");
    let other = common::write_log(dir.path(), "b.log", &["b"]);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![other], window, cx))).unwrap();
    let one = dir.path().join("one.rogg");
    ws.update(cx, |ws, cx| ws.export_session_to(one.clone(), cx));
    common::wait_until(cx, |cx| toasted(&ws, cx, "Saved session to one.rogg"));
    assert_eq!(Session::read_rogg(&one).unwrap().tabs.len(), 1, "this tab only");
    let all = dir.path().join("all.rogg");
    ws.update(cx, |ws, cx| {
        ws.export_all_tabs = true;
        ws.export_session_to(all.clone(), cx);
    });
    common::wait_until(cx, |cx| toasted(&ws, cx, "Saved session to all.rogg"));
    assert_eq!(Session::read_rogg(&all).unwrap().tabs.len(), 2);
}

#[gpui_kit::test]
fn a_session_with_nothing_restorable_is_not_saved(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("feed", || Ok(std::io::Cursor::new(b"one\ntwo\n".to_vec())), window, cx))).unwrap();
    let dst = dir.path().join("stream.rogg");
    ws.update(cx, |ws, cx| ws.export_session_to(dst.clone(), cx));
    cx.run_until_parked();
    assert!(toasted(&ws, cx, "Nothing to save: streams and in-memory tabs are not saved"));
    assert!(!dst.exists(), "no empty .rogg is written");
}
