//! Log view frames under the real macOS text system (CoreText). `TestAppContext` uses a no-op text
//! system, which hides shaping failures (char-boundary panics) and glyph-scan costs.
//! `harness = false`: the macOS platform (source of the text system) must be created on the main thread.
#![cfg_attr(not(target_os = "macos"), allow(dead_code, unused_imports))]
use gpui_kit::prelude::*;
use gpui_kit::{AnyWindowHandle, AppContext, Bounds, HeadlessAppContext, Point, WindowBounds, WindowOptions, px, size};
use rogg_ui::logview::{LogLine, LogRow, LogView, LogViewState, Span, SpanStyle, VecRows};
use rogg_ui::theme::{Appearance, Density, RoggTheme};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn headless() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_asset_source(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(rogg_ui::assets::Assets),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        RoggTheme::new(Appearance::Dark, Density::Compact).install(cx);
    });
    cx
}

struct Host(gpui_kit::Entity<LogViewState>);

impl Render for Host {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        LogView::new("log-real", self.0.clone())
    }
}

fn open(cx: &mut HeadlessAppContext, lines: Vec<LogLine>, wrap: bool) -> AnyWindowHandle {
    open_state(cx, lines, wrap).0
}

fn open_state(cx: &mut HeadlessAppContext, lines: Vec<LogLine>, wrap: bool) -> (AnyWindowHandle, gpui_kit::Entity<LogViewState>) {
    let rows = Rc::new(VecRows::new(lines.into_iter().map(LogRow::Line).collect()));
    cx.update(|cx| {
        let state = cx.new(|_| LogViewState::new(rows));
        let shared = state.clone();
        if wrap {
            state.update(cx, |s, cx| s.set_wrap(true, cx));
        }
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(900.), px(260.)) })),
            focus: false,
            show: false,
            ..Default::default()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, move |_, cx| cx.new(|_| Host(state))).expect("open window");
        (handle, shared)
    })
}

fn render(cx: &mut HeadlessAppContext, w: AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
}

fn line(number: u64, text: impl Into<String>, spans: Vec<Span>) -> LogLine {
    LogLine { number, text: text.into().into(), spans, pinned: false, context: false, tint: None }
}

fn misaligned_lines() -> Vec<LogLine> {
    // "é" = 2 bytes, "日" = 3 bytes, "🙂" = 4 bytes.
    let hit = |r: std::ops::Range<usize>| Span { range: r, style: SpanStyle::Hit(0) };
    let search = |r: std::ops::Range<usize>| Span { range: r, style: SpanStyle::SearchHit };
    vec![
        // start inside 'é'
        line(1, "é timeout", vec![hit(1..5)]),
        // end inside 'é'
        line(2, "timeout é", vec![hit(0..9)]),
        // both ends inside, abutting search hit
        line(3, "日本語 error", vec![hit(1..4), search(4..8)]),
        // inside one char, and a color span past the end
        line(4, "a🙂b🙂c", vec![hit(2..3), Span { range: 3..99, style: SpanStyle::Error }]),
        // within one char, and past the end
        line(5, "🙂", vec![search(1..2), hit(3..400)]),
        // across the 64 KB display cut
        line(6, format!("{}é", "x".repeat(70 * 1024)), vec![hit(64 * 1024 - 1..70 * 1024 + 1)]),
    ]
}

fn spans_inside_multibyte_chars_render_without_panicking(cx: &mut HeadlessAppContext) {
    for wrap in [false, true] {
        let w = open(cx, misaligned_lines(), wrap);
        render(cx, w);
    }
}

/// A 64 KB line with ~13k hits and ~6.5k search hits: drawing a frame must not scan the line per mark.
/// Debug build: the old per-mark glyph scan took ~3 s here, the indexed geometry ~40 ms.
fn dense_hit_line_renders_a_frame_quickly(cx: &mut HeadlessAppContext) {
    const BOUND: Duration = Duration::from_millis(100);
    let text = "abcd ".repeat(64 * 1024 / 5);
    let mut spans: Vec<Span> = (0..text.len() / 5).map(|i| Span { range: i * 5..i * 5 + 4, style: SpanStyle::Hit(i % 4) }).collect();
    spans.extend((0..text.len() / 10).map(|i| Span { range: i * 10 + 1..i * 10 + 3, style: SpanStyle::SearchHit }));
    for wrap in [false, true] {
        let w = open(cx, vec![line(1, text.clone(), spans.clone())], wrap);
        render(cx, w); // warm shaping caches
        let start = Instant::now();
        render(cx, w);
        let took = start.elapsed();
        eprintln!("dense hit frame (wrap={wrap}): {took:?}");
        assert!(took < BOUND, "frame took {took:?} (wrap={wrap}), bound {BOUND:?}");
    }
}

/// Right-click at `x` over the first row: the word under the pointer, from the real glyph metrics.
fn right_click_token(cx: &mut HeadlessAppContext, text: &str, wrap: bool, x: f32) -> Option<String> {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, point};
    let (w, state) = open_state(cx, vec![line(1, text, vec![])], wrap);
    render(cx, w);
    let position = point(px(x), px(4. + 11.));
    cx.update_window(w, |_, window, cx| {
        window.dispatch_event(MouseMoveEvent { position, pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        let down = MouseDownEvent { button: MouseButton::Right, position, modifiers: Default::default(), click_count: 1, first_mouse: false };
        window.dispatch_event(down.to_platform_input(), cx);
    })
    .unwrap();
    cx.update(|cx| state.read(cx).context_target().expect("target recorded").token.clone())
}

/// The context menu's word comes from the glyph under the pointer, wrapped or not.
fn right_click_takes_the_word_under_the_pointer(cx: &mut HeadlessAppContext) {
    use rogg_ui::theme::size::{GUTTER_NUMBER_W, GUTTER_PIN_W};
    let left = GUTTER_PIN_W + GUTTER_NUMBER_W;
    let text = format!("alpha{}omega", " ".repeat(40));
    for wrap in [false, true] {
        assert_eq!(right_click_token(cx, &text, wrap, left + 3.).as_deref(), Some("alpha"), "wrap={wrap}, first glyph");
        assert_eq!(right_click_token(cx, &text, wrap, left + 150.), None, "wrap={wrap}, inside the gap");
        assert_eq!(right_click_token(cx, &text, wrap, GUTTER_PIN_W + 5.), None, "wrap={wrap}, number gutter");
        assert_eq!(right_click_token(cx, &text, wrap, 4.), None, "wrap={wrap}, pin gutter");
    }
}

/// Spec B1: a long wrapped row is cut to 3 visual lines and marked clipped; a short wrapped row keeps its height.
fn long_wrapped_rows_show_three_lines(cx: &mut HeadlessAppContext) {
    use rogg_ui::theme::ActiveRoggTheme;
    let (w, state) = open_state(cx, vec![line(1, "word ".repeat(400), vec![]), line(2, "two words", vec![])], true);
    render(cx, w);
    render(cx, w);
    let row_h = cx.update(|cx| f32::from(cx.rogg().metrics.row_height));
    let rows = cx.update(|cx| state.read(cx).last_rows().to_vec());
    assert_eq!(f32::from(rows[0].bounds.size.height), row_h * 3., "capped at 3 lines");
    assert!(rows[0].clipped);
    assert_eq!(f32::from(rows[1].bounds.size.height), row_h, "a short row keeps one line");
    assert!(!rows[1].clipped);
    assert_eq!(rows[1].bounds.origin.y, rows[0].bounds.bottom(), "the next row starts right below the capped one");
}

fn press_move_release(cx: &mut HeadlessAppContext, w: AnyWindowHandle, from: Point<gpui_kit::Pixels>, to: Point<gpui_kit::Pixels>) {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.dispatch_event(MouseMoveEvent { position: from, pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        window.dispatch_event(MouseDownEvent { button: MouseButton::Left, position: from, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), cx);
        window.render_frame(cx);
        if to != from {
            window.dispatch_event(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: Default::default() }.to_platform_input(), cx);
            window.render_frame(cx);
        }
        window.dispatch_event(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: Default::default(), click_count: 1 }.to_platform_input(), cx);
        window.render_frame(cx);
    })
    .unwrap();
}

/// Spec B3: a drag inside one row selects characters (wrapped or not); crossing rows selects whole rows; a click clears.
fn dragging_within_a_row_selects_characters(cx: &mut HeadlessAppContext) {
    use gpui_kit::point;
    use rogg_ui::theme::size::{GUTTER_NUMBER_W, GUTTER_PIN_W};
    let left = GUTTER_PIN_W + GUTTER_NUMBER_W;
    let text = "alpha beta gamma";
    for wrap in [false, true] {
        let (w, state) = open_state(cx, vec![line(1, text, vec![]), line(2, "second row", vec![])], wrap);
        render(cx, w);
        let y1 = 4. + 11.;
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 40.), px(y1)));
        let (sel, copied) = cx.update(|cx| (state.read(cx).char_selection(), state.read(cx).selected_text(false)));
        let range = sel.expect("characters selected").range();
        assert!(range.start == 0 && range.end > 0 && range.end < text.len(), "wrap={wrap}: {range:?}");
        assert_eq!(copied.as_deref(), Some(&text[range.clone()]), "wrap={wrap}");
        // Past the end of the text selects to the end.
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(850.), px(y1)));
        assert_eq!(cx.update(|cx| state.read(cx).char_selection().map(|c| c.range())), Some(0..text.len()), "wrap={wrap}");
        // Into the next row: whole rows.
        let row2_top = cx.update(|cx| state.read(cx).last_rows()[1].bounds.origin.y);
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 20.), row2_top + px(5.)));
        let (sel, range) = cx.update(|cx| (state.read(cx).char_selection(), state.read(cx).selection_range()));
        assert_eq!((sel, range), (None, Some((1, 2))), "wrap={wrap}");
        // A click clears any character selection.
        press_move_release(cx, w, point(px(left + 1.), px(y1)), point(px(left + 40.), px(y1)));
        press_move_release(cx, w, point(px(left + 20.), px(y1)), point(px(left + 20.), px(y1)));
        assert_eq!(cx.update(|cx| state.read(cx).char_selection()), None, "wrap={wrap}");
    }
}

/// Spec B3: a click without a drag selects the row; a wiggle of a pixel or two while pressing is still a click, even
/// where it crosses a glyph's midpoint.
fn a_click_with_a_small_wiggle_selects_the_row_not_a_character(cx: &mut HeadlessAppContext) {
    use gpui_kit::point;
    use rogg_ui::theme::size::{GUTTER_NUMBER_W, GUTTER_PIN_W};
    let left = GUTTER_PIN_W + GUTTER_NUMBER_W;
    for wrap in [false, true] {
        let (w, state) = open_state(cx, vec![line(1, "alpha beta gamma", vec![]), line(2, "second row", vec![])], wrap);
        render(cx, w);
        let y1 = 4. + 11.;
        // Presses spread over more than a glyph's width, so some 2 px moves cross a midpoint.
        for step in 0..20 {
            let x = left + 20. + step as f32 * 0.5;
            press_move_release(cx, w, point(px(x), px(y1)), point(px(x + 2.), px(y1 + 1.)));
            let (sel, range) = cx.update(|cx| (state.read(cx).char_selection(), state.read(cx).selection_range()));
            assert_eq!((sel, range), (None, Some((1, 1))), "wrap={wrap}, press at {x}");
        }
    }
}

type Case = (&'static str, fn(&mut HeadlessAppContext));

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("skipped: CoreText harness is macOS-only");
}

#[cfg(target_os = "macos")]
fn main() {
    let mut cx = headless();
    let cases: [Case; 6] = [
        ("spans_inside_multibyte_chars_render_without_panicking", spans_inside_multibyte_chars_render_without_panicking),
        ("dense_hit_line_renders_a_frame_quickly", dense_hit_line_renders_a_frame_quickly),
        ("right_click_takes_the_word_under_the_pointer", right_click_takes_the_word_under_the_pointer),
        ("long_wrapped_rows_show_three_lines", long_wrapped_rows_show_three_lines),
        ("dragging_within_a_row_selects_characters", dragging_within_a_row_selects_characters),
        ("a_click_with_a_small_wiggle_selects_the_row_not_a_character", a_click_with_a_small_wiggle_selects_the_row_not_a_character),
    ];
    let filter = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    for (name, case) in cases {
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        case(&mut cx);
        println!("test {name} ... ok");
    }
}
