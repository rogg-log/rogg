use super::model::LogRowSource;
use super::scroll::{ScrollAnchor, ScrollGeometry};
use crate::theme::size;
use gpui_kit::{Bounds, Context, EventEmitter, Pixels, Size, px};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct RowGeometry {
    pub row: usize,
    pub bounds: Bounds<Pixels>,
    pub is_gap: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogViewEvent {
    Selected { row: usize, number: u64 },
    TogglePin { number: u64 },
}

/// Scroll position, selection, wrap and layout caches for one log panel.
pub struct LogViewState {
    pub(crate) rows: Rc<dyn LogRowSource>,
    pub(crate) anchor: ScrollAnchor,
    pub(crate) scroll_x: f32,
    pub(crate) max_scroll_x: f32,
    pub(crate) wrap: bool,
    pub(crate) selected: Option<u64>,
    pub(crate) force_hover_row: Option<usize>,
    pub(crate) pending_center: Option<usize>,
    pub(crate) viewport: Size<Pixels>,
    pub(crate) row_h: f32,
    /// Measured heights of wrapped rows for `heights_width`; unmeasured rows use one row height.
    pub(crate) heights: HashMap<usize, f32>,
    pub(crate) heights_width: f32,
    pub(crate) last_rows: Vec<RowGeometry>,
    initialized: bool,
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
            selected: None,
            force_hover_row: None,
            pending_center: None,
            viewport: Size::default(),
            row_h: 23.,
            heights: HashMap::new(),
            heights_width: 0.,
            last_rows: Vec::new(),
            initialized: false,
        }
    }

    pub fn rows(&self) -> &Rc<dyn LogRowSource> {
        &self.rows
    }

    pub fn set_rows(&mut self, rows: Rc<dyn LogRowSource>, cx: &mut Context<Self>) {
        self.rows = rows;
        self.heights.clear();
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
        cx.notify();
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

    pub fn set_selected(&mut self, number: Option<u64>, cx: &mut Context<Self>) {
        self.selected = number;
        cx.notify();
    }

    /// Centres `row` on the next layout (the viewport is only known during prepaint).
    pub fn center_on_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.pending_center = Some(row);
        cx.notify();
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
        self.anchor = match self.pending_center.take() {
            Some(row) => self.with_geometry(|g| g.center_on(row)),
            None => self.with_geometry(|g| g.clamp(self.anchor)),
        };
    }

    pub(crate) fn record_heights(&mut self, measured: &[(usize, f32)]) {
        for &(row, h) in measured {
            self.heights.insert(row, h);
        }
        self.anchor = self.with_geometry(|g| g.clamp(self.anchor));
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
}
