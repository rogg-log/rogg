mod common;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Entity, ScrollDelta, TestAppContext, point, px};
use rogg_ui::logview::{LogLine, LogRow, LogView, LogViewEvent, LogViewState, VecRows};
use std::cell::RefCell;
use std::rc::Rc;

fn rows(n: usize, gap_at: Option<usize>) -> Rc<VecRows> {
    let mut v = Vec::new();
    for i in 0..n {
        if Some(i) == gap_at {
            v.push(LogRow::Gap { omitted: 4 });
        }
        v.push(LogRow::Line(LogLine { number: 1000 + i as u64, text: format!("line {i}").into(), spans: vec![], pinned: false, context: false, tint: None }));
    }
    Rc::new(VecRows::new(v))
}

fn open(cx: &mut TestAppContext, state: Entity<LogViewState>) -> gpui_kit::AnyWindowHandle {
    common::open(cx, 800., 238., move |_, _| LogView::new("log-test", state.clone()).into_any_element())
}

fn render(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn rows_start_below_padding_and_gap_is_shorter(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, Some(2))));
    let w = open(cx, state.clone());
    render(cx, w);
    let geo = state.read_with(cx, |s, _| s.last_rows().to_vec());
    let top = common::bounds(cx, w, "log-test").origin.y;
    assert_eq!(geo[0].bounds.origin.y - top, px(4.));
    assert_eq!(geo[0].bounds.size.height, px(23.));
    assert!(geo[2].is_gap);
    assert_eq!(geo[2].bounds.size.height, px(17.));
    assert_eq!(geo[3].bounds.origin.y - top, px(4. + 23. + 23. + 17.));
}

#[gpui_kit::test]
fn click_selects_and_gutter_click_toggles_pin(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, None)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    cx.update(|cx| {
        cx.subscribe(&state, move |_, ev: &LogViewEvent, _| {
            e.borrow_mut().push(match ev {
                LogViewEvent::Selected { number, .. } => format!("select {number}"),
                LogViewEvent::TogglePin { number } => format!("pin {number}"),
            })
        })
        .detach();
    });
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at("log-test", point(px(300.), px(4. + 23. + 10.)), cx);
        window.click_at("log-test", point(px(10.), px(4. + 46. + 10.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(*events.borrow(), vec!["select 1001".to_string(), "pin 1002".to_string()]);
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1001));
}

#[gpui_kit::test]
fn wheel_scrolls_and_center_on_row_centres(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-50.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert_eq!(first, 2);
    state.update(cx, |s, cx| s.center_on_row(500, cx));
    render(cx, w);
    let row = state.read_with(cx, |s, _| s.last_rows().iter().find(|g| g.row == 500).cloned()).expect("row 500 visible");
    let viewport_mid = common::bounds(cx, w, "log-test").center().y;
    assert!((row.bounds.center().y - viewport_mid).abs() <= px(1.), "row 500 at {:?}, mid {viewport_mid:?}", row.bounds);
}

#[gpui_kit::test]
fn wrap_toggle_and_set_rows_reset_safely(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.center_on_row(99, cx));
    render(cx, w);
    state.update(cx, |s, cx| {
        s.set_wrap(true, cx);
        s.set_rows(rows(3, None), cx);
    });
    render(cx, w);
    let geo = state.read_with(cx, |s, _| s.last_rows().to_vec());
    assert_eq!(geo.len(), 3);
    assert_eq!(geo[0].row, 0);
}
