use crate::theme::{Color, Palette};
use gpui_kit::SharedString;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SpanStyle {
    Faint,
    Muted,
    Error,
    Amber,
    Fg(Color),
    /// Filter-match highlight using palette slot `n % 4`.
    Hit(usize),
    /// A filter match with explicit colours (a highlighting profile's palette); drawn like `Hit`.
    Mark { fg: Color, bg: Color },
    /// Bold text (a profile rule's bold effect).
    Bold,
    /// Per-panel search hit: accent outline.
    SearchHit,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Span {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RowTint {
    /// `.row.error-line` tint.
    Error,
    Color(Color),
}

impl RowTint {
    pub fn resolve(self, p: &Palette) -> Color {
        match self {
            RowTint::Error => p.error_row_tint,
            RowTint::Color(c) => c,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct LogLine {
    pub number: u64,
    pub text: SharedString,
    pub spans: Vec<Span>,
    pub pinned: bool,
    pub context: bool,
    pub tint: Option<RowTint>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum LogRow {
    Line(LogLine),
    Gap { omitted: u64 },
}

/// What a log view displays. M1/M2 implement this over the engine; M0 uses `VecRows`.
pub trait LogRowSource: 'static {
    fn row_count(&self) -> usize;
    fn row(&self, ix: usize) -> LogRow;
    fn is_gap(&self, ix: usize) -> bool;
    fn longest_line_chars(&self) -> usize;
    fn row_for_number(&self, number: u64) -> Option<usize>;

    /// Display number of the line at row `ix`; None for gap rows and out-of-range rows.
    fn number_at(&self, ix: usize) -> Option<u64> {
        if ix >= self.row_count() {
            return None;
        }
        match self.row(ix) {
            LogRow::Line(l) => Some(l.number),
            LogRow::Gap { .. } => None,
        }
    }

    /// The row showing `number`, or the closest row after it (the last row when none is) — used to keep the
    /// viewport on the same line when the rows change. Sources with ordered numbers should override this.
    fn nearest_row(&self, number: u64) -> Option<usize> {
        self.row_for_number(number)
    }

    /// Whether the row's text was cut for display (it ends with the `…` marker and the full line is longer).
    fn is_display_cut(&self, _ix: usize) -> bool {
        false
    }

    /// Full text of the line at row `ix` for copying (rows may hold display-truncated text); None for gaps.
    fn copy_text(&self, ix: usize) -> Option<String> {
        if ix >= self.row_count() {
            return None;
        }
        match self.row(ix) {
            LogRow::Line(l) => Some(l.text.to_string()),
            LogRow::Gap { .. } => None,
        }
    }
}

pub struct VecRows {
    rows: Vec<LogRow>,
    longest: usize,
}

impl VecRows {
    pub fn new(rows: Vec<LogRow>) -> Self {
        let longest = rows
            .iter()
            .map(|r| match r {
                LogRow::Line(l) => l.text.chars().count(),
                LogRow::Gap { .. } => 0,
            })
            .max()
            .unwrap_or(0);
        Self { rows, longest }
    }
}

impl LogRowSource for VecRows {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn row(&self, ix: usize) -> LogRow {
        self.rows[ix].clone()
    }

    fn is_gap(&self, ix: usize) -> bool {
        matches!(self.rows.get(ix), Some(LogRow::Gap { .. }))
    }

    fn longest_line_chars(&self) -> usize {
        self.longest
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, LogRow::Line(l) if l.number == number))
    }

    fn nearest_row(&self, number: u64) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        let after = self.rows.iter().position(|r| matches!(r, LogRow::Line(l) if l.number >= number));
        Some(after.unwrap_or(self.rows.len() - 1))
    }
}
