use super::model::{LogRow, LogRowSource};
use super::scroll::{ScrollAnchor, ScrollGeometry};
use super::text::{MAX_DISPLAY_TEXT_BYTES, display_text};
use crate::theme::size;
use gpui_kit::{Bounds, Context, EventEmitter, Pixels, Size, px};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

/// Wrapped-row heights kept; when a measurement would exceed this, the cache is cleared first.
const MAX_CACHED_HEIGHTS: usize = 20_000;
/// A copied range holds at most this many rows (a shift-click across millions of lines copies its start).
pub const MAX_COPY_ROWS: usize = 100_000;
/// The horizontal scrollbar stays visible this long after a horizontal scroll (it is otherwise shown only on hover).
pub const HBAR_LINGER: Duration = Duration::from_millis(1200);
/// The horizontal scrollbar shows while the pointer is within this distance of the panel's bottom edge: its own strip
/// (the 6 px thumb, its 2 px margin and the 3 px of slack its click area allows), so rows above stay clickable.
pub const HBAR_HOVER_ZONE: f32 = size::SCROLL_THUMB_W + 5.;

#[derive(Clone, Debug, PartialEq)]
pub struct RowGeometry {
    pub row: usize,
    pub bounds: Bounds<Pixels>,
    pub is_gap: bool,
    /// A wrapped row cut to `MAX_WRAPPED_LINES`.
    pub clipped: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogViewEvent {
    Selected { row: usize, number: u64 },
    TogglePin { number: u64 },
    /// Context menu "Filter on …" / "Exclude …": add the word as a filter term.
    AddTerm { text: String, exclude: bool },
    /// The user scrolled away from the end while in tail mode (live follow pauses on this).
    TailLeft,
    /// Context menu "Show full line…": open the whole line in a dialog.
    ShowFullLine { number: u64 },
}

/// Where `scroll_to_row` puts the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTo {
    Center,
    /// Only scroll if needed, by the smallest amount.
    Visible,
    Top,
}

/// The line under the pointer when the context menu opened, and the word under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextTarget {
    pub number: u64,
    pub pinned: bool,
    pub token: Option<String>,
}

/// A mouse drag in progress, started by a press on a row or a scrollbar thumb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drag {
    /// `chars`: the start row's number and the byte index the press landed on, while the drag stays in that row
    /// (a character selection); None once it left the row, and for shift-clicks and gutter presses.
    Select { chars: Option<(u64, usize)> },
    /// `grab`: pointer offset from the thumb's top (or left) edge.
    VThumb { grab: f32 },
    HThumb { grab: f32 },
}

/// Characters selected within one row by a drag: byte offsets into the row's display text, on char boundaries and
/// within the display cut. `anchor` is where the drag started, `head` where the pointer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharSelection {
    pub number: u64,
    pub anchor: usize,
    pub head: usize,
}

impl CharSelection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

/// Scroll position, selection, wrap and layout caches for one log panel.
pub struct LogViewState {
    pub(crate) rows: Rc<dyn LogRowSource>,
    pub(crate) anchor: ScrollAnchor,
    pub(crate) scroll_x: f32,
    pub(crate) max_scroll_x: f32,
    pub(crate) wrap: bool,
    pub(crate) tail: bool,
    /// The selection head (the line moved by the keyboard and shown in the status bar).
    pub(crate) selected: Option<u64>,
    /// The other end of a range selection; None for a single line.
    pub(crate) range_anchor: Option<u64>,
    /// Characters selected within the selected row; cleared whenever the line selection or the rows change.
    pub(crate) char_selection: Option<CharSelection>,
    pub(crate) force_hover_row: Option<usize>,
    pub(crate) pending_scroll: Option<(usize, ScrollTo)>,
    /// The request resolved by the current layout; re-applied once wrapped rows are measured.
    applied_scroll: Option<(usize, ScrollTo)>,
    pub(crate) viewport: Size<Pixels>,
    pub(crate) row_h: f32,
    /// Measured heights of wrapped rows for `heights_width`; unmeasured rows use one row height.
    pub(crate) heights: HashMap<usize, f32>,
    pub(crate) heights_width: f32,
    pub(crate) last_rows: Vec<RowGeometry>,
    pub(crate) drag: Option<Drag>,
    pub(crate) context_target: Option<ContextTarget>,
    /// The pointer is over the horizontal scrollbar's zone at the bottom edge.
    pub(crate) hbar_hover: bool,
    /// A horizontal scroll happened within the last `HBAR_LINGER`.
    pub(crate) hbar_scrolled: bool,
    /// Bumped on each horizontal scroll, so only the newest linger timer hides the bar.
    hbar_generation: u64,
    /// Always show the horizontal scrollbar (gallery specimens).
    pub(crate) force_hbar: bool,
    initialized: bool,
    /// Scrolls the user made (wheel, scrollbar, drag-select past an edge); programmatic scrolls do not count.
    user_scrolls: u64,
}

impl EventEmitter<LogViewEvent> for LogViewState {}

impl LogViewState {
    pub fn new(rows: Rc<dyn LogRowSource>) -> Self {
        Self {
            rows,
            anchor: ScrollAnchor::default(),
            scroll_x: 0.,
            max_scroll_x: 0.,
            wrap: false,
            tail: false,
            selected: None,
            range_anchor: None,
            char_selection: None,
            force_hover_row: None,
            pending_scroll: None,
            applied_scroll: None,
            viewport: Size::default(),
            row_h: 23.,
            heights: HashMap::new(),
            heights_width: 0.,
            last_rows: Vec::new(),
            drag: None,
            context_target: None,
            hbar_hover: false,
            hbar_scrolled: false,
            hbar_generation: 0,
            force_hbar: false,
            initialized: false,
            user_scrolls: 0,
        }
    }

    /// Whether the horizontal scrollbar is shown: only while the pointer is near the bottom edge, while its thumb is
    /// dragged, and briefly after a horizontal scroll (the vertical thumb is always shown).
    pub fn hbar_visible(&self) -> bool {
        self.force_hbar || self.hbar_hover || self.hbar_scrolled || matches!(self.drag, Some(Drag::HThumb { .. }))
    }

    /// Always show the horizontal scrollbar (gallery specimens).
    pub fn force_hbar(&mut self, on: bool, cx: &mut Context<Self>) {
        self.force_hbar = on;
        cx.notify();
    }

    pub(crate) fn set_hbar_hover(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.hbar_hover != on {
            self.hbar_hover = on;
            cx.notify();
        }
    }

    /// Shows the horizontal scrollbar for `HBAR_LINGER` after a horizontal scroll.
    pub(crate) fn note_horizontal_scroll(&mut self, cx: &mut Context<Self>) {
        self.hbar_scrolled = true;
        self.hbar_generation += 1;
        let generation = self.hbar_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HBAR_LINGER).await;
            let _ = this.update(cx, |s, cx| {
                if s.hbar_generation == generation {
                    s.hbar_scrolled = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn context_target(&self) -> Option<&ContextTarget> {
        self.context_target.as_ref()
    }

    pub fn rows(&self) -> &Rc<dyn LogRowSource> {
        &self.rows
    }

    /// Display number of the first line in view (skipping a gap row at the top); None when there are no rows. A
    /// requested top row (`ScrollTo::Top`) counts before the layout that applies it.
    pub fn top_number(&self) -> Option<u64> {
        let top = match self.pending_scroll {
            Some((row, ScrollTo::Top)) => row,
            _ => self.anchor.row,
        };
        (0..3).find_map(|k| self.rows.number_at(top + k))
    }

    /// Replaces the rows, keeping the line at the top of the viewport in place (a list scrolled to its top
    /// stays at its top), so filter changes don't make the view jump.
    pub fn set_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        let at_top = !self.initialized || self.with_geometry(|g| self.anchor == g.top());
        let keep = if at_top {
            None
        } else {
            (0..3).find_map(|k| self.rows.number_at(self.anchor.row + k).map(|n| (n, k == 0)))
        };
        self.rows = rows;
        self.char_selection = None;
        self.heights.clear();
        if let Some((number, same_row)) = keep
            && let Some(row) = self.rows.nearest_row(number)
        {
            self.anchor = ScrollAnchor { row, offset: if same_row { self.anchor.offset.max(0.) } else { 0. } };
        }
        if at_top && self.initialized {
            self.anchor = self.with_geometry(|g| g.top());
        }
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
        cx.notify();
    }

    /// Like `set_rows` for rows that start with the current ones (lines were appended): keeps the scroll position, the
    /// selection and the measured wrapped heights. In tail mode the anchor moves to the new end at once, so a wheel
    /// event arriving before the next layout is not judged against the stale end.
    pub fn extend_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.anchor = if self.tail { self.with_geometry(|g| g.bottom()) } else { self.with_geometry(|g| g.clamp(self.anchor)) };
        cx.notify();
    }

    /// Stick-to-bottom mode (live follow, spec §4.5): every layout keeps the last row in view, so appended rows scroll
    /// in. Scrolling away — the wheel, the scrollbar, or a jump to an earlier row — leaves it and emits `TailLeft`.
    pub fn set_tail(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tail = on;
        cx.notify();
    }

    pub fn tail(&self) -> bool {
        self.tail
    }

    /// Whether the view shows the end of its rows (true before the first layout and when everything fits).
    pub fn at_bottom(&self) -> bool {
        !self.initialized || self.with_geometry(|g| g.is_at_bottom(self.anchor))
    }

    /// After a user scroll: leaves tail mode when the end is no longer in view.
    pub(crate) fn user_scrolled(&mut self, cx: &mut Context<Self>) {
        self.user_scrolls += 1;
        if self.tail && !self.with_geometry(|g| g.is_at_bottom(self.anchor)) {
            self.tail = false;
            cx.emit(LogViewEvent::TailLeft);
        }
    }

    /// How many times the user has scrolled this view (wheel, scrollbar, drag-select past an edge).
    pub fn user_scrolls(&self) -> u64 {
        self.user_scrolls
    }

    pub fn wrap(&self) -> bool {
        self.wrap
    }

    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap;
        self.scroll_x = 0.;
        self.heights.clear();
        cx.notify();
    }

    /// Horizontal scroll offset of the text column (always zero in wrap mode).
    pub fn scroll_x(&self) -> Pixels {
        px(self.scroll_x)
    }

    pub fn selected(&self) -> Option<u64> {
        self.selected
    }

    /// Selects one line, or nothing; clears any range.
    pub fn set_selected(&mut self, number: Option<u64>, cx: &mut Context<Self>) {
        self.selected = number;
        self.range_anchor = None;
        self.char_selection = None;
        cx.notify();
    }

    pub fn char_selection(&self) -> Option<CharSelection> {
        self.char_selection
    }

    /// Selects characters `anchor..head` of line `number` (a drag within one row). Offsets snap back to char boundaries
    /// and stop at the display cut (never inside the `…` marker); an empty range clears the selection.
    pub fn set_char_selection(&mut self, number: u64, anchor: usize, head: usize, cx: &mut Context<Self>) {
        let Some(ix) = self.rows.row_for_number(number) else { return };
        let LogRow::Line(line) = self.rows.row(ix) else { return };
        let (display, truncated) = display_text(&line.text, MAX_DISPLAY_TEXT_BYTES);
        // The row producer may already have cut the line and added the marker itself.
        let cut = if truncated || (self.rows.is_display_cut(ix) && display.ends_with('…')) { display.len() - '…'.len_utf8() } else { display.len() };
        let snap = |i: usize| display.floor_char_boundary(i.min(cut));
        let (anchor, head) = (snap(anchor), snap(head));
        self.char_selection = (anchor != head).then_some(CharSelection { number, anchor, head });
        cx.notify();
    }

    pub fn clear_char_selection(&mut self, cx: &mut Context<Self>) {
        if self.char_selection.take().is_some() {
            cx.notify();
        }
    }

    /// Extends the selection from its anchor (the line selected before the first extension) to `number`.
    pub fn extend_selection(&mut self, number: u64, cx: &mut Context<Self>) {
        if self.range_anchor.is_none() {
            self.range_anchor = Some(self.selected.unwrap_or(number));
        }
        self.selected = Some(number);
        self.char_selection = None;
        cx.notify();
    }

    /// The selected lines as an inclusive `(first, last)` pair of display numbers.
    pub fn selection_range(&self) -> Option<(u64, u64)> {
        let head = self.selected?;
        let anchor = self.range_anchor.unwrap_or(head);
        Some((anchor.min(head), anchor.max(head)))
    }

    pub fn is_selected(&self, number: u64) -> bool {
        self.selection_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&number))
    }

    /// Scrolls `row` into place on the next layout (the viewport is only known during prepaint).
    pub fn scroll_to_row(&mut self, row: usize, how: ScrollTo, cx: &mut Context<Self>) {
        if self.tail && row + 1 < self.rows.row_count() {
            self.tail = false;
            cx.emit(LogViewEvent::TailLeft);
        }
        self.pending_scroll = Some((row, how));
        cx.notify();
    }

    /// Centres `row` on the next layout.
    pub fn center_on_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.scroll_to_row(row, ScrollTo::Center, cx);
    }

    /// Moves the selection by `delta` rows (skipping gap rows), scrolls it into view and emits `Selected`.
    /// With no selection, `delta >= 0` selects the first row and a negative delta the last; when the selected line has
    /// no row, it moves from the nearest row instead.
    pub fn move_selection(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        if count == 0 {
            return;
        }
        let target = match self.selected {
            None if delta >= 0 => 0,
            None => count - 1,
            Some(n) => match self.rows.row_for_number(n) {
                Some(row) => (row as isize + delta).clamp(0, count as isize - 1) as usize,
                // The selected line is not shown any more (a filter change): continue from where it would be.
                None => {
                    let near = self.rows.nearest_row(n).unwrap_or(0);
                    let after = self.rows.number_at(near).is_some_and(|m| m > n);
                    // `near` is the first row after the selection (or the last row when none is).
                    let steps = match (after, delta > 0) {
                        (true, true) => delta - 1,
                        (false, false) => delta + 1,
                        _ => delta,
                    };
                    (near as isize + steps).clamp(0, count as isize - 1) as usize
                }
            },
        };
        self.select_row(target, if delta >= 0 { 1 } else { -1 }, extend, cx);
    }

    /// PgUp / PgDn: moves the selection by one viewport of rows.
    pub fn page(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let page = self.with_geometry(|g| g.visible(self.anchor).len()).saturating_sub(1).max(1) as isize;
        self.move_selection(if down { page } else { -page }, extend, cx);
    }

    /// ⌘↑ / ⌘↓: selects the first or last line.
    pub fn select_edge(&mut self, last: bool, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        if count == 0 {
            return;
        }
        if last {
            self.select_row(count - 1, -1, extend, cx);
        } else {
            self.select_row(0, 1, extend, cx);
        }
    }

    fn select_row(&mut self, mut row: usize, step: isize, extend: bool, cx: &mut Context<Self>) {
        let count = self.rows.row_count();
        let mut guard = count;
        while self.rows.is_gap(row) && guard > 0 {
            let next = row as isize + step;
            if next < 0 || next >= count as isize {
                return;
            }
            row = next as usize;
            guard -= 1;
        }
        let Some(number) = self.rows.number_at(row) else { return };
        if extend {
            self.extend_selection(number, cx);
        } else {
            self.set_selected(Some(number), cx);
        }
        self.scroll_to_row(row, ScrollTo::Visible, cx);
        cx.emit(LogViewEvent::Selected { row, number });
    }

    /// The selected lines as text, one per line, gap rows skipped; `with_numbers` prefixes `number\t`.
    pub fn selected_text(&self, with_numbers: bool) -> Option<String> {
        if !with_numbers
            && let Some(c) = self.char_selection
            && self.selection_range() == Some((c.number, c.number))
        {
            // The display text's kept prefix has the same bytes as the line (control chars become spaces one for one),
            // so the offsets index the full line too.
            let text = self.rows.copy_text(self.rows.row_for_number(c.number)?)?;
            let r = c.range();
            let end = text.floor_char_boundary(r.end.min(text.len()));
            return Some(text[text.floor_char_boundary(r.start.min(end))..end].to_string());
        }
        let (lo, hi) = self.selection_range()?;
        let first = self.rows.row_for_number(lo)?;
        let last = self.rows.row_for_number(hi)?;
        let mut out = String::new();
        for ix in first..=last.min(first + MAX_COPY_ROWS - 1) {
            let (Some(number), Some(text)) = (self.rows.number_at(ix), self.rows.copy_text(ix)) else { continue };
            if with_numbers {
                let _ = write!(out, "{number}\t");
            }
            out.push_str(&text);
            out.push('\n');
        }
        Some(out)
    }

    /// Paint `row` as hovered (gallery specimens).
    pub fn force_hover_row(&mut self, row: Option<usize>, cx: &mut Context<Self>) {
        self.force_hover_row = row;
        cx.notify();
    }

    /// Bounds of the rows painted in the last frame (window coordinates); used by tests and capture geometry.
    pub fn last_rows(&self) -> &[RowGeometry] {
        &self.last_rows
    }

    pub(crate) fn height_of(&self, row: usize) -> f32 {
        if self.rows.is_gap(row) {
            size::GAP_ROW_H
        } else if self.wrap {
            self.heights.get(&row).copied().unwrap_or(self.row_h)
        } else {
            self.row_h
        }
    }

    pub(crate) fn with_geometry<R>(&self, f: impl FnOnce(&ScrollGeometry) -> R) -> R {
        let height_of = |row: usize| self.height_of(row);
        let geometry = ScrollGeometry {
            row_count: self.rows.row_count(),
            viewport: f32::from(self.viewport.height),
            pad: size::LOGS_PAD_Y,
            height_of: &height_of,
        };
        f(&geometry)
    }

    fn resolve_scroll(&self, (row, how): (usize, ScrollTo)) -> ScrollAnchor {
        self.with_geometry(|g| match how {
            ScrollTo::Center => g.center_on(row),
            ScrollTo::Visible => g.ensure_visible(self.anchor, row),
            ScrollTo::Top => g.row_top(row),
        })
    }

    /// Called at the start of prepaint with the element size and current metrics.
    pub(crate) fn begin_layout(&mut self, viewport: Size<Pixels>, text_width: f32, row_h: f32) {
        if self.row_h != row_h || (self.wrap && self.heights_width != text_width) {
            self.heights.clear();
        }
        self.row_h = row_h;
        self.heights_width = text_width;
        self.viewport = viewport;
        if !self.initialized {
            self.anchor = self.with_geometry(|g| g.top());
            self.initialized = true;
        }
        self.applied_scroll = self.pending_scroll.take();
        self.anchor = match self.applied_scroll {
            Some(request) => self.resolve_scroll(request),
            None if self.tail => self.with_geometry(|g| g.bottom()),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    /// Stores measured wrapped heights and re-resolves this frame's scroll request with them, so a centred
    /// row stays centred once its real height is known.
    pub(crate) fn record_heights(&mut self, measured: &[(usize, f32)]) {
        if self.heights.len() + measured.len() > MAX_CACHED_HEIGHTS {
            self.heights.clear();
        }
        for &(row, h) in measured {
            self.heights.insert(row, h);
        }
        self.anchor = match self.applied_scroll {
            Some(request) => self.resolve_scroll(request),
            None if self.tail => self.with_geometry(|g| g.bottom()),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    pub(crate) fn visible_rows(&self) -> Vec<(usize, f32)> {
        self.with_geometry(|g| g.visible(self.anchor))
    }

    pub(crate) fn thumb(&self) -> Option<(f32, f32)> {
        self.with_geometry(|g| g.thumb(self.anchor))
    }

    /// Updates the horizontal extent and re-clamps the offset (lines may have shrunk or the view widened).
    pub(crate) fn set_max_scroll_x(&mut self, max: f32) {
        self.max_scroll_x = max.max(0.);
        self.scroll_x = if self.wrap { 0. } else { self.scroll_x.clamp(0., self.max_scroll_x) };
    }

    pub(crate) fn scroll_by_px(&mut self, dy: f32, dx: f32) {
        self.anchor = self.with_geometry(|g| g.scroll_by(self.anchor, dy));
        if !self.wrap {
            self.scroll_x = (self.scroll_x + dx).clamp(0., self.max_scroll_x.max(0.));
        }
    }

    /// Drag-select with the pointer above (`down` false) or below the panel: scrolls one row and moves the
    /// selection head to the first or last visible line row (gap rows skipped), emitting `Selected` on change.
    pub(crate) fn autoscroll_select(&mut self, down: bool, cx: &mut Context<Self>) {
        let dy = if down { self.row_h } else { -self.row_h };
        self.scroll_by_px(dy, 0.);
        self.user_scrolled(cx);
        let visible = self.visible_rows();
        let mut lines = visible.iter().map(|&(row, _)| row).filter(|&row| !self.rows.is_gap(row));
        let edge = if down { lines.next_back() } else { lines.next() };
        if let Some(row) = edge
            && let Some(number) = self.rows.number_at(row)
            && self.selected != Some(number)
        {
            self.extend_selection(number, cx);
            cx.emit(LogViewEvent::Selected { row, number });
        }
        cx.notify();
    }

    /// Scrolls so the vertical thumb's top sits `thumb_top` px below the track top (thumb of `thumb_h` px in a
    /// track of `track` px), using the same mapping the element uses to place the thumb.
    pub(crate) fn drag_vthumb(&mut self, thumb_top: f32, track: f32, thumb_h: f32) {
        let Some((_, len)) = self.thumb() else { return };
        let travel = (track - thumb_h).max(1.);
        let pos = (thumb_top / travel).clamp(0., 1.) * (1. - len);
        self.anchor = self.with_geometry(|g| g.at_position(pos));
    }

    /// Scrolls horizontally so the thumb's left edge sits `thumb_left` px into a track with `travel` px of play.
    pub(crate) fn drag_hthumb(&mut self, thumb_left: f32, travel: f32) {
        if self.wrap {
            return;
        }
        self.scroll_x = ((thumb_left / travel.max(1.)).clamp(0., 1.) * self.max_scroll_x).clamp(0., self.max_scroll_x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logview::{LogLine, LogRow, VecRows};

    #[test]
    fn heights_cache_is_bounded() {
        let rows: Vec<LogRow> = (0..10)
            .map(|i| LogRow::Line(LogLine { number: i, text: "x".into(), spans: vec![], pinned: false, context: false, tint: None }))
            .collect();
        let mut s = LogViewState::new(Rc::new(VecRows::new(rows)));
        s.wrap = true;
        let many: Vec<(usize, f32)> = (0..MAX_CACHED_HEIGHTS + 10).map(|r| (r, 46.)).collect();
        s.record_heights(&many);
        assert!(s.heights.len() <= MAX_CACHED_HEIGHTS + 10);
        s.record_heights(&[(1, 69.)]);
        assert!(s.heights.len() <= 1, "an over-full cache is cleared before inserting");
    }
}
