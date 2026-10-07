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
                LogViewEvent::AddTerm { text, exclude } => format!("term {text} {exclude}"),
                LogViewEvent::TailLeft => "tail-left".to_string(),
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

fn rows_without(skip: u64) -> Rc<VecRows> {
    Rc::new(VecRows::new(
        (1000..1100u64)
            .filter(|n| *n != skip)
            .map(|n| LogRow::Line(LogLine { number: n, text: format!("line {n}").into(), spans: vec![], pinned: false, context: false, tint: None }))
            .collect(),
    ))
}

#[gpui_kit::test]
fn arrows_continue_from_where_a_vanished_selection_was(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    state.update(cx, |s, cx| {
        s.set_selected(Some(1005), cx);
        s.set_rows(rows_without(1005), cx);
        s.move_selection(1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1006), "down selects the row that took its place");
    state.update(cx, |s, cx| {
        s.set_selected(Some(1005), cx);
        s.move_selection(-1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1004), "up selects the row before it");
    state.update(cx, |s, cx| {
        s.set_selected(Some(5000), cx);
        s.move_selection(1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1099), "a selection past the end stays at the end going down");
    state.update(cx, |s, cx| {
        s.set_selected(Some(5000), cx);
        s.move_selection(-1, false, cx);
    });
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1099), "and the last row going up");
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
fn top_number_is_the_first_line_in_view(cx: &mut TestAppContext) {
    common::setup(cx);
    let empty = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(Vec::new()))));
    assert_eq!(empty.read_with(cx, |s, _| s.top_number()), None);
    // Row 2 is a gap; the line after it is 1002.
    let state = cx.new(|_| LogViewState::new(rows(1000, Some(2))));
    let w = open(cx, state.clone());
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.top_number()), Some(1000));
    state.update(cx, |s, cx| s.scroll_to_row(2, ScrollTo::Top, cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.top_number()), Some(1002), "a gap row at the top is skipped");
    state.update(cx, |s, cx| s.scroll_to_row(501, ScrollTo::Top, cx));
    assert_eq!(state.read_with(cx, |s, _| s.top_number()), Some(1500), "a requested top counts before the next layout");
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| s.top_number()), Some(1500));
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

#[gpui_kit::test]
fn dragging_over_rows_extends_the_selection(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(300.), px(4. + 23. + 10.)), point(px(300.), px(4. + 23. * 4. + 10.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1001, 1004)));
}

#[gpui_kit::test]
fn dragging_the_scrollbar_thumb_scrolls(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    // The thumb sits at the right edge (x 792..798), 24 px tall at the top of the 238 px track.
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(795.), px(10.)), point(px(795.), px(117.)), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert!((400..600).contains(&first), "half-way thumb shows the middle rows, got {first}");
    assert_eq!(state.read_with(cx, |s, _| s.selected()), None, "grabbing the thumb does not select");
}

#[gpui_kit::test]
fn dragging_the_horizontal_thumb_scrolls_long_lines(cx: &mut TestAppContext) {
    common::setup(cx);
    let long = LogRow::Line(LogLine { number: 1, text: "x".repeat(500).into(), spans: vec![], pinned: false, context: false, tint: None });
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(vec![long]))));
    let w = open(cx, state.clone());
    // The horizontal thumb starts at the text column (x = 89) and sits 2 px above the bottom (y 230..236).
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.drag(point(px(99.), px(233.)), point(px(299.), px(233.)), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(state.read_with(cx, |s, _| s.scroll_x()) > px(0.));
}

fn press_left(window: &mut gpui_kit::Window, at: gpui_kit::Point<gpui_kit::Pixels>, shift: bool, cx: &mut gpui_kit::App) {
    use gpui_kit::{InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent};
    let modifiers = Modifiers { shift, ..Default::default() };
    window.dispatch_event(MouseDownEvent { button: MouseButton::Left, position: at, modifiers, click_count: 1, first_mouse: false }.to_platform_input(), cx);
    window.dispatch_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers, click_count: 1 }.to_platform_input(), cx);
}

#[gpui_kit::test]
fn shift_click_extends_from_the_selected_line(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(50, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        press_left(window, point(px(300.), px(4. + 23. + 10.)), false, cx);
        window.render_frame(cx);
        press_left(window, point(px(300.), px(4. + 23. * 3. + 10.)), true, cx);
        window.render_frame(cx);
        // The press is released: a later move with a stale pressed button must not keep extending.
        let stale = gpui_kit::MouseMoveEvent { position: point(px(300.), px(4. + 23. * 6. + 10.)), pressed_button: Some(gpui_kit::MouseButton::Left), modifiers: Default::default() };
        window.dispatch_event(gpui_kit::InputEvent::to_platform_input(stale), cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selection_range()), Some((1001, 1003)));
}

#[gpui_kit::test]
fn clicking_the_scrollbar_track_centres_the_thumb_on_the_pointer(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(1000, None)));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        press_left(window, point(px(795.), px(200.)), false, cx);
        window.render_frame(cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert!(first > 600, "a click low in the track jumps far down, got {first}");
    assert_eq!(state.read_with(cx, |s, _| s.selected()), None);
}

#[gpui_kit::test]
fn dragging_below_the_panel_scrolls_and_extends_past_the_visible_rows(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    render(cx, w);
    let last_visible = state.read_with(cx, |s, _| s.last_rows().last().unwrap().row);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(300.), px(4. + 10.)), point(px(300.), px(600.)), cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    assert!(first > 0, "pointer below the panel auto-scrolls");
    let (lo, hi) = state.read_with(cx, |s, _| s.selection_range()).unwrap();
    assert_eq!(lo, 1000);
    assert!(hi > 1000 + last_visible as u64, "range reaches past the rows visible before the drag: {hi}");
}

#[gpui_kit::test]
fn dragging_above_the_panel_extends_up_past_the_visible_rows(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.scroll_to_row(50, ScrollTo::Top, cx));
    render(cx, w);
    let first_before = state.read_with(cx, |s, _| s.last_rows()[0].row);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.drag(point(px(300.), px(4. + 23. * 3. + 10.)), point(px(300.), px(-100.)), cx);
    })
    .unwrap();
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    let (lo, hi) = state.read_with(cx, |s, _| s.selection_range()).unwrap();
    assert_eq!(hi, 1000 + first_before as u64 + 3);
    assert!(first < first_before, "pointer above the panel auto-scrolls up");
    assert!(lo < 1000 + first_before as u64, "range reaches above the rows visible before the drag: {lo}");
}

#[gpui_kit::test]
fn the_bottom_right_corner_outside_the_horizontal_track_selects_rows(cx: &mut TestAppContext) {
    common::setup(cx);
    // Ten long lines fill the panel exactly: a horizontal thumb but no vertical one.
    let lines: Vec<LogRow> = (0..10)
        .map(|i| LogRow::Line(LogLine { number: 1000 + i, text: "x".repeat(500).into(), spans: vec![], pinned: false, context: false, tint: None }))
        .collect();
    let state = cx.new(|_| LogViewState::new(Rc::new(VecRows::new(lines))));
    let w = open(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        // Row 9 spans y 211..234; the thumb zone starts at y 227, but x 795 is right of the track (ends at 788).
        window.click_at("log-test", point(px(795.), px(229.)), cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1009));
    assert_eq!(state.read_with(cx, |s, _| s.scroll_x()), px(0.));
}

use gpui_kit::{FocusHandle, KeyBinding};
use rogg_ui::logview::actions::{self, CopySelection, CopySelectionWithNumbers, SelectLast, SelectNext};

fn open_focusable(cx: &mut TestAppContext, state: Entity<LogViewState>) -> (gpui_kit::AnyWindowHandle, FocusHandle) {
    let focus = cx.update(|cx| cx.focus_handle());
    let f = focus.clone();
    let w = common::open(cx, 800., 238., move |_, _| LogView::new("log-test", state.clone()).focus(f.clone()).into_any_element());
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("down", SelectNext, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("cmd-down", SelectLast, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("cmd-c", CopySelection, Some(actions::KEY_CONTEXT)),
            KeyBinding::new("alt-cmd-c", CopySelectionWithNumbers, Some(actions::KEY_CONTEXT)),
        ])
    });
    (w, focus)
}

#[gpui_kit::test]
fn clicking_focuses_and_keys_move_and_copy(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let (w, focus) = open_focusable(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.click_at("log-test", point(px(300.), px(4. + 23. + 10.)), cx);
        assert!(focus.is_focused(window), "a click focuses the panel");
        window.press("down", cx);
        window.press("cmd-c", cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1002));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("line 2\n"));
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.press("cmd-down", cx);
        window.press("alt-cmd-c", cx);
    })
    .unwrap();
    assert_eq!(state.read_with(cx, |s, _| s.selected()), Some(1019));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("1019\tline 19\n"));
}

#[gpui_kit::test]
fn right_click_outside_any_line_clears_the_context_target(cx: &mut TestAppContext) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent};
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(3, None)));
    let (w, focus) = open_focusable(cx, state.clone());
    let right_click_at = |cx: &mut TestAppContext, position| {
        cx.update_window(w, |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            window.dispatch_event(MouseMoveEvent { position, pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
            window.render_frame(cx);
            let down = MouseDownEvent { button: MouseButton::Right, position, modifiers: Default::default(), click_count: 1, first_mouse: false };
            window.dispatch_event(down.to_platform_input(), cx);
        })
        .unwrap();
    };
    right_click_at(cx, point(px(300.), px(4. + 23. + 10.)));
    assert_eq!(state.read_with(cx, |s, _| s.context_target().map(|t| t.number)), Some(1001));
    // Below the last of 3 rows of a 238 px panel: no line, so no stale target.
    cx.update_window(w, |_, window, cx| window.blur(cx)).unwrap();
    right_click_at(cx, point(px(300.), px(200.)));
    assert_eq!(state.read_with(cx, |s, _| s.context_target().cloned()), None);
    cx.update_window(w, |_, window, _| assert!(focus.is_focused(window), "an empty-area right-click still focuses the panel")).unwrap();
}

#[gpui_kit::test]
fn right_click_outside_the_panel_leaves_focus_and_target_alone(cx: &mut TestAppContext) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, div};
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let focus = cx.update(|cx| cx.focus_handle());
    let (s, f) = (state.clone(), focus.clone());
    // A 120 px log panel above a sibling that fills the rest of the 238 px window.
    let w = common::open(cx, 800., 238., move |_, _| {
        div()
            .size_full()
            .child(div().h(px(120.)).child(LogView::new("log-test", s.clone()).focus(f.clone())))
            .child(div().id("sibling").h(px(118.)))
            .into_any_element()
    });
    let right_click_at = |cx: &mut TestAppContext, position| {
        cx.update_window(w, |_, window, cx| {
            use gpui_kit::test::TestWindowExt;
            window.render_frame(cx);
            window.dispatch_event(MouseMoveEvent { position, pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
            window.render_frame(cx);
            let down = MouseDownEvent { button: MouseButton::Right, position, modifiers: Default::default(), click_count: 1, first_mouse: false };
            window.dispatch_event(down.to_platform_input(), cx);
        })
        .unwrap();
    };
    right_click_at(cx, point(px(300.), px(4. + 23. + 10.)));
    let target = state.read_with(cx, |s, _| s.context_target().cloned());
    assert_eq!(target.as_ref().map(|t| t.number), Some(1001));
    // Close the menu the right-click opened, focus something else (as the rest of the app would), then
    // right-click the sibling.
    let other = cx.update(|cx| cx.focus_handle());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.press("escape", cx);
        other.focus(window, cx);
        assert!(other.is_focused(window) && !focus.is_focused(window));
    })
    .unwrap();
    right_click_at(cx, point(px(300.), px(200.)));
    assert_eq!(state.read_with(cx, |s, _| s.context_target().cloned()), target, "the target is untouched");
    cx.update_window(w, |_, window, _| {
        assert!(other.is_focused(window), "focus stays where it was");
        assert!(!focus.is_focused(window), "the panel did not steal focus");
    })
    .unwrap();
}

#[gpui_kit::test]
fn right_click_selects_and_records_the_context_target(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let (w, _) = open_focusable(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.right_click("log-test", cx); // the centre of a 238 px panel is row 5
    })
    .unwrap();
    cx.run_until_parked();
    state.read_with(cx, |s, _| {
        assert_eq!(s.selected(), Some(1005));
        let target = s.context_target().expect("target recorded");
        assert_eq!((target.number, target.pinned), (1005, false));
    });
}

fn tail_left_count(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> Rc<RefCell<usize>> {
    let count = Rc::new(RefCell::new(0));
    let c = count.clone();
    cx.update(|cx| {
        cx.subscribe(state, move |_, ev: &LogViewEvent, _| {
            if *ev == LogViewEvent::TailLeft {
                *c.borrow_mut() += 1;
            }
        })
        .detach();
    });
    count
}

fn last_visible(cx: &mut TestAppContext, state: &Entity<LogViewState>) -> usize {
    state.read_with(cx, |s, _| s.last_rows().last().map(|g| g.row).unwrap())
}

#[gpui_kit::test]
fn tail_mode_follows_appended_rows_until_the_user_scrolls_up(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let left = tail_left_count(cx, &state);
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.set_tail(true, cx));
    render(cx, w);
    assert_eq!(last_visible(cx, &state), 99);
    assert!(state.read_with(cx, |s, _| s.tail() && s.at_bottom()));
    state.update(cx, |s, cx| s.extend_rows(rows(150, None), cx));
    render(cx, w);
    assert_eq!(last_visible(cx, &state), 149, "appended rows scroll into view");
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-200.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(state.read_with(cx, |s, _| s.tail()), "scrolling down at the end stays in tail mode");
    assert_eq!(*left.borrow(), 0);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(100.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(!state.read_with(cx, |s, _| s.tail() || s.at_bottom()));
    assert_eq!(*left.borrow(), 1);
    state.update(cx, |s, cx| s.extend_rows(rows(200, None), cx));
    render(cx, w);
    assert!(last_visible(cx, &state) < 199, "once left, appended rows no longer pull the view");
}

#[gpui_kit::test]
fn jumping_to_an_earlier_row_leaves_tail_mode(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let left = tail_left_count(cx, &state);
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| {
        s.set_tail(true, cx);
        s.scroll_to_row(99, ScrollTo::Visible, cx);
    });
    render(cx, w);
    assert!(state.read_with(cx, |s, _| s.tail()), "the last row keeps tail mode");
    state.update(cx, |s, cx| s.scroll_to_row(10, ScrollTo::Center, cx));
    render(cx, w);
    assert!(!state.read_with(cx, |s, _| s.tail()));
    assert_eq!(*left.borrow(), 1);
}

#[gpui_kit::test]
fn extend_rows_keeps_the_position_and_selection(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| {
        s.set_selected(Some(1040), cx);
        s.scroll_to_row(40, ScrollTo::Top, cx);
    });
    render(cx, w);
    let first = state.read_with(cx, |s, _| s.last_rows()[0].row);
    state.update(cx, |s, cx| s.extend_rows(rows(300, None), cx));
    render(cx, w);
    assert_eq!(state.read_with(cx, |s, _| (s.last_rows()[0].row, s.selected())), (first, Some(1040)));
}

#[gpui_kit::test]
fn a_wheel_event_right_after_extend_rows_keeps_tail_mode(cx: &mut TestAppContext) {
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{InputEvent, ScrollWheelEvent};
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(100, None)));
    let left = tail_left_count(cx, &state);
    let w = open(cx, state.clone());
    state.update(cx, |s, cx| s.set_tail(true, cx));
    render(cx, w);
    // Parks the pointer over the log (this wheel at the end changes nothing).
    cx.update_window(w, |_, window, cx| window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-10.))), cx)).unwrap();
    assert!(state.read_with(cx, |s, _| s.tail()));
    let at = common::bounds(cx, w, "log-test").center();
    // The wheel arrives before the next layout, as with a poll that appended rows between two frames (no redraw
    // happens inside one `update_window`).
    cx.update_window(w, |_, window, cx| {
        state.update(cx, |s, cx| s.extend_rows(rows(300, None), cx));
        let wheel = ScrollWheelEvent { position: at, delta: ScrollDelta::Pixels(point(px(0.), px(-50.))), ..Default::default() };
        window.dispatch_event(wheel.to_platform_input(), cx);
    })
    .unwrap();
    assert_eq!(*left.borrow(), 0, "no false TailLeft from a stale anchor");
    assert!(state.read_with(cx, |s, _| s.tail()));
    render(cx, w);
    assert_eq!(last_visible(cx, &state), 299);
}

fn wide_rows(n: usize) -> Rc<VecRows> {
    let v = (0..n)
        .map(|i| LogRow::Line(LogLine { number: 1000 + i as u64, text: "x".repeat(500).into(), spans: vec![], pinned: false, context: false, tint: None }))
        .collect();
    Rc::new(VecRows::new(v))
}

fn move_mouse(cx: &mut TestAppContext, w: gpui_kit::AnyWindowHandle, to: gpui_kit::Point<gpui_kit::Pixels>) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.simulate_mouse_move(to, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_horizontal_bar_shows_only_on_its_strip_or_after_a_horizontal_scroll(cx: &mut TestAppContext) {
    use rogg_ui::logview::{HBAR_HOVER_ZONE, HBAR_LINGER};
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(wide_rows(50)));
    let w = open(cx, state.clone());
    render(cx, w);
    let visible = |cx: &mut TestAppContext| state.read_with(cx, |s, _| s.hbar_visible());
    assert!(!visible(cx), "hidden by default");
    let b = common::bounds(cx, w, "log-test");
    move_mouse(cx, w, point(b.left() + px(300.), b.bottom() - px(HBAR_HOVER_ZONE - 1.)));
    assert!(visible(cx), "shown while the pointer is on the bar's strip");
    move_mouse(cx, w, point(b.left() + px(300.), b.bottom() - px(HBAR_HOVER_ZONE + 2.)));
    assert!(!visible(cx), "the bottom row above the strip does not show it");
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(0.), px(-50.))), cx);
    })
    .unwrap();
    assert!(!visible(cx), "a vertical scroll does not show it");
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-test", ScrollDelta::Pixels(point(px(-100.), px(0.))), cx);
    })
    .unwrap();
    assert!(visible(cx), "shown right after a horizontal scroll");
    cx.executor().advance_clock(HBAR_LINGER / 2);
    cx.run_until_parked();
    assert!(visible(cx), "still shown within the linger time");
    cx.executor().advance_clock(HBAR_LINGER);
    cx.run_until_parked();
    assert!(!visible(cx), "hidden again after the linger time");
}

#[gpui_kit::test]
fn the_focus_line_is_on_the_panel_perimeter(cx: &mut TestAppContext) {
    common::setup(cx);
    let state = cx.new(|_| LogViewState::new(rows(20, None)));
    let (w, _focus) = open_focusable(cx, state.clone());
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.click_at("log-test", point(px(300.), px(4. + 23. + 10.)), cx);
    })
    .unwrap();
    assert_eq!(common::bounds(cx, w, "log-focus"), common::bounds(cx, w, "log-test"), "drawn on the panel's edge, not inset");
}
