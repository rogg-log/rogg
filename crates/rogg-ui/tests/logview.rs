mod common;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Entity, ScrollDelta, TestAppContext, point, px};
use rogg_ui::logview::{LogLine, LogRow, LogView, LogViewEvent, LogViewState, ScrollTo, VecRows};
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
fn horizontal_scroll_is_reclamped_when_lines_get_shorter(cx: &mut TestAppContext) {
    common::setup(cx);
    let long = LogRow::Line(LogLine { number: 1, text: "x".repeat(500).into(), spans: vec![], pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![long]))));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.scroll("log-test", ScrollDelta::Pixels(point(px(-300.), px(0.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.scroll_x()), px(300.));
    state.update(cx, |s, cx| s.set_rows(rows(3, None), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.scroll_x()), px(0.));
}

#[gpui_kit::test]
fn wrapped_line_with_hits_across_wrap_boundaries_paints(cx: &mut TestAppContext) {
    use rogg_ui::logview::{Span, SpanStyle};
    common::setup(cx);
    let spans = vec![
        Span { range: 0..20, style: SpanStyle::Faint },
        Span { range: 10..250, style: SpanStyle::Hit(1) },
        Span { range: 15..240, style: SpanStyle::SearchHit },
    ];
    let long = LogRow::Line(LogLine { number: 1, text: "word ".repeat(60).into(), spans, pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![long]))));
    state.update(cx, |s, cx| s.set_wrap(true, cx));
    let w = open(cx, state.clone());
    render(cx, w);
    let geo = state.read_with(cx, |s, _| s.last_rows().to_vec());
    let h = f32::from(geo[0].bounds.size.height);
    assert!(h > 23. && h % 23. == 0., "wrapped row spans several visual lines, got {h}");
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

#[gpui_kit::test]
fn horizontal_scroll_stops_at_the_display_cut(cx: &mut TestAppContext) {
    use rogg_ui::logview::text::MAX_DISPLAY_BYTES;
    common::setup(cx);
    // Scrolled fully right, a 1 MB line reaches exactly as far as a line holding the cut text plus its `…`.
    let max_scroll = |cx: &mut TestAppContext, chars: usize| {
        let line = LogRow::Line(LogLine { number: 1, text: "x".repeat(chars).into(), spans: vec![], pinned: false, context: false, tint: None });
        let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![line]))));
        let w = open(cx, state.clone());
        cx.update_window(w, |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            window.scroll("log-test", ScrollDelta::Pixels(point(px(-1e9), px(0.))), cx);
            window.render_frame(cx);
        })
        .unwrap();
        state.read_with(cx, |s, _| s.scroll_x())
    };
    let cut = max_scroll(cx, MAX_DISPLAY_BYTES + 1);
    assert!(cut > px(0.));
    assert_eq!(max_scroll(cx, 1024 * 1024), cut);
}

fn rows_from(start: u64, n: usize) -> Rc<VecRows> {
    Rc::new(VecRows::new(
        (0..n)
            .map(|i| LogRow::Line(LogLine { number: start + i as u64, text: format!("line {i}").into(), spans: vec![], pinned: false, context: false, tint: None }))
            .collect(),
    ))
}

fn record(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> Rc<RefCell<Vec<String>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let e = events.clone();
    cx.update(|cx| {
        cx.subscribe(state, move |_, ev: &LogViewEvent, _| {
            if let LogViewEvent::Selected { number, .. } = ev {
                e.borrow_mut().push(format!("select {number}"))
            }
        })
        .detach();
    });
    events
}

#[gpui_kit::test]
fn scroll_to_row_visible_moves_minimally(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| s.scroll_to_row(5, ScrollTo::Visible, cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 0);
    state.update(cx, |s, cx| s.scroll_to_row(300, ScrollTo::Visible, cx));
    render(cx, w);
    let view = common::bounds(cx, w, "log-test");
    let row = state.read_with(cx, |s, _| s.last_rows().iter().find(|g| g.row == 300).cloned()).expect("row 300 visible");
    assert!((row.bounds.bottom() - view.bottom()).abs() <= px(0.5), "row 300 bottom-aligned: {:?} vs {view:?}", row.bounds);
}

#[gpui_kit::test]
fn keyboard_moves_skip_gaps_page_and_jump_to_edges(cx: &mut TestAppContext) {
    common::setup(cx);
    // rows: 1000, 1001, 1002, gap, 1003, 1004, …, 1099
    let state = cx.new(|_| LogViewState::new(rows(100, Some(3))));
    let events = record(cx, &state);
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| {
        s.set_selected(Some(1002), cx);
        s.move_selection(1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1003), "the gap row is skipped");
    state.update(cx, |s, cx| s.move_selection(-1, false, cx));
    state.update(cx, |s, cx| s.select_edge(true, false, cx));
    state.update(cx, |s, cx| s.select_edge(false, false, cx));
    render(cx, w);
    // viewport 238 px: 11 rows intersect it from the top, so a page moves 10 rows (row 10 holds line 1009)
    state.update(cx, |s, cx| s.page(true, false, cx));
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1009));
    assert_eq!(*events.borrow(), ["select 1003", "select 1002", "select 1099", "select 1000", "select 1009"]);
}

#[gpui_kit::test]
fn set_rows_keeps_the_top_line_in_place(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| s.set_rows(rows_from(900, 1100), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 0, "a list at its top stays at its top");
    state.update(cx, |s, cx| s.scroll_to_row(500, ScrollTo::Top, cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 500); // line 1400
    state.update(cx, |s, cx| s.set_rows(rows(1000, None), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.last_rows()[0].row), 400, "line 1400 stays on top");
}

#[gpui_kit::test]
fn range_selection_copies_lines_with_and_without_numbers(cx: &mut TestAppContext) {
    common::setup(cx);
    // rows: 1000..1004, gap, 1005..1009
    let state = cx.new(|_| LogViewState::new(rows(10, Some(5))));
    state.update(cx, |s, cx| {
        s.set_selected(Some(1003), cx);
        s.extend_selection(1006, cx);
    });
    state.read_with(cx, |s, _| {
        assert_eq!(s.selection_range(), Some((1003, 1006)));
        assert!(s.is_selected(1004) && !s.is_selected(1007));
        assert_eq!(s.selected_text(false).as_deref(), Some("line 3\nline 4\nline 5\nline 6\n"));
        assert_eq!(s.selected_text(true).unwrap().lines().next(), Some("1003\tline 3"));
    });
    state.update(cx, |s, cx| s.extend_selection(1001, cx));
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1001, 1003)), "the anchor stays at 1003");
    state.update(cx, |s, cx| s.set_selected(Some(1008), cx));
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1008, 1008)), "a plain selection clears the range");
}
