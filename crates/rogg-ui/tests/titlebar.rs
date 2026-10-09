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
    // The test platform's windows are server-decorated; force the controls as a mock-up would.
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).force_window_controls(true).into_any_element());
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
        TitleBar::new(tabs(2)).force_window_controls(true).on_close_window(move |_, _| *c.borrow_mut() += 1).into_any_element()
    });
    common::click(cx, w, "window-close");
    assert_eq!(*closed.borrow(), 1);
}

#[gpui_kit::test]
fn windows_controls_leave_their_clicks_to_the_system(cx: &mut TestAppContext) {
    // gpui-windows runs the native minimise / maximise / close after the element's handlers: a click handler would act twice.
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Windows);
    let closed = Rc::new(RefCell::new(0));
    let c = closed.clone();
    let w = common::open(cx, 1440., 200., move |_, _| {
        let c = c.clone();
        TitleBar::new(tabs(2)).on_close_window(move |_, _| *c.borrow_mut() += 1).into_any_element()
    });
    common::click(cx, w, "window-close");
    assert_eq!(*closed.borrow(), 0);
}

#[gpui_kit::test]
fn linux_style_keeps_the_system_controls_of_a_server_decorated_window(cx: &mut TestAppContext) {
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Linux);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).app_menu(|m, _, _| m).into_any_element());
    for id in ["window-minimize", "window-maximize", "window-close"] {
        assert!(!common::exists(cx, w, id), "{id} drawn for a server-decorated window");
    }
    assert!(common::exists(cx, w, "app-menu"), "the ☰ menu stays");
}

/// Pointer input for the Linux drag-area tests, in order, over the centre of element `id`. The test platform's
/// `start_window_move` and `zoom` are both `unimplemented!()`, so a test that reaches either panics with "not
/// implemented", and one that reaches neither returns normally.
#[derive(Clone, Copy)]
enum Pointer {
    Down(usize),
    Up(usize),
    /// A move with the left button held (`true`) or no button.
    Move(bool),
}

fn linux_drag(cx: &mut TestAppContext, id: &'static str, input: &[Pointer]) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Linux);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).into_any_element());
    let position = common::bounds(cx, w, id).center();
    let input = input.to_vec();
    cx.update_window(w, move |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        // Hover first, so the drag area's hitbox is under the pointer for the events that follow.
        window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers: Modifiers::default() }), cx);
        window.render_frame(cx);
        for event in input {
            let event = match event {
                Pointer::Down(click_count) => PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position, click_count, ..Default::default() }),
                Pointer::Up(click_count) => {
                    PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position, modifiers: Modifiers::default(), click_count })
                }
                Pointer::Move(held) => PlatformInput::MouseMove(MouseMoveEvent {
                    position: position + gpui_kit::point(px(3.), px(0.)),
                    pressed_button: held.then_some(MouseButton::Left),
                    modifiers: Modifiers::default(),
                }),
            };
            window.dispatch_event(event, cx);
            // A frame between events, as on a real desktop: the armed state must outlive the rebuilt title bar.
            window.render_frame(cx);
        }
    })
    .unwrap();
}

/// A press alone does not hand the pointer to the window manager (which would swallow a double-click's second press),
/// and neither does a later move without the button: neither `start_window_move` nor `zoom` is reached.
#[gpui_kit::test]
fn linux_style_press_without_dragging_does_not_move_the_window(cx: &mut TestAppContext) {
    linux_drag(cx, "window-drag", &[Pointer::Down(1), Pointer::Up(1), Pointer::Move(false)]);
}

/// Releasing the button disarms the drag: a move with the button held after the release (as when a press elsewhere
/// is dragged over the title bar) does not move the window.
#[gpui_kit::test]
fn linux_style_release_disarms_the_drag(cx: &mut TestAppContext) {
    linux_drag(cx, "brand", &[Pointer::Down(1), Pointer::Up(1), Pointer::Move(true)]);
}

/// Press, then move with the button held: the move reaches `start_window_move` (observed through the test platform's
/// panic; the press alone does not panic, per the test above).
#[gpui_kit::test]
#[should_panic(expected = "not implemented")]
fn linux_style_press_and_drag_moves_the_window(cx: &mut TestAppContext) {
    linux_drag(cx, "window-drag", &[Pointer::Down(1), Pointer::Move(true)]);
}

/// The same on the brand label, the drag area's other half.
#[gpui_kit::test]
#[should_panic(expected = "not implemented")]
fn linux_style_press_and_drag_on_the_brand_moves_the_window(cx: &mut TestAppContext) {
    linux_drag(cx, "brand", &[Pointer::Down(1), Pointer::Move(true)]);
}

/// A full double-click: the first press and release reach nothing (see above), so the panic comes from the second
/// press, which zooms the window (`zoom_window`; gpui's `titlebar_double_click` acts only on macOS).
#[gpui_kit::test]
#[should_panic(expected = "not implemented")]
fn linux_style_double_click_zooms_the_window(cx: &mut TestAppContext) {
    linux_drag(cx, "window-drag", &[Pointer::Down(1), Pointer::Up(1), Pointer::Down(2)]);
}

#[gpui_kit::test]
fn mac_style_double_click_leaves_zooming_to_the_system(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, MouseDownEvent, PlatformInput};
    common::setup_chrome(cx, rogg_ui::theme::ChromeStyle::Mac);
    let w = common::open(cx, 1440., 200., |_, _| TitleBar::new(tabs(2)).into_any_element());
    let position = common::bounds(cx, w, "window-drag").center();
    cx.update_window(w, |_, window, cx| {
        let down = MouseDownEvent { button: MouseButton::Left, position, click_count: 2, ..Default::default() };
        window.dispatch_event(PlatformInput::MouseDown(down), cx);
    })
    .unwrap();
}
