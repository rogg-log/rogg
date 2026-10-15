mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, Focusable, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::Pane;
use rogg_app::workspace::Workspace;

fn open_two(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>) {
    let dir = common::setup(cx);
    let a = common::write_log(dir.path(), "a.log", &["alpha", "error one"]);
    let b = common::write_log(dir.path(), "b.log", &["beta"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![a, b], ..Default::default() });
    for tab in ws.read_with(cx, |ws, _| ws.tabs().to_vec()) {
        common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    }
    (dir, w, ws)
}

fn expanded(ws: &Entity<Workspace>, index: usize, cx: &mut TestAppContext) -> Option<Pane> {
    ws.read_with(cx, |ws, cx| ws.tabs()[index].read(cx).expanded)
}

#[gpui_kit::test]
fn the_expand_button_fills_the_window_with_its_pane_and_restores(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "tool-expand-bottom");
    assert_eq!(expanded(&ws, 1, cx), Some(Pane::Filtered));
    for gone in ["sidebar", "meta", "filter-band", "pane-top", "statusbar"] {
        assert!(!common::exists(cx, w, gone), "{gone} should be hidden");
    }
    assert!(common::exists(cx, w, "pane-bottom"));
    common::click(cx, w, "tool-expand-bottom");
    assert_eq!(expanded(&ws, 1, cx), None, "the same button restores");
    assert!(common::exists(cx, w, "pane-top") && common::exists(cx, w, "filter-band") && common::exists(cx, w, "statusbar"));
}

#[gpui_kit::test]
fn escape_restores_an_expanded_pane(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "tool-expand-top");
    assert_eq!(expanded(&ws, 1, cx), Some(Pane::Full));
    common::press(cx, w, "escape");
    assert_eq!(expanded(&ws, 1, cx), None);
    assert!(common::exists(cx, w, "pane-bottom"));
}

#[gpui_kit::test]
fn each_tab_keeps_its_own_expanded_pane(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "tool-expand-top");
    common::click(cx, w, "tab-0");
    assert_eq!((expanded(&ws, 0, cx), expanded(&ws, 1, cx)), (None, Some(Pane::Full)));
    assert!(common::exists(cx, w, "filter-band"), "tab a shows the normal layout");
    common::click(cx, w, "tab-1");
    assert!(!common::exists(cx, w, "filter-band"), "tab b is still expanded");
}

#[gpui_kit::test]
fn an_expanded_pane_is_not_restored_with_the_session(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "tool-expand-top");
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    assert_eq!(ws2.read_with(cx, |ws, _| ws.tabs().len()), 2);
    assert_eq!(expanded(&ws2, 1, cx), None);
}

#[gpui_kit::test]
fn edit_filters_restores_the_panes_so_the_editor_shows(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    common::click(cx, w, "tool-expand-top");
    common::press(cx, w, "cmd-l");
    assert_eq!(expanded(&ws, 1, cx), None);
    assert!(common::exists(cx, w, "expression"), "the filter editor is visible");
}

#[gpui_kit::test]
fn find_and_tab_stay_in_the_expanded_pane(cx: &mut TestAppContext) {
    let (_dir, w, ws) = open_two(cx);
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[1].clone());
    tab.update(cx, |t, _| t.last_pane = Pane::Full);
    common::click(cx, w, "tool-expand-bottom");
    common::press(cx, w, "cmd-f");
    let find = tab.read_with(cx, |t, _| t.find[Pane::Filtered.index()].clone());
    let find_focused = cx.update_window(w, |_, window, cx| find.focus_handle(cx).is_focused(window)).unwrap();
    assert!(find_focused, "⌘F goes to the expanded pane's find field");
    for _ in 0..3 {
        common::press(cx, w, "tab");
        let (top, band) = cx.update_window(w, |_, window, cx| {
            let t = tab.read(cx);
            (t.focus[0].contains_focused(window, cx), t.focus[1].contains_focused(window, cx))
        })
        .unwrap();
        assert!(!top && !band, "Tab never moves focus into a hidden part");
    }
}
