mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, Focusable, TestAppContext, point, px};
use rogg_app::actions::{DensityComfortable, ThemeDark, ThemeLight};
use rogg_app::args::Args;
use rogg_app::document::Pane;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density};

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn open_one(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, Entity<Workspace>, Entity<Tab>) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &LINES);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    (dir, w, ws, tab)
}

fn type_text(cx: &mut TestAppContext, w: AnyWindowHandle, text: &str) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.input(text, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn focused(cx: &mut TestAppContext, w: AnyWindowHandle, tab: &Entity<Tab>, i: usize) -> bool {
    cx.update_window(w, |_, window, cx| tab.read(cx).focus[i].is_focused(window)).unwrap()
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
fn cmd_l_edits_and_enter_applies(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    common::press(cx, w, "cmd-l");
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()));
    type_text(cx, w, "error");
    common::press(cx, w, "enter");
    assert!(tab.read_with(cx, |t, _| t.editing.is_none()));
    assert_eq!(tab.read_with(cx, |t, cx| t.doc.read(cx).filter.spec.expression()), "error");
}

#[gpui_kit::test]
fn escape_cancels_editing_and_clears_the_error(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "alt-cmd-r");
    assert!(doc.read_with(cx, |d, _| d.filter.spec.regex));
    common::press(cx, w, "cmd-l");
    type_text(cx, w, " || (");
    common::press(cx, w, "enter");
    assert!(common::exists(cx, w, "filter-error"));
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()));
    common::press(cx, w, "escape");
    assert!(tab.read_with(cx, |t, _| t.editing.is_none()));
    assert!(doc.read_with(cx, |d, _| d.filter.error.is_none()));
    assert!(!common::exists(cx, w, "filter-error"));
    assert!(common::exists(cx, w, "token-0") && !common::exists(cx, w, "token-1"), "the applied chips are back");
}

#[gpui_kit::test]
fn cmd_b_pins_and_return_reveals(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    click_at(cx, w, "log-top", 300., 4. + 23. + 10.); // line 2
    common::press(cx, w, "cmd-b");
    assert!(doc.read_with(cx, |d, _| d.filter.pins.contains(&1)));
    doc.update(cx, |d, cx| d.set_expression("done", cx)).unwrap();
    common::press(cx, w, "shift-cmd-f");
    assert!(!tab.read_with(cx, |t, _| t.follow));
    // filtered rows: line 2 (pinned), gap, line 8
    click_at(cx, w, "log-bottom", 300., 4. + 23. + 17. + 10.);
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(2), "follow is off");
    common::press(cx, w, "enter");
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(8), "Return reveals in the full log");
}

#[gpui_kit::test]
fn tab_cycles_focus_between_the_three_regions(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    // a filter, so the filtered pane shows a log view (not the no-matches state) that can take focus
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 0));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 2));
    common::press(cx, w, "shift-tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "enter");
    assert!(tab.read_with(cx, |t, _| t.editing.is_some()), "Enter on the filter line edits");
}

/// Ruling R7: with no matches the filtered pane shows the empty state, whose focus handle is not attached.
#[gpui_kit::test]
fn tab_skips_the_filtered_log_in_the_no_matches_state(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    // no filter: the filtered pane shows the no-matches state
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 0));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "tab");
    assert!(focused(cx, w, &tab, 0), "the empty filtered pane is skipped");
    common::press(cx, w, "shift-tab");
    assert!(focused(cx, w, &tab, 1));
    common::press(cx, w, "shift-tab");
    assert!(focused(cx, w, &tab, 0), "backwards skips it too");
}

#[gpui_kit::test]
fn go_to_line_selects_and_centres(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open_one(cx);
    common::press(cx, w, "cmd-g");
    assert!(common::exists(cx, w, "dialog"));
    type_text(cx, w, "5");
    common::press(cx, w, "enter");
    assert!(!common::exists(cx, w, "dialog"));
    assert_eq!(tab.read_with(cx, |t, cx| t.full.read(cx).selected()), Some(5));
    common::press(cx, w, "cmd-g");
    type_text(cx, w, "99");
    common::press(cx, w, "enter");
    assert!(common::exists(cx, w, "dialog"), "an unknown line keeps the dialog");
    assert!(ws.read_with(cx, |ws, _| !ws.toasts.is_empty()));
}

#[gpui_kit::test]
fn flag_find_and_sidebar_shortcuts(cx: &mut TestAppContext) {
    let (_dir, w, ws, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::press(cx, w, "alt-cmd-c");
    assert!(doc.read_with(cx, |d, _| d.filter.spec.case_sensitive));
    common::press(cx, w, "cmd-f");
    let find = tab.read_with(cx, |t, _| t.find[Pane::Full.index()].clone());
    assert!(cx.update_window(w, |_, window, cx| find.focus_handle(cx).is_focused(window)).unwrap());
    common::press(cx, w, "escape");
    common::press(cx, w, "cmd-\\");
    assert!(!ws.read_with(cx, |ws, _| ws.sidebar_visible));
}

#[gpui_kit::test]
fn theme_and_density_actions_reinstall_the_theme(cx: &mut TestAppContext) {
    let (_dir, w, _, _) = open_one(cx);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ThemeDark), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().appearance), Appearance::Dark);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ThemeLight), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().appearance), Appearance::Light);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(DensityComfortable), cx)).unwrap();
    assert_eq!(cx.update(|cx| cx.rogg().density), Density::Comfortable);
}

/// Ruling R9: a loading or failed tab renders a placeholder; focusing its unattached handles would leave the
/// workspace shortcuts dead.
#[gpui_kit::test]
fn focus_keys_do_nothing_on_a_placeholder_tab(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args { demo: false, paths: vec![dir.path().join("missing.log")] });
    assert!(common::exists(cx, w, "error"));
    for keys in ["tab", "shift-tab", "cmd-f", "cmd-l"] {
        common::press(cx, w, keys);
        common::render(cx, w);
        cx.run_until_parked();
        common::press(cx, w, "cmd-g");
        assert!(common::exists(cx, w, "dialog"), "{keys} left the workspace shortcuts dead");
        common::press(cx, w, "escape");
        assert!(!common::exists(cx, w, "dialog"), "escape after {keys}");
    }
}

#[gpui_kit::test]
fn escaping_a_dialog_with_an_input_keeps_the_shortcuts_alive(cx: &mut TestAppContext) {
    let (_dir, w, _, _) = open_one(cx);
    for round in 0..3 {
        common::press(cx, w, "cmd-g");
        assert!(common::exists(cx, w, "dialog"), "round {round}: the shortcut worked");
        common::press(cx, w, "escape");
        assert!(!common::exists(cx, w, "dialog"), "round {round}: escape closed it");
    }
}

#[gpui_kit::test]
fn focus_returns_to_the_workspace_when_the_filtered_log_empties(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    for _ in 0..3 {
        common::press(cx, w, "tab");
    }
    assert!(focused(cx, w, &tab, 2));
    doc.update(cx, |d, cx| d.set_expression("no such text", cx)).unwrap();
    common::render(cx, w);
    cx.run_until_parked();
    assert!(!focused(cx, w, &tab, 2), "the unrendered filtered log gave focus back");
    common::press(cx, w, "cmd-g");
    assert!(common::exists(cx, w, "dialog"), "workspace shortcuts still work");
}

#[gpui_kit::test]
fn tab_does_not_move_focus_behind_a_dialog(cx: &mut TestAppContext) {
    let (_dir, w, _, tab) = open_one(cx);
    common::press(cx, w, "cmd-g");
    common::press(cx, w, "tab");
    assert!(common::exists(cx, w, "dialog"));
    for i in 0..3 {
        assert!(!focused(cx, w, &tab, i), "region {i} took focus behind the modal");
    }
    common::press(cx, w, "escape");
    assert!(!common::exists(cx, w, "dialog"));
}
