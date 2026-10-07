use super::model::{LogRow, LogRowSource, Span};
use super::state::{LogViewEvent, LogViewState, RowGeometry};
use super::text::{MAX_DISPLAY_BYTES, Segment, display_text, segments};
use crate::icons::IconName;
use crate::theme::{ActiveRoggTheme, Color, RoggTheme, size};
use crate::logview::RowTint;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, Corners, DispatchPhase, Element, ElementId, Entity, Font, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ScrollWheelEvent,
    LineLayout, ShapedLine, SharedString, Style, TextAlign, TextRun, TransformationMatrix, Window, WrappedLine, fill, font, outline, point, px,
    relative, size as gsize,
};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Range;

pub struct LogViewElement {
    state: Entity<LogViewState>,
}

impl LogViewElement {
    pub fn new(state: Entity<LogViewState>) -> Self {
        Self { state }
    }
}

impl IntoElement for LogViewElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

enum Shaped {
    Line(ShapedLine),
    Wrapped(Vec<WrappedLine>),
}

impl Shaped {
    /// Height of a hit box: the font's ascent plus descent.
    fn box_height(&self) -> Pixels {
        match self {
            Shaped::Line(line) => line.ascent + line.descent,
            Shaped::Wrapped(lines) => lines.first().map(|l| l.ascent() + l.descent()).unwrap_or_default(),
        }
    }

    /// Hit-box geometry for this text, built once per row so each box costs O(log n), not a line scan.
    fn geometry(&self) -> TextGeometry {
        match self {
            Shaped::Line(line) => {
                let only = LineGeometry { base: 0, len: line.len, first_visual: 0, starts: vec![0], start_xs: vec![px(0.)], xs: XIndex::new(line) };
                TextGeometry { lines: vec![only] }
            }
            Shaped::Wrapped(lines) => {
                let mut out = Vec::with_capacity(lines.len());
                let (mut base, mut visual) = (0, 0);
                for l in lines {
                    let layout = &l.unwrapped_layout;
                    let xs = XIndex::new(layout);
                    let starts: Vec<usize> = std::iter::once(0)
                        .chain(l.wrap_boundaries().iter().map(|b| l.runs()[b.run_ix].glyphs[b.glyph_ix].index))
                        .collect();
                    let start_xs = starts.iter().map(|&i| xs.x(i)).collect();
                    let first_visual = visual;
                    visual += starts.len();
                    out.push(LineGeometry { base, len: layout.len, first_visual, starts, start_xs, xs });
                    // shape_text splits on '\n' (one byte) into separate WrappedLines.
                    base += layout.len + 1;
                }
                TextGeometry { lines: out }
            }
        }
    }

    fn paint(&self, origin: Point<Pixels>, row_h: Pixels, window: &mut Window, cx: &mut App) {
        match self {
            Shaped::Line(line) => {
                line.paint(origin, row_h, TextAlign::Left, None, window, cx).ok();
            }
            Shaped::Wrapped(lines) => {
                let mut y = origin.y;
                for l in lines {
                    l.paint(point(origin.x, y), row_h, TextAlign::Left, None, window, cx).ok();
                    y += l.size(row_h).height;
                }
            }
        }
    }
}

/// `LineLayout::x_for_index` as a lookup: the x of the first glyph (in layout order) whose byte index is
/// `>= index`, else the line width. `reach` is the running maximum of glyph indices, so it is sorted
/// and binary-searchable even if glyph indices are not.
struct XIndex {
    reach: Vec<usize>,
    xs: Vec<Pixels>,
    width: Pixels,
    /// Glyph x never decreases in layout order (true for left-to-right text), so x grows with the byte index.
    monotone: bool,
}

impl XIndex {
    fn new(layout: &LineLayout) -> Self {
        let glyphs = layout.runs.iter().flat_map(|r| r.glyphs.iter());
        let (mut reach, mut xs) = (Vec::new(), Vec::new());
        for g in glyphs {
            reach.push(reach.last().map_or(g.index, |&r: &usize| r.max(g.index)));
            xs.push(g.position.x);
        }
        let monotone = xs.windows(2).all(|w| w[0] <= w[1]) && xs.last().is_none_or(|&x| x <= layout.width);
        Self { reach, xs, width: layout.width, monotone }
    }

    fn x(&self, index: usize) -> Pixels {
        let ix = self.reach.partition_point(|&r| r < index);
        self.xs.get(ix).copied().unwrap_or(self.width)
    }
}

/// One shaped (possibly wrapped) line: its byte span in the row text and its visual line starts.
struct LineGeometry {
    base: usize,
    len: usize,
    first_visual: usize,
    starts: Vec<usize>,
    /// x of each visual line start; boxes are relative to it.
    start_xs: Vec<Pixels>,
    xs: XIndex,
}

struct TextGeometry {
    lines: Vec<LineGeometry>,
}

impl TextGeometry {
    /// Text boxes covering byte `range`, one per visual line: (visual line, x start, x end), x relative to the text origin.
    fn boxes(&self, range: &Range<usize>, mut each: impl FnMut((usize, Pixels, Pixels))) {
        for l in &self.lines {
            let local = range.start.saturating_sub(l.base).min(l.len)..range.end.saturating_sub(l.base).min(l.len);
            for (ix, sub) in split_at_lines(local, &l.starts, l.len) {
                let line_x = l.start_xs[ix];
                each((l.first_visual + ix, l.xs.x(sub.start) - line_x, l.xs.x(sub.end) - line_x));
            }
        }
    }

    /// (visual line, x from that line's start) of byte `index`; non-decreasing in `index` for monotone lines.
    fn position(&self, index: usize) -> (f32, f32) {
        let Some(l) = self.lines.get(self.lines.partition_point(|l| l.base <= index).saturating_sub(1)) else {
            return (0., 0.);
        };
        let local = index.saturating_sub(l.base).min(l.len);
        let ix = l.starts.partition_point(|&s| s <= local).saturating_sub(1);
        ((l.first_visual + ix) as f32, f32::from(l.xs.x(local) - l.start_xs[ix]))
    }

    /// Bytes whose boxes can reach `visible` (relative to the text origin): a superset with generous margins
    /// (two visual lines, 8px for outline dilation). Everything when glyph x is not monotone.
    fn visible_bytes(&self, visible: Bounds<Pixels>, row_h: Pixels) -> Range<usize> {
        let end = self.lines.last().map_or(0, |l| l.base + l.len);
        if !self.lines.iter().all(|l| l.xs.monotone) {
            return 0..end + 1;
        }
        // A visible box is within both the line bounds and the x bounds; clamping the line bounds to the
        // text's lines makes the x bounds bite on single-line (unwrapped) text.
        let last_line = self.lines.last().map_or(0., |l| (l.first_visual + l.starts.len() - 1) as f32);
        let line_of = |y: Pixels| (f32::from(y) / f32::from(row_h)).floor().clamp(0., last_line);
        let lower = (line_of(visible.top() - row_h * 2.), f32::from(visible.left()) - 8.);
        let upper = (line_of(visible.bottom() + row_h * 2.), f32::from(visible.right()) + 8.);
        // First byte in 0..=end where the monotone predicate holds (end + 1 if none).
        let first = |pred: &dyn Fn((f32, f32)) -> bool| {
            let (mut lo, mut hi) = (0, end + 1);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if pred(self.position(mid)) { hi = mid } else { lo = mid + 1 }
            }
            lo
        };
        first(&|p| p >= lower)..first(&|p| p > upper)
    }
}

/// The items of sorted, disjoint `items` whose byte range can be visible: ends at or after `bytes.start`
/// and starts before `bytes.end`.
fn in_window<'a, T>(items: &'a [T], range: impl Fn(&T) -> &Range<usize>, bytes: &Range<usize>) -> &'a [T] {
    let from = items.partition_point(|it| range(it).end < bytes.start);
    let to = items.partition_point(|it| range(it).start < bytes.end).max(from);
    &items[from..to]
}

/// Rectangle for a text box, vertically centred in its visual line like the glyphs.
fn hit_box(origin: Point<Pixels>, row_h: Pixels, box_h: Pixels, (visual, x0, x1): (usize, Pixels, Pixels)) -> Bounds<Pixels> {
    let top = origin.y + row_h * visual as f32 + (row_h - box_h) / 2.;
    Bounds::new(point(origin.x + x0, top), gsize(x1 - x0, box_h))
}

/// Joins abutting ranges with equal values: `segments()` splits a hit wherever any other style changes.
fn merge_abutting<T: PartialEq>(items: impl IntoIterator<Item = (Range<usize>, T)>) -> Vec<(Range<usize>, T)> {
    let mut out: Vec<(Range<usize>, T)> = Vec::new();
    for (range, value) in items {
        match out.last_mut() {
            Some((last, v)) if last.end == range.start && *v == value => last.end = range.end,
            _ => out.push((range, value)),
        }
    }
    out
}

/// Splits `range` at the sorted visual line starts of a wrapped line of `len` bytes: (visual line, sub-range).
/// Only the lines the range touches are visited.
fn split_at_lines(range: Range<usize>, starts: &[usize], len: usize) -> Vec<(usize, Range<usize>)> {
    let first = starts.partition_point(|&s| s <= range.start).saturating_sub(1);
    starts
        .iter()
        .enumerate()
        .skip(first)
        .take_while(|&(_, &start)| start < range.end)
        .filter_map(|(ix, &start)| {
            let end = starts.get(ix + 1).copied().unwrap_or(len);
            let sub = range.start.max(start)..range.end.min(end);
            (sub.start < sub.end).then_some((ix, sub))
        })
        .collect()
}

enum RowBody {
    Gap(ShapedLine),
    Line {
        number: u64,
        number_line: ShapedLine,
        text: Shaped,
        pinned: bool,
        tint: Option<RowTint>,
        marks: Vec<(Range<usize>, Color)>,
        outlines: Vec<Range<usize>>,
    },
}

struct PaintRow {
    row: usize,
    bounds: Bounds<Pixels>,
    hitbox: Hitbox,
    body: RowBody,
}

pub struct LogViewPrepaint {
    hitbox: Hitbox,
    rows: Vec<PaintRow>,
    thumb: Option<Bounds<Pixels>>,
    theme: RoggTheme,
    selection: Option<(u64, u64)>,
    force_hover_row: Option<usize>,
    text_left: Pixels,
    scroll_x: Pixels,
}

fn run(len: usize, font: &Font, fg: Color, bg: Option<Color>) -> TextRun {
    TextRun { len, font: font.clone(), color: fg.hsla(), background_color: bg.map(|c| c.hsla()), underline: None, strikethrough: None }
}

/// Spans clipped to the original text kept by `display_text` (its first `cut` bytes), so a trailing `…`
/// stays base-colored, and widened to whole chars: text runs must end on char boundaries, and producers
/// may hand us byte offsets inside a multi-byte char.
fn clip_spans(spans: &[Span], display: &str, cut: usize) -> Vec<Span> {
    let kept = &display[..cut];
    spans
        .iter()
        .filter_map(|s| {
            let range = kept.floor_char_boundary(s.range.start.min(cut))..kept.ceil_char_boundary(s.range.end.min(cut));
            (range.start < range.end).then_some(Span { range, style: s.style })
        })
        .collect()
}

fn shape_row(rows: &dyn LogRowSource, row: usize, theme: &RoggTheme, mono: &Font, ui: &Font, wrap_width: Option<Pixels>, window: &mut Window) -> RowBody {
    let p = theme.palette;
    match rows.row(row) {
        LogRow::Gap { omitted } => {
            let label: SharedString = format!("··· {omitted} lines omitted").into();
            let runs = [run(label.len(), ui, p.faint, None)];
            RowBody::Gap(window.text_system().shape_line(label, px(9.), &runs, None))
        }
        LogRow::Line(line) => {
            let (display, truncated) = display_text(&line.text, MAX_DISPLAY_BYTES);
            let cut = if truncated { display.len() - '…'.len_utf8() } else { display.len() };
            let spans = clip_spans(&line.spans, &display, cut);
            let display: SharedString = display.into();
            let base = if line.context { p.muted } else { p.log_text };
            let segs: Vec<Segment> = segments(display.len(), &spans, base, &p);
            let runs: Vec<TextRun> = segs.iter().map(|s| run(s.range.len(), mono, s.fg, None)).collect();
            let marks = merge_abutting(segs.iter().filter_map(|s| s.hit_bg.map(|bg| (s.range.clone(), bg))));
            let outlines =
                merge_abutting(segs.iter().filter(|s| s.outline).map(|s| (s.range.clone(), ()))).into_iter().map(|(r, _)| r).collect();
            let number_text: SharedString = line.number.to_string().into();
            let number_runs = [run(number_text.len(), mono, p.faint, None)];
            let number_line = window.text_system().shape_line(number_text, px(10.), &number_runs, None);
            let font_size = theme.metrics.log_font_size;
            let text = match wrap_width {
                Some(w) => Shaped::Wrapped(
                    window.text_system().shape_text(display, font_size, &runs, Some(w), None).map(|v| v.into_vec()).unwrap_or_default(),
                ),
                None => Shaped::Line(window.text_system().shape_line(display, font_size, &runs, None)),
            };
            RowBody::Line { number: line.number, number_line, text, pinned: line.pinned, tint: line.tint, marks, outlines }
        }
    }
}

fn body_height(body: &RowBody, row_h: Pixels) -> f32 {
    match body {
        RowBody::Gap(_) => size::GAP_ROW_H,
        RowBody::Line { text: Shaped::Line(_), .. } => f32::from(row_h),
        RowBody::Line { text: Shaped::Wrapped(lines), .. } => {
            let h: f32 = lines.iter().map(|l| f32::from(l.size(row_h).height)).sum();
            h.max(f32::from(row_h))
        }
    }
}

impl Element for LogViewElement {
    type RequestLayoutState = ();
    type PrepaintState = LogViewPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        style.flex_grow = 1.;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> LogViewPrepaint {
        let theme = cx.rogg().clone();
        let row_h = theme.metrics.row_height;
        let text_left = bounds.left() + px(size::GUTTER_PIN_W + size::GUTTER_NUMBER_W);
        let text_right = bounds.right() - px(size::LINE_PAD_R);
        let text_w = f32::from(text_right - text_left).max(10.);
        let mono = font(theme.fonts.mono.clone());
        let ui = font(theme.fonts.ui.clone());

        self.state.update(cx, |s, _| s.begin_layout(bounds.size, text_w, f32::from(row_h)));

        // Wrapped rows: shape the visible set, record heights, and repeat once so positions use real heights.
        let wrap = self.state.read(cx).wrap();
        let mut bodies: HashMap<usize, RowBody> = HashMap::new();
        for _ in 0..(if wrap { 2 } else { 1 }) {
            let rows = self.state.read(cx).rows().clone();
            let visible = self.state.read(cx).visible_rows();
            let mut measured = Vec::new();
            for (row, _) in visible {
                if let Entry::Vacant(slot) = bodies.entry(row) {
                    let body = shape_row(rows.as_ref(), row, &theme, &mono, &ui, wrap.then(|| px(text_w)), window);
                    measured.push((row, body_height(&body, row_h)));
                    slot.insert(body);
                }
            }
            if wrap {
                self.state.update(cx, |s, _| s.record_heights(&measured));
            }
        }

        // Horizontal extent for no-wrap scrolling: a displayed line holds at most MAX_DISPLAY_BYTES chars plus `…`.
        let longest = self.state.read(cx).rows().longest_line_chars().min(MAX_DISPLAY_BYTES + 1) as f32;
        let font_id = window.text_system().resolve_font(&mono);
        let advance = window.text_system().ch_advance(font_id, theme.metrics.log_font_size).map(f32::from).unwrap_or(7.);
        self.state.update(cx, |s, _| s.set_max_scroll_x(longest * advance - text_w));

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let (visible, rows_src, selection, force_hover_row, scroll_x, thumb) = {
            let s = self.state.read(cx);
            (s.visible_rows(), s.rows().clone(), s.selection_range(), s.force_hover_row, s.scroll_x, s.thumb())
        };
        let mut rows = Vec::with_capacity(visible.len());
        let mut geometry = Vec::with_capacity(visible.len());
        for (row, y) in visible {
            let body = match bodies.remove(&row) {
                Some(b) => b,
                None => shape_row(rows_src.as_ref(), row, &theme, &mono, &ui, wrap.then(|| px(text_w)), window),
            };
            let h = self.state.read(cx).height_of(row);
            let row_bounds = Bounds::new(point(bounds.left(), bounds.top() + px(y)), gsize(bounds.size.width, px(h)));
            let row_hitbox = window.insert_hitbox(row_bounds, HitboxBehavior::Normal);
            geometry.push(RowGeometry { row, bounds: row_bounds, is_gap: matches!(body, RowBody::Gap(_)) });
            rows.push(PaintRow { row, bounds: row_bounds, hitbox: row_hitbox, body });
        }
        self.state.update(cx, |s, _| s.last_rows = geometry);

        let thumb = thumb.map(|(top, len)| {
            let track = f32::from(bounds.size.height);
            let h = (track * len).max(size::SCROLL_THUMB_MIN_H);
            let y = (track - h) * (top / (1. - len).max(f32::EPSILON)).clamp(0., 1.);
            Bounds::new(
                point(bounds.right() - px(size::SCROLL_THUMB_W + 2.), bounds.top() + px(y)),
                gsize(px(size::SCROLL_THUMB_W), px(h)),
            )
        });

        LogViewPrepaint { hitbox, rows, thumb, theme, selection, force_hover_row, text_left, scroll_x: px(scroll_x) }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        pre: &mut LogViewPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let p = pre.theme.palette;
        let m = pre.theme.metrics;
        let row_h = m.row_height;
        window.paint_quad(fill(bounds, p.bg));
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for row in &pre.rows {
                match &row.body {
                    RowBody::Gap(label) => {
                        window.paint_quad(fill(row.bounds, p.bg));
                        let origin = point(row.bounds.left() + px(size::GAP_ROW_INSET), row.bounds.top());
                        label.paint(origin, row.bounds.size.height, TextAlign::Left, None, window, cx).ok();
                    }
                    RowBody::Line { number, number_line, text, pinned, tint, marks, outlines } => {
                        let hovered = pre.force_hover_row == Some(row.row) || row.hitbox.is_hovered(window);
                        let selected = pre.selection.is_some_and(|(lo, hi)| (lo..=hi).contains(number));
                        let bg = if selected {
                            Some(p.selection)
                        } else if let Some(t) = tint {
                            Some(t.resolve(&p))
                        } else if hovered {
                            Some(p.hover)
                        } else {
                            None
                        };
                        if let Some(bg) = bg {
                            window.paint_quad(fill(row.bounds, bg));
                        }
                        if selected {
                            window.paint_quad(fill(Bounds::new(row.bounds.origin, gsize(px(2.), row.bounds.size.height)), p.accent));
                        }
                        let pin = Bounds::new(
                            point(row.bounds.left() + px((size::GUTTER_PIN_W - size::PIN_GLYPH) / 2.), row.bounds.top() + m.pin_top),
                            gsize(px(size::PIN_GLYPH), px(size::PIN_GLYPH)),
                        );
                        if *pinned {
                            window.paint_svg(pin, IconName::PinFilled.path().into(), None, TransformationMatrix::unit(), p.accent.hsla(), cx).ok();
                        } else if hovered {
                            window.paint_svg(pin, IconName::Pin.path().into(), None, TransformationMatrix::unit(), p.faint.hsla().opacity(0.7), cx).ok();
                        }
                        let number_x =
                            row.bounds.left() + px(size::GUTTER_PIN_W + size::GUTTER_NUMBER_W - size::NUMBER_PAD_R) - number_line.width();
                        number_line.paint(point(number_x, row.bounds.top()), row_h, TextAlign::Left, None, window, cx).ok();

                        // `.line{overflow:hidden; padding-right:20px}` clips at its padding box: the 20px only narrows
                        // the wrap width, unwrapped text runs to the row's right edge.
                        let clip = Bounds::from_corners(point(pre.text_left, row.bounds.top()), point(row.bounds.right(), row.bounds.bottom()));
                        let origin = point(pre.text_left - pre.scroll_x, row.bounds.top());
                        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                            // Boxes wholly outside the visible clip would paint nothing: skip them, and only
                            // look at the marks in the visible byte window.
                            let visible = window.content_mask().bounds;
                            let box_h = text.box_height();
                            let geometry = (!marks.is_empty() || !outlines.is_empty()).then(|| {
                                let geometry = text.geometry();
                                let bytes = geometry.visible_bytes(Bounds::new(visible.origin - origin, visible.size), row_h);
                                (geometry, bytes)
                            });
                            if let Some((geometry, bytes)) = &geometry {
                                for (range, color) in in_window(marks, |m| &m.0, bytes) {
                                    geometry.boxes(range, |b| {
                                        let mark = hit_box(origin, row_h, box_h, b);
                                        if mark.intersects(&visible) {
                                            window.paint_quad(fill(mark, *color).corner_radii(Corners::all(px(2.))));
                                        }
                                    });
                                }
                            }
                            text.paint(origin, row_h, window, cx);
                            if let Some((geometry, bytes)) = &geometry {
                                for range in in_window(outlines, |r| r, bytes) {
                                    geometry.boxes(range, |b| {
                                        let ring = hit_box(origin, row_h, box_h, b).dilate(px(2.));
                                        if ring.intersects(&visible) {
                                            window.paint_quad(outline(ring, p.accent, BorderStyle::Solid));
                                        }
                                    });
                                }
                            }
                        });
                    }
                }
            }
            if let Some(thumb) = pre.thumb {
                window.paint_quad(fill(thumb, p.scrollbar_thumb).corner_radii(Corners::all(px(3.))));
            }
        });

        let state = self.state.clone();
        let hitbox = pre.hitbox.clone();
        window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || !hitbox.should_handle_scroll(window) {
                return;
            }
            let delta = e.delta.pixel_delta(row_h);
            state.update(cx, |s, cx| {
                s.scroll_by_px(-f32::from(delta.y), -f32::from(delta.x));
                cx.notify();
            });
            cx.stop_propagation();
        });

        let targets: Vec<(Hitbox, usize, u64)> = pre
            .rows
            .iter()
            .filter_map(|r| match r.body {
                RowBody::Line { number, .. } => Some((r.hitbox.clone(), r.row, number)),
                RowBody::Gap(_) => None,
            })
            .collect();
        let state = self.state.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                return;
            }
            if let Some((hb, row, number)) = targets.iter().find(|(hb, _, _)| hb.is_hovered(window)) {
                let in_gutter = e.position.x < hb.bounds.left() + px(size::GUTTER_PIN_W);
                let (row, number) = (*row, *number);
                state.update(cx, |s, cx| {
                    if in_gutter {
                        cx.emit(LogViewEvent::TogglePin { number });
                    } else {
                        s.set_selected(Some(number), cx);
                        cx.emit(LogViewEvent::Selected { row, number });
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }
        });

        let row_hitboxes: Vec<(usize, Hitbox)> = pre.rows.iter().map(|r| (r.row, r.hitbox.clone())).collect();
        let hovered_now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let now = row_hitboxes.iter().find(|(_, h)| h.is_hovered(window)).map(|(r, _)| *r);
            if now != hovered_now {
                window.refresh();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logview::SpanStyle;

    #[test]
    fn clip_spans_keeps_ellipsis_uncovered() {
        let spans = vec![
            Span { range: 0..3, style: SpanStyle::Error },
            Span { range: 2..10, style: SpanStyle::Hit(0) },
            Span { range: 6..9, style: SpanStyle::SearchHit },
        ];
        let clipped = clip_spans(&spans, "abcdefghij", 5);
        assert_eq!(clipped, vec![Span { range: 0..3, style: SpanStyle::Error }, Span { range: 2..5, style: SpanStyle::Hit(0) }]);
    }

    #[test]
    fn clip_spans_widens_to_char_boundaries() {
        // "é" 0..2, " " 2..3, "日" 3..6, "x" 6..7, then the cut at 7 and a 3-byte "…".
        let display = "é 日x…";
        let span = |range: Range<usize>| Span { range, style: SpanStyle::Hit(0) };
        let clipped = clip_spans(&[span(1..2), span(1..4), span(4..5), span(5..99), span(7..9)], display, 7);
        assert_eq!(clipped, vec![span(0..2), span(0..6), span(3..6), span(3..7)]);
    }

    fn layout(glyphs: &[(usize, f32)], width: f32, len: usize) -> LineLayout {
        let glyphs = glyphs
            .iter()
            .map(|&(index, x)| gpui_kit::ShapedGlyph { id: gpui_kit::GlyphId(0), position: point(px(x), px(0.)), index, is_emoji: false })
            .collect();
        LineLayout { width: px(width), len, runs: vec![gpui_kit::ShapedRun { font_id: gpui_kit::FontId(0), glyphs }], ..Default::default() }
    }

    #[test]
    fn x_index_matches_x_for_index() {
        let multibyte = layout(&[(0, 0.), (2, 7.), (3, 14.), (6, 21.), (7, 28.)], 35., 8);
        // Out-of-order glyphs (as in right-to-left runs) still answer like the linear scan.
        let reordered = layout(&[(4, 14.), (5, 21.), (0, 0.), (1, 7.), (6, 28.)], 35., 7);
        for l in [&multibyte, &reordered] {
            let xs = XIndex::new(l);
            for i in 0..=l.len + 1 {
                assert_eq!(xs.x(i), l.x_for_index(i), "index {i}");
            }
        }
        assert!(XIndex::new(&multibyte).monotone);
        assert!(!XIndex::new(&reordered).monotone);
    }

    fn one_line(l: &LineLayout) -> TextGeometry {
        TextGeometry { lines: vec![LineGeometry { base: 0, len: l.len, first_visual: 0, starts: vec![0], start_xs: vec![px(0.)], xs: XIndex::new(l) }] }
    }

    /// Asserts that every mark with a (dilated) box inside `visible` survives the byte-window cut; returns the kept count.
    fn kept_marks(geometry: &TextGeometry, marks: &[Range<usize>], visible: Bounds<Pixels>, row_h: Pixels) -> usize {
        let kept = in_window(marks, |r| r, &geometry.visible_bytes(visible, row_h));
        for m in marks {
            let mut shown = false;
            geometry.boxes(m, |b| shown |= hit_box(point(px(0.), px(0.)), row_h, px(15.), b).dilate(px(2.)).intersects(&visible));
            assert!(!shown || kept.contains(m), "visible mark {m:?} skipped");
        }
        kept.len()
    }

    #[test]
    fn visible_window_keeps_every_visible_mark_and_little_else() {
        let glyphs: Vec<(usize, f32)> = (0..1000).map(|i| (i, i as f32 * 7.)).collect();
        let marks: Vec<Range<usize>> = (0..200).map(|i| i * 5..i * 5 + 3).collect();
        let row_h = px(23.);
        let geometry = one_line(&layout(&glyphs, 7000., 1000));
        for left in [0., 3., 700., 6990.] {
            let kept = kept_marks(&geometry, &marks, Bounds::new(point(px(left), px(0.)), gsize(px(700.), row_h)), row_h);
            assert!(kept < 25, "kept {kept} of {} at x {left}", marks.len());
        }

        // Wrapped into 10 visual lines of 100 bytes: only marks on the lines near the viewport.
        let mut wrapped = one_line(&layout(&glyphs, 7000., 1000));
        let line = wrapped.lines.first_mut().unwrap();
        line.starts = (0..10).map(|v| v * 100).collect();
        line.start_xs = line.starts.iter().map(|&s| line.xs.x(s)).collect();
        for top in [0., 3., 5. * 23., 9. * 23.] {
            let kept = kept_marks(&wrapped, &marks, Bounds::new(point(px(0.), px(top)), gsize(px(700.), row_h)), row_h);
            assert!(kept <= 6 * 20 + 1, "kept {kept} at y {top}");
        }

        // Not monotone: nothing can be skipped.
        let reordered = one_line(&layout(&[(4, 14.), (5, 21.), (0, 0.), (1, 7.), (6, 28.)], 35., 7));
        assert_eq!(reordered.visible_bytes(Bounds::new(point(px(100.), px(0.)), gsize(px(10.), row_h)), row_h), 0..8);
    }

    #[test]
    fn merge_abutting_joins_only_touching_equal_ranges() {
        let (a, b) = (Color(0x11), Color(0x22));
        // abutting + same color merge; abutting + different color stay; separated stay.
        let merged = merge_abutting(vec![(0..2, a), (2..5, a), (5..7, b), (9..10, b), (10..12, b), (14..15, a)]);
        assert_eq!(merged, vec![(0..5, a), (5..7, b), (9..12, b), (14..15, a)]);
        assert_eq!(merge_abutting(Vec::<(Range<usize>, ())>::new()), vec![]);
        // a search hit split by a style change becomes one outline
        assert_eq!(merge_abutting(vec![(3..6, ()), (6..9, ()), (9..11, ())]), vec![(3..11, ())]);
    }

    #[test]
    fn split_at_lines_cuts_ranges_at_wrap_starts() {
        let starts = [0, 10, 20];
        assert_eq!(split_at_lines(2..5, &starts, 25), vec![(0, 2..5)]);
        assert_eq!(split_at_lines(8..22, &starts, 25), vec![(0, 8..10), (1, 10..20), (2, 20..22)]);
        assert_eq!(split_at_lines(10..12, &starts, 25), vec![(1, 10..12)]);
        assert_eq!(split_at_lines(24..30, &starts, 25), vec![(2, 24..25)]);
        assert!(split_at_lines(5..5, &starts, 25).is_empty());
    }
}
