mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::titlebar::{TabItem, TitleBar};
use std::cell::RefCell;
use std::rc::Rc;

fn tabs(n: usize) -> Vec<TabItem> {
    (0..n)
        .map(|i| TabItem { label: format!("service-{i}-with-a-long-name.log").into(), tooltip: "bundle.zip".into(), active: i == 0 })
        .collect()
}

#[gpui_kit::test]
fn titlebar_geometry(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(3)).traffic_lights(true).into_any_element());
    let bar = common::bounds(cx, w, "titlebar");
    assert_eq!(bar.size.height, px(42.));
    assert_eq!(common::bounds(cx, w, "titlebar-leading").size.width, px(211.));
    let tab = common::bounds(cx, w, "tab-shell-0");
    assert_eq!(tab.size.height, px(29.));
    assert!(tab.size.width <= px(152.));
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(1440. - 10.) + px(0.5), "10px right padding");
}

#[gpui_kit::test]
fn many_tabs_scroll_instead_of_pushing_actions_out(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| TitleBar::new(tabs(12)).into_any_element());
    let actions = common::bounds(cx, w, "title-actions");
    assert!(actions.right() <= px(900.), "title actions must stay visible, got {actions:?}");
    assert!(common::bounds(cx, w, "tab-shell-11").size.width <= px(152.));
}

#[gpui_kit::test]
fn tab_select_and_close_report_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let (s, c) = (e.clone(), e.clone());
        TitleBar::new(tabs(3))
            .on_select_tab(move |i, _, _| s.borrow_mut().push(format!("select {i}")))
            .on_close_tab(move |i, _, _| c.borrow_mut().push(format!("close {i}")))
            .into_any_element()
    });
    common::click(cx, w, "tab-1");
    common::click(cx, w, "tab-close-2");
    assert_eq!(*events.borrow(), vec!["select 1".to_string(), "close 2".to_string()]);
}

#[gpui_kit::test]
fn middle_click_closes_the_tab(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, PlatformInput};
    common::setup(cx);
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let (s, c) = (e.clone(), e.clone());
        TitleBar::new(tabs(3))
            .on_select_tab(move |i, _, _| s.borrow_mut().push(format!("select {i}")))
            .on_close_tab(move |i, _, _| c.borrow_mut().push(format!("close {i}")))
            .into_any_element()
    });
    let position = common::bounds(cx, w, "tab-shell-1").center();
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers: Modifiers::default() }), cx);
        window.render_frame(cx);
        let down = MouseDownEvent { button: MouseButton::Middle, position, click_count: 1, ..Default::default() };
        window.dispatch_event(PlatformInput::MouseDown(down), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*events.borrow(), vec!["close 1".to_string()]);
}

#[gpui_kit::test]
fn overflowing_tab_strip_stays_left_of_actions(cx: &mut TestAppContext) {
    common::setup(cx);
    // 60 tabs overflow even with zero-size text; the strip must clip/scroll inside its own box.
    let w = common::open(cx, 900., 200., |_, _| TitleBar::new(tabs(60)).into_any_element());
    let strip = common::bounds(cx, w, "tabs");
    let actions = common::bounds(cx, w, "title-actions");
    assert!(strip.right() <= actions.left() + px(0.5), "tab strip {strip:?} overlaps actions {actions:?}");
    assert!(actions.right() <= px(900.), "actions out of window: {actions:?}");
}

#[gpui_kit::test]
fn mac_style_has_traffic_space_and_no_window_controls(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Mac);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).on_toggle_sidebar(|_, _| {}).into_any_element());
    assert!(common::exists(cx, w, "traffic"));
    for id in ["window-minimize", "window-maximize", "window-close", "app-menu"] {
        assert!(!common::exists(cx, w, id), "{id} on Mac");
    }
}

#[gpui_kit::test]
fn windows_style_draws_rectangular_controls_on_the_right(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Windows);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).into_any_element());
    assert!(!common::exists(cx, w, "traffic"));
    let close = common::bounds(cx, w, "window-close");
    let max = common::bounds(cx, w, "window-maximize");
    let min = common::bounds(cx, w, "window-minimize");
    assert_eq!((close.size.width, close.size.height), (px(46.), px(42.)));
    assert!(min.right() <= max.left() + px(0.5) && max.right() <= close.left() + px(0.5), "min | max | close, no gaps");
    assert!(close.right() >= px(1440.) - px(0.5), "flush with the right edge");
    assert!(common::exists(cx, w, "app-menu"));
}

#[gpui_kit::test]
fn linux_style_draws_round_controls_with_gaps(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Linux);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).into_any_element());
    let close = common::bounds(cx, w, "window-close");
    let max = common::bounds(cx, w, "window-maximize");
    assert_eq!((close.size.width, close.size.height), (px(24.), px(24.)));
    assert_eq!(close.left() - max.right(), px(8.));
    assert!(common::exists(cx, w, "app-menu"));
}

#[gpui_kit::test]
fn the_close_control_runs_the_close_hook(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Linux);
    let closed = Rc::new(RefCell::new(0));
    let c = closed.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let c = c.clone();
        TitleBar::new(tabs(2)).on_close_window(move |_, _| *c.borrow_mut() += 1).into_any_element()
    });
    common::click(cx, w, "window-close");
    assert_eq!(*closed.borrow(), 1);
}
