//! The engine as `LogRowSource`s for the two panes. Rows are materialised on demand, for the visible range only;
//! each raw line is cut to the display limit before it is decoded (M1 carry-over).

use crate::decorate;
use crate::document::{Document, Pane};
use gpui_kit::SharedString;
use rogg_core::highlight::Highlighter;
use rogg_core::search::CompiledSearch;
use rogg_core::text::LogText;
use rogg_core::view::{FilteredView, ViewRow};
use rogg_ui::logview::text::MAX_DISPLAY_BYTES;
use rogg_ui::logview::{LogLine, LogRow, LogRowSource, RowTint, Span, SpanStyle};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

/// Colour slots in the filter-hit palette (spec §5.1).
const HIT_SLOTS: usize = 4;

/// At most `MAX_DISPLAY_BYTES` + 4 bytes of line `i`, lossily decoded: enough for the log view to cut the line
/// at the display limit and append `…`, without decoding a multi-megabyte line.
pub fn display_line(text: &LogText, i: u64) -> SharedString {
    let raw = text.line(i);
    let cut = raw.len().min(MAX_DISPLAY_BYTES + 4);
    String::from_utf8_lossy(&raw[..cut]).into_owned().into()
}

fn decorated(display: SharedString, number: u64, pinned: bool, context: bool, hits: Vec<Span>, search: Option<&CompiledSearch>) -> LogLine {
    let mut spans = Vec::with_capacity(hits.len() + 1);
    let stamp = decorate::timestamp_len(&display);
    if stamp > 0 {
        spans.push(Span { range: 0..stamp, style: SpanStyle::Faint });
    }
    spans.extend(hits);
    if let Some(search) = search {
        spans.extend(search.spans(&display).into_iter().map(|range| Span { range, style: SpanStyle::SearchHit }));
    }
    let tint = decorate::is_error(&display).then_some(RowTint::Error);
    LogLine { number, text: display, spans, pinned, context, tint }
}

/// Every line of the text (the full log pane).
pub struct FullRows {
    pub text: Arc<LogText>,
    /// Display number of line 0.
    pub base: u64,
    pub pins: Arc<BTreeSet<u64>>,
    pub search: Option<Arc<CompiledSearch>>,
}

impl LogRowSource for FullRows {
    fn row_count(&self) -> usize {
        self.text.line_count() as usize
    }

    fn row(&self, ix: usize) -> LogRow {
        let i = ix as u64;
        LogRow::Line(decorated(display_line(&self.text, i), self.base + i, self.pins.contains(&i), false, Vec::new(), self.search.as_deref()))
    }

    fn is_gap(&self, _: usize) -> bool {
        false
    }

    fn longest_line_chars(&self) -> usize {
        self.text.index().longest_line() as usize
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        let i = number.checked_sub(self.base)?;
        (i < self.text.line_count()).then_some(i as usize)
    }

    fn number_at(&self, ix: usize) -> Option<u64> {
        ((ix as u64) < self.text.line_count()).then(|| self.base + ix as u64)
    }

    fn nearest_row(&self, number: u64) -> Option<usize> {
        let count = self.text.line_count();
        (count > 0).then(|| number.saturating_sub(self.base).min(count - 1) as usize)
    }

    fn copy_text(&self, ix: usize) -> Option<String> {
        ((ix as u64) < self.text.line_count()).then(|| self.text.line_text(ix as u64).into_owned())
    }
}

/// Matches, pins, context and gap rows (the filtered log pane).
pub struct FilteredRows {
    pub text: Arc<LogText>,
    pub base: u64,
    pub view: Arc<FilteredView>,
    pub highlighter: Option<Arc<Highlighter>>,
    pub search: Option<Arc<CompiledSearch>>,
}

impl LogRowSource for FilteredRows {
    fn row_count(&self) -> usize {
        self.view.row_count() as usize
    }

    fn row(&self, ix: usize) -> LogRow {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, matched, pinned }) => {
                let display = display_line(&self.text, number);
                let hits = self
                    .highlighter
                    .as_ref()
                    .map(|h| h.spans(&display, HIT_SLOTS).into_iter().map(|(range, slot)| Span { range, style: SpanStyle::Hit(slot) }).collect())
                    .unwrap_or_default();
                LogRow::Line(decorated(display, self.base + number, pinned, !matched && !pinned, hits, self.search.as_deref()))
            }
            Some(ViewRow::Gap { omitted }) => LogRow::Gap { omitted },
            None => LogRow::Gap { omitted: 0 },
        }
    }

    fn is_gap(&self, ix: usize) -> bool {
        matches!(self.view.row(ix as u64), Some(ViewRow::Gap { .. }))
    }

    fn longest_line_chars(&self) -> usize {
        self.text.index().longest_line() as usize
    }

    fn row_for_number(&self, number: u64) -> Option<usize> {
        let line = number.checked_sub(self.base)?;
        self.view.row_for_line(line).map(|r| r as usize)
    }

    fn number_at(&self, ix: usize) -> Option<u64> {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, .. }) => Some(self.base + number),
            _ => None,
        }
    }

    fn nearest_row(&self, number: u64) -> Option<usize> {
        let shown = self.view.shown_lines();
        if shown.is_empty() {
            return None;
        }
        let line = number.saturating_sub(self.base);
        let i = shown.partition_point(|&l| l < line).min(shown.len() - 1);
        self.view.row_for_line(shown[i]).map(|r| r as usize)
    }

    fn copy_text(&self, ix: usize) -> Option<String> {
        match self.view.row(ix as u64) {
            Some(ViewRow::Line { number, .. }) => Some(self.text.line_text(number).into_owned()),
            _ => None,
        }
    }
}

impl Document {
    /// Rows for the full log pane; None until the text is loaded.
    pub fn full_rows(&self) -> Option<Rc<FullRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FullRows { text, base: self.number_base(), pins: Arc::new(self.filter.pins.clone()), search: self.search(Pane::Full).compiled.clone() }))
    }

    /// Rows for the filtered log pane; None until the text is loaded.
    pub fn filtered_rows(&self) -> Option<Rc<FilteredRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FilteredRows {
            text,
            base: self.number_base(),
            view: self.filter.view.clone(),
            highlighter: self.filter.highlighter.clone(),
            search: self.search(Pane::Filtered).compiled.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::filter::FilterSpec;
    use rogg_core::search::SearchQuery;
    use rogg_ui::logview::{LogRow, LogRowSource, RowTint, SpanStyle};

    fn text(lines: &[&str]) -> Arc<LogText> {
        Arc::new(LogText::from_bytes(lines.join("\n").into_bytes()))
    }

    fn line(row: LogRow) -> rogg_ui::logview::LogLine {
        match row {
            LogRow::Line(l) => l,
            LogRow::Gap { omitted } => panic!("expected a line, got a gap of {omitted}"),
        }
    }

    #[test]
    fn full_rows_number_pin_and_decorate() {
        let t = text(&["2026-10-05 14:32:08.142  ERROR  disk full", "plain"]);
        let rows = FullRows { text: t, base: 100, pins: Arc::new(BTreeSet::from([1])), search: None };
        assert_eq!(rows.row_count(), 2);
        let first = line(rows.row(0));
        assert_eq!(first.number, 100);
        assert_eq!(first.tint, Some(RowTint::Error));
        assert_eq!(first.spans[0].range, 0..23);
        assert_eq!(first.spans[0].style, SpanStyle::Faint);
        assert!(!first.pinned && !first.context);
        let second = line(rows.row(1));
        assert!(second.pinned && second.spans.is_empty() && second.tint.is_none());
        assert_eq!(rows.row_for_number(101), Some(1));
        assert_eq!(rows.row_for_number(99), None);
        assert_eq!(rows.number_at(1), Some(101));
        assert_eq!(rows.nearest_row(500), Some(1));
        assert_eq!(rows.copy_text(0).as_deref(), Some("2026-10-05 14:32:08.142  ERROR  disk full"));
    }

    #[test]
    fn filtered_rows_show_hits_context_and_gaps() {
        let t = text(&["a", "ERROR disk full", "b", "c", "d", "pinned line", "e", "f"]);
        let spec = FilterSpec::from_expression("disk");
        let view = FilteredView::build(Arc::new(vec![1]), &BTreeSet::from([5]), 0, 8, |_| false);
        let rows = FilteredRows { text: t.clone(), base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: None };
        // shown lines 1 and 5 with a gap of three between them
        assert_eq!(rows.row_count(), 3);
        let hit = line(rows.row(0));
        assert_eq!(hit.number, 2);
        assert!(hit.spans.iter().any(|s| s.range == (6..10) && s.style == SpanStyle::Hit(0)));
        assert_eq!(rows.row(1), LogRow::Gap { omitted: 3 });
        assert!(rows.is_gap(1) && !rows.is_gap(0));
        let pinned = line(rows.row(2));
        assert!(pinned.pinned && !pinned.context);
        assert_eq!(rows.row_for_number(6), Some(2));
        assert_eq!(rows.row_for_number(3), None, "hidden lines have no row");
        assert_eq!(rows.nearest_row(3), Some(2), "the next shown line");
        assert_eq!(rows.number_at(1), None);
        assert_eq!(rows.copy_text(2).as_deref(), Some("pinned line"));

        let ctx = FilteredView::build(Arc::new(vec![1]), &BTreeSet::new(), 1, 8, |_| false);
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(ctx), highlighter: None, search: None };
        assert!(line(rows.row(0)).context && !line(rows.row(1)).context && line(rows.row(2)).context);
    }

    #[test]
    fn search_hits_are_outlined() {
        let t = text(&["one timeout two timeout"]);
        let search = Arc::new(SearchQuery { text: "timeout".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FullRows { text: t, base: 1, pins: Arc::new(BTreeSet::new()), search: Some(search) };
        let ranges: Vec<_> = line(rows.row(0)).spans.into_iter().filter(|s| s.style == SpanStyle::SearchHit).map(|s| s.range).collect();
        assert_eq!(ranges, [4..11, 16..23]);
    }

    #[test]
    fn long_lines_are_truncated_for_display_but_copied_in_full() {
        let long = format!("{} needle", "a".repeat(70_000));
        let t = text(&[&long]);
        let display = display_line(&t, 0);
        assert_eq!(display.len(), MAX_DISPLAY_BYTES + 4, "only a little past the display limit is decoded");
        let spec = FilterSpec::from_expression("needle || a");
        let view = FilteredView::build(Arc::new(vec![0]), &BTreeSet::new(), 0, 1, |_| false);
        let search = Arc::new(SearchQuery { text: "aaa".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: Some(search) };
        let row = line(rows.row(0));
        assert!(row.spans.iter().all(|s| s.range.end <= row.text.len()), "spans stay inside the display text");
        assert_eq!(rows.copy_text(0).map(|s| s.len()), Some(long.len()));
    }
}
