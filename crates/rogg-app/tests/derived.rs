mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::Document;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Entity<Tab> {
    ws.read_with(cx, |ws, _| ws.active_tab().unwrap())
}

fn doc(tab: &Entity<Tab>, cx: &mut TestAppContext) -> Entity<Document> {
    tab.read_with(cx, |t, _| t.doc.clone())
}

fn ready(tab: &Entity<Tab>, cx: &mut TestAppContext) {
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx) && t.doc.read(cx).filter.progress.is_none()));
}

fn filter(tab: &Entity<Tab>, expr: &str, cx: &mut TestAppContext) {
    let d = doc(tab, cx);
    d.update(cx, |d, cx| d.set_expression(expr, cx)).unwrap();
    ready(tab, cx);
}

fn numbers(tab: &Entity<Tab>, cx: &mut TestAppContext) -> Vec<u64> {
    tab.read_with(cx, |t, cx| {
        let rows = t.full.read(cx).rows().clone();
        (0..rows.row_count()).filter_map(|i| rows.number_at(i)).collect()
    })
}

fn texts(tab: &Entity<Tab>, cx: &mut TestAppContext) -> Vec<String> {
    tab.read_with(cx, |t, cx| {
        let rows = t.full.read(cx).rows().clone();
        (0..rows.row_count()).filter_map(|i| rows.copy_text(i)).collect()
    })
}

fn open(cx: &mut TestAppContext, lines: &[&str]) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log], ..Default::default() });
    let tab = active(&ws, cx);
    ready(&tab, cx);
    (dir, w, ws, tab)
}

#[gpui_kit::test]
fn open_as_tab_freezes_the_filtered_rows(cx: &mut TestAppContext) {
    let (_dir, w, ws, parent) = open(cx, &["a error", "b", "c error", "d", "e error"]);
    filter(&parent, "error", cx);
    doc(&parent, cx).update(cx, |d, cx| d.toggle_pin(1, cx));
    ready(&parent, cx);
    common::click(cx, w, "tool-open-tab");
    let child = active(&ws, cx);
    assert_ne!(child.entity_id(), parent.entity_id());
    ready(&child, cx);
    assert_eq!(child.read_with(cx, |t, cx| t.title(cx)).as_ref(), "a.log ⟩ filtered");
    assert_eq!(texts(&child, cx), ["a error", "b", "c error", "e error"]);
    assert_eq!(numbers(&child, cx), [1, 2, 3, 4], "an ordinary text, numbered from 1");
    filter(&parent, "zzz", cx);
    assert_eq!(texts(&child, cx), ["a error", "b", "c error", "e error"], "frozen at creation");
    let crumbs = child.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs());
    assert_eq!(crumbs.last().map(|c| c.as_ref()), Some("filtered"));
    let parent_crumb = crumbs.len() - 2;
    let crumb_id: &'static str = Box::leak(format!("crumb-{parent_crumb}").into_boxed_str());
    common::click(cx, w, crumb_id);
    assert_eq!(active(&ws, cx).entity_id(), parent.entity_id(), "a breadcrumb focuses the tab behind it");
}

#[gpui_kit::test]
fn nested_derived_tabs_are_frozen_copies(cx: &mut TestAppContext) {
    let lines = ["0", "1", "x2", "3", "4", "x y5", "6", "7", "x y8", "9"];
    let (_dir, w, ws, root) = open(cx, &lines);
    filter(&root, "x", cx);
    common::press(cx, w, "shift-cmd-t");
    let a = active(&ws, cx);
    let label = doc(&a, cx).read_with(cx, |d, _| match &d.load {
        rogg_app::document::LoadState::Loading { label, .. } => label.to_string(),
        _ => panic!("the copy is built on a background job"),
    });
    assert!(label.starts_with("Building tab ") && label.ends_with('%'), "{label}");
    ready(&a, cx);
    assert_eq!(texts(&a, cx), ["x2", "x y5", "x y8"]);
    assert_eq!(numbers(&a, cx), [1, 2, 3]);
    filter(&a, "y", cx);
    common::press(cx, w, "shift-cmd-t");
    let b = active(&ws, cx);
    ready(&b, cx);
    assert_eq!(texts(&b, cx), ["x y5", "x y8"]);
    assert_eq!(numbers(&b, cx), [1, 2]);
    assert!(b.update(cx, |t, cx| t.go_to_line(2, cx)));
    assert_eq!(b.read_with(cx, |t, _| t.selection()), Some(2));
    assert!(!b.update(cx, |t, cx| t.go_to_line(3, cx)), "the tab has two lines");
    b.update(cx, |t, cx| t.toggle_pin(2, cx));
    assert!(doc(&b, cx).read_with(cx, |d, _| d.filter.pins.contains(&1)));
    filter(&a, "zzz", cx);
    assert_eq!(texts(&b, cx), ["x y5", "x y8"], "a parent's later filter does not touch its child");
    let crumbs = b.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs());
    assert_eq!(&crumbs[crumbs.len() - 2..], ["filtered", "filtered"]);
    let a_crumb: &'static str = Box::leak(format!("crumb-{}", crumbs.len() - 2).into_boxed_str());
    common::click(cx, w, a_crumb);
    assert_eq!(active(&ws, cx).entity_id(), a.entity_id());
}

#[gpui_kit::test]
fn a_live_parent_gives_a_snapshot_and_streams_are_not_restorable(cx: &mut TestAppContext) {
    common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("feed", || Ok(std::io::Cursor::new(b"error one\nok\nerror two\n".to_vec())), window, cx))).unwrap();
    let parent = active(&ws, cx);
    common::wait_until(cx, |cx| parent.read_with(cx, |t, cx| t.doc.read(cx).text().is_some_and(|x| x.line_count() >= 3)));
    filter(&parent, "error", cx);
    common::press(cx, w, "shift-cmd-t");
    let child = active(&ws, cx);
    ready(&child, cx);
    assert!(common::exists(cx, w, "snapshot"));
    let restorable = child.read_with(cx, |t, cx| match &t.doc.read(cx).source {
        rogg_app::document::Source::Derived(d) => d.spec.is_some(),
        _ => panic!("not derived"),
    });
    assert!(!restorable, "a stream parent cannot be rebuilt (spec §6.4: stdin tabs are not restored)");
}

#[gpui_kit::test]
fn open_as_tab_refuses_an_empty_view_and_closing_deletes_the_copy(cx: &mut TestAppContext) {
    let (_dir, w, ws, parent) = open(cx, &["a", "b"]);
    filter(&parent, "zzz", cx);
    common::press(cx, w, "shift-cmd-t");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1);
    assert!(ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains("No lines to open"))));
    filter(&parent, "a", cx);
    common::press(cx, w, "shift-cmd-t");
    let child = active(&ws, cx);
    ready(&child, cx);
    let file = doc(&child, cx).read_with(cx, |d, _| d.derived_file().map(|p| p.to_path_buf())).unwrap();
    assert!(file.exists());
    // The test's handle would keep the tab (and its document) alive past the close.
    drop(child);
    common::press(cx, w, "cmd-w");
    cx.run_until_parked();
    assert!(!file.exists(), "closing a derived tab deletes its copy");
}

#[gpui_kit::test]
fn open_as_tab_waits_for_a_view_build_in_flight(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    // 100 matches: the context-free view is built at once, a ±2 view off the UI thread.
    cx.update(|cx| cx.set_global(rogg_app::document::EngineConfig { view_sync_work: 150, ..Default::default() }));
    let lines: Vec<&str> = (0..1000).map(|i| if i % 10 == 5 { "match here" } else { "filler" }).collect();
    let log = common::write_log(dir.path(), "a.log", &lines);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log], ..Default::default() });
    let parent = active(&ws, cx);
    ready(&parent, cx);
    filter(&parent, "match", cx);
    common::wait_until(cx, |cx| doc(&parent, cx).read_with(cx, |d, _| d.filter_settled()));
    doc(&parent, cx).update(cx, |d, cx| d.set_context(2, cx));
    assert!(doc(&parent, cx).read_with(cx, |d, _| d.filter.progress.is_none() && !d.filter_settled()), "the ±2 view is still being built");
    common::press(cx, w, "shift-cmd-t");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1, "no tab from the stale view");
    assert!(ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, t)| t.contains("Filtering is still running — open as tab when it finishes"))));
}
