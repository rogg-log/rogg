//! The engine as `LogRowSource`s for the two panes. Rows are materialised on demand, for the visible range only;
//! each raw line is cut to the display limit before it is decoded (M1 carry-over).

use crate::document::{Document, Pane};
use crate::profiles::RowStyler;
use gpui_kit::SharedString;
use rogg_core::highlight::Highlighter;
use rogg_core::search::CompiledSearch;
use rogg_core::text::LogText;
use rogg_core::view::{FilteredView, ViewRow};
use rogg_ui::logview::text::MAX_DISPLAY_BYTES;
use rogg_ui::logview::{LogLine, LogRow, LogRowSource, Span, SpanStyle};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

/// Colour slots in the filter-hit palette (spec §5.1).
const HIT_SLOTS: usize = 4;

/// Line `i` for display (spec §4.4): a line over `MAX_DISPLAY_BYTES` is cut there — never inside a UTF-8 character —
/// and marked with `…`; invalid bytes become U+FFFD. Only the kept bytes are decoded, so a multi-megabyte line costs
/// no more than a 64 KB one.
pub fn display_line(text: &LogText, i: u64) -> SharedString {
    let raw = text.line(i);
    if raw.len() <= MAX_DISPLAY_BYTES {
        return String::from_utf8_lossy(raw).into_owned().into();
    }
    // Back up over continuation bytes (at most 3) so the cut falls before a character, not inside it.
    let mut cut = MAX_DISPLAY_BYTES;
    while cut > MAX_DISPLAY_BYTES - 3 && raw[cut] & 0xC0 == 0x80 {
        cut -= 1;
    }
    let mut display = String::from_utf8_lossy(&raw[..cut]).into_owned();
    display.push('…');
    display.into()
}

/// Whether `display_line` cuts line `i` (the line is longer than `MAX_DISPLAY_BYTES`); looks at the length only.
fn is_cut(text: &LogText, i: u64) -> bool {
    text.line(i).len() > MAX_DISPLAY_BYTES
}

/// What decorates a row besides its filter hits: the active find, the profile styler, and which pane it is for.
struct Decor<'a> {
    search: Option<&'a CompiledSearch>,
    styler: Option<&'a RowStyler>,
    filtered: bool,
}

fn decorated(display: SharedString, number: u64, pinned: bool, context: bool, hits: Vec<Span>, decor: Decor) -> LogLine {
    let (mut spans, tint) = decor.styler.map(|s| s.decorate(&display, decor.filtered)).unwrap_or_default();
    spans.extend(hits);
    if let Some(search) = decor.search {
        spans.extend(search.spans(&display).into_iter().map(|range| Span { range, style: SpanStyle::SearchHit }));
    }
    LogLine { number, text: display, spans, pinned, context, tint }
}

/// Every line of the text (the full log pane).
pub struct FullRows {
    pub text: Arc<LogText>,
    /// Display number of line 0.
    pub base: u64,
    pub pins: Arc<BTreeSet<u64>>,
    pub search: Option<Arc<CompiledSearch>>,
    pub styler: Option<Arc<RowStyler>>,
}

impl LogRowSource for FullRows {
    fn row_count(&self) -> usize {
        self.text.line_count() as usize
    }

    fn row(&self, ix: usize) -> LogRow {
        let i = ix as u64;
        LogRow::Line(decorated(display_line(&self.text, i), self.base + i, self.pins.contains(&i), false, Vec::new(), Decor { search: self.search.as_deref(), styler: self.styler.as_deref(), filtered: false }))
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

    fn is_display_cut(&self, ix: usize) -> bool {
        (ix as u64) < self.text.line_count() && is_cut(&self.text, ix as u64)
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
    pub styler: Option<Arc<RowStyler>>,
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
                    .map(|h| {
                        let slots = self.styler.as_ref().map_or(HIT_SLOTS, |s| s.palette_len());
                        h.spans(&display, slots)
                            .into_iter()
                            .map(|(range, slot)| Span { range, style: self.styler.as_ref().map_or(SpanStyle::Hit(slot), |s| s.hit(slot)) })
                            .collect()
                    })
                    .unwrap_or_default();
                LogRow::Line(decorated(display, self.base + number, pinned, !matched && !pinned, hits, Decor { search: self.search.as_deref(), styler: self.styler.as_deref(), filtered: true }))
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

    fn is_display_cut(&self, ix: usize) -> bool {
        matches!(self.view.row(ix as u64), Some(ViewRow::Line { number, .. }) if is_cut(&self.text, number))
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
    pub fn full_rows(&self, styler: Option<Arc<RowStyler>>) -> Option<Rc<FullRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FullRows { text, base: self.number_base(), pins: Arc::new(self.filter.pins.clone()), search: self.search(Pane::Full).compiled.clone(), styler }))
    }

    /// Rows for the filtered log pane; None until the text is loaded.
    pub fn filtered_rows(&self, styler: Option<Arc<RowStyler>>) -> Option<Rc<FilteredRows>> {
        let text = self.text()?.clone();
        Some(Rc::new(FilteredRows {
            text,
            base: self.number_base(),
            view: self.filter.view.clone(),
            highlighter: self.filter.highlighter.clone(),
            search: self.search(Pane::Filtered).compiled.clone(),
            styler,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::filter::FilterSpec;
    use rogg_core::search::SearchQuery;
    use rogg_ui::logview::{LogRow, LogRowSource, RowTint, SpanStyle};
    use rogg_ui::theme::{Appearance, Palette};

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
        let rows = FullRows { text: t, base: 100, pins: Arc::new(BTreeSet::from([1])), search: None, styler: Some(Arc::new(RowStyler::from_profile(&rogg_core::profile::builtin_default(), &Palette::dark(), Appearance::Dark))) };
        assert_eq!(rows.row_count(), 2);
        let first = line(rows.row(0));
        assert_eq!(first.number, 100);
        assert_eq!(first.tint, Some(RowTint::Color(Palette::dark().error_row_tint)));
        assert!(first.spans.iter().any(|s| *s == Span { range: 0..23, style: SpanStyle::Fg(Palette::dark().faint) }));
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
        let rows = FilteredRows { text: t.clone(), base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: None, styler: None };
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
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(ctx), highlighter: None, search: None, styler: None };
        assert!(line(rows.row(0)).context && !line(rows.row(1)).context && line(rows.row(2)).context);
    }

    #[test]
    fn search_hits_are_outlined() {
        let t = text(&["one timeout two timeout"]);
        let search = Arc::new(SearchQuery { text: "timeout".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FullRows { text: t, base: 1, pins: Arc::new(BTreeSet::new()), search: Some(search), styler: None };
        let ranges: Vec<_> = line(rows.row(0)).spans.into_iter().filter(|s| s.style == SpanStyle::SearchHit).map(|s| s.range).collect();
        assert_eq!(ranges, [4..11, 16..23]);
    }

    #[test]
    fn long_lines_are_truncated_for_display_but_copied_in_full() {
        let long = format!("{} needle", "a".repeat(70_000));
        let t = text(&[&long]);
        let display = display_line(&t, 0);
        assert_eq!(display.len(), MAX_DISPLAY_BYTES + '…'.len_utf8(), "cut at the limit and marked");
        assert!(display.ends_with('…'));
        let spec = FilterSpec::from_expression("needle || a");
        let view = FilteredView::build(Arc::new(vec![0]), &BTreeSet::new(), 0, 1, |_| false);
        let search = Arc::new(SearchQuery { text: "aaa".into(), regex: false, case_sensitive: false }.compile().unwrap());
        let rows = FilteredRows { text: t, base: 1, view: Arc::new(view), highlighter: Some(Arc::new(Highlighter::new(&spec).unwrap())), search: Some(search), styler: None };
        let row = line(rows.row(0));
        assert!(row.spans.iter().all(|s| s.range.end <= row.text.len()), "spans stay inside the display text");
        assert_eq!(rows.copy_text(0).map(|s| s.len()), Some(long.len()));
    }

    #[test]
    fn rows_report_which_lines_are_cut_for_display() {
        let long = "a".repeat(MAX_DISPLAY_BYTES + 1);
        let t = text(&["short", &long]);
        let full = FullRows { text: t.clone(), base: 1, pins: Arc::new(BTreeSet::new()), search: None, styler: None };
        assert!(!full.is_display_cut(0) && full.is_display_cut(1) && !full.is_display_cut(2));
        let view = FilteredView::build(Arc::new(vec![0, 1]), &BTreeSet::new(), 0, 2, |_| false);
        let filtered = FilteredRows { text: t, base: 1, view: Arc::new(view), highlighter: None, search: None, styler: None };
        assert!(!filtered.is_display_cut(0) && filtered.is_display_cut(1));
    }

    #[test]
    fn invalid_bytes_in_a_short_line_are_not_marked_as_cut() {
        let t = Arc::new(LogText::from_bytes(vec![0xFF; 30_000]));
        let display = display_line(&t, 0);
        assert_eq!(display.chars().count(), 30_000, "every invalid byte becomes one U+FFFD");
        assert!(!display.ends_with('…'));
        let (shown, cut) = rogg_ui::logview::text::display_text(&display, rogg_ui::logview::text::MAX_DISPLAY_TEXT_BYTES);
        assert!(!cut && shown.len() == display.len(), "the log view keeps it whole");
    }

    #[test]
    fn the_display_cut_never_splits_a_character() {
        let mut raw = vec![b'a'; MAX_DISPLAY_BYTES - 1];
        raw.extend("é".as_bytes()); // straddles the limit
        raw.extend(b"tail");
        let t = Arc::new(LogText::from_bytes(raw));
        let display = display_line(&t, 0);
        assert_eq!(display.as_ref(), format!("{}…", "a".repeat(MAX_DISPLAY_BYTES - 1)));
    }
}
