mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::Source;
use rogg_app::tab::TabInit;
use rogg_app::workspace::Workspace;
use rogg_app::workspace::tab_menu::{TabMenuAction, tab_menu_items};
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, write_archive};
use rogg_ui::theme::ChromeStyle;

fn open_four(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let paths = ["a", "b", "c", "d"].map(|n| common::write_log(dir.path(), &format!("{n}.log"), &[n])).to_vec();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths, ..Default::default() });
    (dir, w, ws)
}

/// Tab titles, and the active tab's title.
fn state(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> (Vec<String>, Option<String>) {
    ws.read_with(cx, |ws, cx| {
        let titles: Vec<String> = ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect();
        let active = titles.get(ws.active_index()).cloned();
        (titles, active)
    })
}

fn run(cx: &mut TestAppContext, w: AnyWindowHandle, ws: &Entity<Workspace>, index: usize, action: TabMenuAction) {
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.run_tab_action(index, action, window, cx))).unwrap();
    cx.run_until_parked();
}

fn activate(cx: &mut TestAppContext, w: AnyWindowHandle, ws: &Entity<Workspace>, index: usize) {
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(index, window, cx))).unwrap();
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[gpui_kit::test]
fn close_to_the_right_keeps_a_surviving_active_tab(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    activate(cx, w, &ws, 0);
    run(cx, w, &ws, 1, TabMenuAction::CloseRight);
    assert_eq!(state(&ws, cx), (names(&["a.log", "b.log"]), Some("a.log".into())));
}

#[gpui_kit::test]
fn closing_the_active_tab_activates_the_right_clicked_one(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    assert_eq!(state(&ws, cx).1.as_deref(), Some("d.log"));
    run(cx, w, &ws, 1, TabMenuAction::CloseRight);
    assert_eq!(state(&ws, cx), (names(&["a.log", "b.log"]), Some("b.log".into())));
}

#[gpui_kit::test]
fn close_to_the_left(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    activate(cx, w, &ws, 0);
    run(cx, w, &ws, 2, TabMenuAction::CloseLeft);
    assert_eq!(state(&ws, cx), (names(&["c.log", "d.log"]), Some("c.log".into())));
}

#[gpui_kit::test]
fn close_others_and_close_all(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    run(cx, w, &ws, 1, TabMenuAction::CloseOthers);
    assert_eq!(state(&ws, cx), (names(&["b.log"]), Some("b.log".into())));
    run(cx, w, &ws, 0, TabMenuAction::CloseAll);
    assert_eq!(state(&ws, cx), (vec![], None));
    assert!(common::exists(cx, w, "welcome"));
}

#[gpui_kit::test]
fn close_tab_closes_the_right_clicked_tab_not_the_active_one(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    run(cx, w, &ws, 1, TabMenuAction::Close);
    assert_eq!(state(&ws, cx), (names(&["a.log", "c.log", "d.log"]), Some("d.log".into())));
}

#[gpui_kit::test]
fn closing_in_bulk_stops_a_stream_capture(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_four(cx);
    let (reader, _writer) = std::io::pipe().unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("stdin", move || Ok(reader), window, cx))).unwrap().expect("a stream tab");
    assert_eq!(ws.read_with(cx, |ws, _| ws.stream_paths().len()), 1);
    run(cx, w, &ws, 0, TabMenuAction::CloseOthers);
    assert!(ws.read_with(cx, |ws, _| ws.stream_paths().is_empty()), "the capture is dropped with its tab");
}

#[gpui_kit::test]
fn copy_path_and_reveal_for_a_file_and_a_member(cx: &mut TestAppContext) {
    let (dir, w, ws) = open_four(cx);
    run(cx, w, &ws, 0, TabMenuAction::CopyPath);
    // Opened files are canonical (`/private/var/…` for a macOS temp folder).
    let root = dir.path().canonicalize().unwrap();
    let a = root.join("a.log");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some(a.display().to_string()));
    run(cx, w, &ws, 0, TabMenuAction::Reveal);
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(a));

    let zip = root.join("bundle.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"one\n")]).unwrap();
    let member = Source::Member(SourceRef::member(SourceRef::File(zip.clone()), "logs/api.log"));
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_source(member, TabInit::default(), window, cx))).unwrap();
    let index = ws.read_with(cx, |ws, _| ws.tabs().len() - 1);
    run(cx, w, &ws, index, TabMenuAction::CopyPath);
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), Some(zip.join("logs/api.log").display().to_string()));
    run(cx, w, &ws, index, TabMenuAction::Reveal);
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(zip), "a member reveals its archive");
}

#[test]
fn menu_items_enable_by_position_and_source() {
    use TabMenuAction::*;
    let enabled = |index, count, has_path| -> Vec<(TabMenuAction, bool)> {
        tab_menu_items(index, count, has_path, ChromeStyle::Mac).into_iter().flatten().map(|i| (i.action, i.enabled)).collect()
    };
    let all = [Close, CloseOthers, CloseLeft, CloseRight, CloseAll, CopyPath, Reveal];
    assert_eq!(enabled(1, 3, true), all.map(|a| (a, true)));
    assert_eq!(enabled(0, 3, true)[2], (CloseLeft, false), "nothing left of the first tab");
    assert_eq!(enabled(2, 3, true)[3], (CloseRight, false), "nothing right of the last tab");
    let single = enabled(0, 1, true);
    assert_eq!([single[1], single[2], single[3]], [(CloseOthers, false), (CloseLeft, false), (CloseRight, false)]);
    let no_file = enabled(1, 3, false);
    assert_eq!([no_file[5], no_file[6]], [(CopyPath, false), (Reveal, false)], "stdin, filtered and demo tabs have no file");

    let items = tab_menu_items(1, 3, true, ChromeStyle::Mac);
    assert_eq!(items.iter().position(Option::is_none), Some(5), "a separator before the path items");
    let label = |chrome| tab_menu_items(0, 1, true, chrome).into_iter().flatten().last().unwrap().label;
    assert_eq!([label(ChromeStyle::Mac), label(ChromeStyle::Linux), label(ChromeStyle::Windows)], ["Show in Finder", "Show in Files", "Show in Explorer"]);
}
