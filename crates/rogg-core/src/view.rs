//! The filtered view: matches ∪ pins ∪ context, with gap rows (spec §4.7).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::filter::MAX_CONTEXT;

/// A row checkpoint is stored every CHECKPOINT shown lines; row lookups walk at most this many lines.
pub const CHECKPOINT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewRow {
    Line { number: u64, matched: bool, pinned: bool },
    Gap { omitted: u64 },
}

impl ViewRow {
    /// A context line: shown only because it is near a match or pin.
    pub fn is_context(&self) -> bool {
        matches!(self, ViewRow::Line { matched: false, pinned: false, .. })
    }
}

pub struct FilteredView {
    matches: Arc<Vec<u64>>,
    pins: BTreeSet<u64>,
    shown: Arc<Vec<u64>>,
    checkpoints: Vec<u64>,
    row_count: u64,
    context_lines: u64,
}

fn merge_anchors(matches: &[u64], pins: &BTreeSet<u64>) -> Vec<u64> {
    let mut out = Vec::with_capacity(matches.len() + pins.len());
    let mut pins = pins.iter().copied().peekable();
    for &m in matches {
        while let Some(&p) = pins.peek() {
            if p >= m {
                break;
            }
            out.push(p);
            pins.next();
        }
        if pins.peek() == Some(&m) {
            pins.next();
        }
        out.push(m);
    }
    out.extend(pins);
    out
}

fn compose(anchors: &[u64], radius: u64, total: u64, excluded: &impl Fn(u64) -> bool) -> Vec<u64> {
    if total == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((anchors.len() * (1 + 2 * radius as usize).min(8)).min(total as usize));
    let mut next_free = 0u64;
    for (k, &a) in anchors.iter().enumerate() {
        for j in a.saturating_sub(radius).max(next_free)..a {
            if !excluded(j) {
                out.push(j);
            }
        }
        out.push(a);
        let hi = (a + radius).min(total - 1);
        let next_anchor = anchors.get(k + 1).copied().unwrap_or(u64::MAX);
        let mut j = a + 1;
        while j <= hi && j < next_anchor {
            if !excluded(j) {
                out.push(j);
            }
            j += 1;
        }
        next_free = j;
    }
    out
}

fn layout(shown: &[u64]) -> (Vec<u64>, u64) {
    let mut checkpoints = Vec::with_capacity(shown.len() / CHECKPOINT + 1);
    let mut row = 0u64;
    for (k, &line) in shown.iter().enumerate() {
        if k > 0 && line > shown[k - 1] + 1 {
            row += 1;
        }
        if k % CHECKPOINT == 0 {
            checkpoints.push(row);
        }
        row += 1;
    }
    (checkpoints, row)
}

impl FilteredView {
    /// Builds the view of `matches` plus `pins` plus `context` lines around each, with gap rows.
    ///
    /// Preconditions: `matches` is sorted, unique, and every value is `< total_lines`. Pins at or
    /// beyond `total_lines` are ignored. `context` is clamped to `MAX_CONTEXT` (spec §4.7).
    /// `excluded` tells whether a context line is hidden by an exclude term; callers pass
    /// `|n| filter.is_excluded(text.line(n))`. Matches and pins are always shown, even if excluded.
    pub fn build(matches: Arc<Vec<u64>>, pins: &BTreeSet<u64>, context: u32, total_lines: u64, excluded: impl Fn(u64) -> bool) -> Self {
        let context = context.min(MAX_CONTEXT);
        let pins: BTreeSet<u64> = pins.iter().copied().filter(|&p| p < total_lines).collect();
        let shown = if context == 0 && pins.is_empty() {
            matches.clone()
        } else {
            let anchors = merge_anchors(&matches, &pins);
            Arc::new(compose(&anchors, context as u64, total_lines, &excluded))
        };
        // Every in-range match and pin is shown, so context lines are what remains.
        let matches_in_range = matches.partition_point(|&m| m < total_lines) as u64;
        let pinned_unmatched = pins.iter().filter(|p| matches.binary_search(p).is_err()).count() as u64;
        let context_lines = shown.len() as u64 - matches_in_range - pinned_unmatched;
        let (checkpoints, row_count) = layout(&shown);
        Self { matches, pins, shown, checkpoints, row_count, context_lines }
    }

    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    fn gap_before(&self, k: usize) -> bool {
        k > 0 && self.shown[k] > self.shown[k - 1] + 1
    }

    /// The row of line `number` (matched / pinned flags only; the line need not be shown).
    pub fn line_row(&self, number: u64) -> ViewRow {
        ViewRow::Line { number, matched: self.matches.binary_search(&number).is_ok(), pinned: self.pins.contains(&number) }
    }

    pub fn row(&self, r: u64) -> Option<ViewRow> {
        if r >= self.row_count {
            return None;
        }
        let c = self.checkpoints.partition_point(|&x| x <= r) - 1;
        let mut k = c * CHECKPOINT;
        let mut row = self.checkpoints[c];
        while row < r {
            k += 1;
            row += 1 + self.gap_before(k) as u64;
        }
        if row == r {
            Some(self.line_row(self.shown[k]))
        } else {
            Some(ViewRow::Gap { omitted: self.shown[k] - self.shown[k - 1] - 1 })
        }
    }

    pub fn row_for_line(&self, number: u64) -> Option<u64> {
        let k = self.shown.binary_search(&number).ok()?;
        let c = k / CHECKPOINT;
        let mut row = self.checkpoints[c];
        for j in c * CHECKPOINT + 1..=k {
            row += 1 + self.gap_before(j) as u64;
        }
        Some(row)
    }

    /// A matched or pinned line (shown for itself, not as context).
    pub fn is_anchor(&self, line: u64) -> bool {
        self.matches.binary_search(&line).is_ok() || self.pins.contains(&line)
    }

    pub fn shown_lines(&self) -> &Arc<Vec<u64>> {
        &self.shown
    }

    pub fn matches(&self) -> &Arc<Vec<u64>> {
        &self.matches
    }

    pub fn context_lines(&self) -> u64 {
        self.context_lines
    }

    pub fn pinned_count(&self) -> u64 {
        self.pins.len() as u64
    }

    pub fn pinned_unmatched(&self) -> u64 {
        self.pins.iter().filter(|p| self.matches.binary_search(p).is_err()).count() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXCLUDED: [u64; 4] = [5, 6, 18, 26];

    fn reference(context: u32) -> FilteredView {
        FilteredView::build(Arc::new(vec![12, 15, 20, 23, 28, 34]), &BTreeSet::from([7]), context, 40, |l| EXCLUDED.contains(&l))
    }

    fn line(r: Option<ViewRow>) -> u64 {
        match r {
            Some(ViewRow::Line { number, .. }) => number,
            other => panic!("expected a line row, got {other:?}"),
        }
    }

    #[test]
    fn reference_without_context() {
        let v = reference(0);
        assert_eq!(**v.shown_lines(), vec![7, 12, 15, 20, 23, 28, 34]);
        assert_eq!(v.row_count(), 7 + 6);
        assert_eq!(v.row(0), Some(ViewRow::Line { number: 7, matched: false, pinned: true }));
        assert_eq!(v.row(1), Some(ViewRow::Gap { omitted: 4 }));
        assert_eq!(v.row(2), Some(ViewRow::Line { number: 12, matched: true, pinned: false }));
        assert_eq!(v.row(12), Some(ViewRow::Line { number: 34, matched: true, pinned: false }));
        assert_eq!(v.row(13), None);
        assert_eq!(v.row_for_line(15), Some(4));
        assert_eq!(v.row_for_line(8), None);
        assert_eq!((v.context_lines(), v.pinned_count(), v.pinned_unmatched()), (0, 1, 1));
    }

    #[test]
    fn reference_with_context_one_and_three() {
        let v1 = reference(1);
        assert_eq!(**v1.shown_lines(), vec![7, 8, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 27, 28, 29, 33, 34, 35]);
        assert_eq!(v1.context_lines(), 13);
        assert!(matches!(v1.row(1), Some(ViewRow::Line { number: 8, matched: false, pinned: false })));
        assert!(v1.row(1).unwrap().is_context());
        let v3 = reference(3);
        assert_eq!(v3.shown_lines().len(), 30);
        assert_eq!(&v3.shown_lines()[..3], &[4, 7, 8]);
        assert_eq!(v3.context_lines(), 23);
    }

    #[test]
    fn shares_matches_when_nothing_is_added() {
        let matches = Arc::new(vec![1, 2, 9]);
        let v = FilteredView::build(matches.clone(), &BTreeSet::new(), 0, 10, |_| false);
        assert!(Arc::ptr_eq(v.shown_lines(), &matches));
    }

    #[test]
    fn no_context_no_pins_has_zero_context_lines_and_shares_matches() {
        let matches = Arc::new((0..2050u64).collect::<Vec<_>>());
        let v = FilteredView::build(matches.clone(), &BTreeSet::new(), 0, 2050, |_| false);
        assert_eq!(v.context_lines(), 0);
        assert!(Arc::ptr_eq(v.shown_lines(), &matches));
    }

    #[test]
    fn edges_and_empty_inputs() {
        let empty = FilteredView::build(Arc::new(vec![]), &BTreeSet::new(), 3, 0, |_| false);
        assert_eq!(empty.row_count(), 0);
        assert_eq!(empty.row(0), None);
        let edges = FilteredView::build(Arc::new(vec![0, 9]), &BTreeSet::from([42]), 2, 10, |_| false);
        assert_eq!(**edges.shown_lines(), vec![0, 1, 2, 7, 8, 9], "pins beyond the file are ignored; windows clamp");
        assert_eq!(edges.pinned_count(), 0);
        assert_eq!(edges.row(3), Some(ViewRow::Gap { omitted: 4 }));
    }

    #[test]
    fn rows_resolve_across_checkpoints() {
        let matches: Vec<u64> = (0..5_000).map(|i| i * 3).collect();
        let v = FilteredView::build(Arc::new(matches.clone()), &BTreeSet::new(), 0, 15_000, |_| false);
        assert_eq!(v.row_count(), 5_000 + 4_999);
        for (k, &m) in matches.iter().enumerate().step_by(997) {
            let r = v.row_for_line(m).unwrap();
            assert_eq!(r, (k as u64) * 2);
            assert_eq!(line(v.row(r)), m);
        }
    }

    #[test]
    fn anchors_are_matches_and_pins() {
        let v = reference(1);
        assert!(v.is_anchor(12) && v.is_anchor(7));
        assert!(!v.is_anchor(8), "a context line");
    }
}
