//! Log view frames under the real macOS text system (CoreText). `TestAppContext` uses a no-op text
//! system, which hides shaping failures (char-boundary panics) and glyph-scan costs.
//! `harness = false`: the macOS platform (source of the text system) must be created on the main thread.
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
    let rows = Rc::new(VecRows::new(lines.into_iter().map(LogRow::Line).collect()));
    cx.update(|cx| {
        let state = cx.new(|_| LogViewState::new(rows));
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
        handle
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

fn main() {
    let mut cx = headless();
    let cases: [(&str, fn(&mut HeadlessAppContext)); 2] = [
        ("spans_inside_multibyte_chars_render_without_panicking", spans_inside_multibyte_chars_render_without_panicking),
        ("dense_hit_line_renders_a_frame_quickly", dense_hit_line_renders_a_frame_quickly),
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
