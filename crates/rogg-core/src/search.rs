//! Per-panel search (spec §4.10): a query independent of the filters, over the whole text or the shown rows.

use crate::cancel::CancelToken;
use crate::engine::{chunk_ranges, run_filter};
use crate::error::CoreError;
use crate::filter::{CompiledFilter, FilterSpec, Term};
use crate::highlight::Highlighter;
use crate::text::LogText;
use rayon::prelude::*;
use std::ops::Range;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct SearchQuery {
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
}

#[derive(Clone, Debug)]
pub enum SearchScope {
    All,
    /// The shown lines (ascending, as `FilteredView::shown_lines` yields them).
    Lines(Arc<Vec<u64>>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    /// Lines containing at least one hit, ascending.
    pub lines: Vec<u64>,
}

pub struct CompiledSearch {
    filter: CompiledFilter,
    highlighter: Highlighter,
}

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn compile(&self) -> Result<CompiledSearch, CoreError> {
        let terms = if self.is_empty() { Vec::new() } else { vec![Term::include(self.text.clone())] };
        let spec = FilterSpec { terms, regex: self.regex, case_sensitive: self.case_sensitive, context: 0 };
        Ok(CompiledSearch {
            filter: CompiledFilter::compile(&spec)?,
            highlighter: Highlighter::single(&self.text, self.regex, self.case_sensitive)?,
        })
    }
}

impl CompiledSearch {
    pub fn run(&self, text: &LogText, scope: &SearchScope, chunk_lines: u64, cancel: &CancelToken) -> Result<SearchResult, CoreError> {
        match scope {
            SearchScope::All => run_filter(text, &self.filter, chunk_lines, cancel).map(|r| SearchResult { lines: r.matches }),
            SearchScope::Lines(lines) => self.run_lines(text, lines, chunk_lines, cancel),
        }
    }

    fn run_lines(&self, text: &LogText, lines: &[u64], chunk_lines: u64, cancel: &CancelToken) -> Result<SearchResult, CoreError> {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if lines.is_empty() {
            return Ok(SearchResult::default());
        }
        let sorted;
        let lines = if lines.is_sorted_by(|a, b| a < b) {
            lines
        } else {
            sorted = {
                let mut v = lines.to_vec();
                v.sort_unstable();
                v.dedup();
                v
            };
            &sorted
        };
        // Chunk by line number with the engine's helper (capped by lines and bytes, so long-line
        // files keep their cancel latency); each chunk takes the shown lines that fall inside it.
        // Numbers past the end of the text are not lines: they are never visited (and bound the chunk count).
        let span = lines[0]..lines[lines.len() - 1].saturating_add(1).min(text.line_count());
        let ranges = chunk_ranges(text, span, chunk_lines.max(1));
        let parts: Vec<Option<Vec<u64>>> = ranges
            .par_iter()
            .map(|r| {
                if cancel.is_cancelled() {
                    return None;
                }
                let lo = lines.partition_point(|&n| n < r.start);
                let hi = lines.partition_point(|&n| n < r.end);
                Some(lines[lo..hi].iter().copied().filter(|&n| self.filter.line_matches(text.line(n))).collect())
            })
            .collect();
        if cancel.is_cancelled() || parts.iter().any(Option::is_none) {
            return Err(CoreError::Cancelled);
        }
        Ok(SearchResult { lines: parts.into_iter().flatten().flatten().collect() })
    }

    /// Hit ranges on a display string, for the accent outline.
    pub fn spans(&self, display: &str) -> Vec<Range<usize>> {
        self.highlighter.spans(display, 1).into_iter().map(|(r, _)| r).collect()
    }
}

impl SearchResult {
    pub fn count(&self) -> usize {
        self.lines.len()
    }

    /// The first hit after `line` (wrapping); the first hit when `line` is None.
    pub fn next_after(&self, line: Option<u64>) -> Option<u64> {
        let first = *self.lines.first()?;
        match line {
            None => Some(first),
            Some(l) => Some(self.lines.get(self.lines.partition_point(|&x| x <= l)).copied().unwrap_or(first)),
        }
    }

    /// The last hit before `line` (wrapping); the last hit when `line` is None.
    pub fn prev_before(&self, line: Option<u64>) -> Option<u64> {
        let last = *self.lines.last()?;
        match line {
            None => Some(last),
            Some(l) => {
                let i = self.lines.partition_point(|&x| x < l);
                Some(if i == 0 { last } else { self.lines[i - 1] })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::LogText;

    fn text() -> LogText {
        LogText::from_bytes(b"alpha timeout\nbeta\ngamma TIMEOUT\ndelta\ntimeout again\n".to_vec())
    }

    #[test]
    fn counts_lines_in_whole_text_and_in_shown_lines() {
        let q = SearchQuery { text: "timeout".into(), ..Default::default() };
        let c = q.compile().unwrap();
        let all = c.run(&text(), &SearchScope::All, 2, &CancelToken::new()).unwrap();
        assert_eq!(all.lines, vec![0, 2, 4]);
        assert_eq!(all.count(), 3);
        let shown = c.run(&text(), &SearchScope::Lines(Arc::new(vec![1, 2, 3])), 2, &CancelToken::new()).unwrap();
        assert_eq!(shown.lines, vec![2]);
    }

    #[test]
    fn regex_case_and_errors() {
        let q = SearchQuery { text: "^[a-z]+ timeout$".into(), regex: true, case_sensitive: true };
        assert_eq!(q.compile().unwrap().run(&text(), &SearchScope::All, 8, &CancelToken::new()).unwrap().lines, vec![0]);
        assert!(SearchQuery { text: "(".into(), regex: true, ..Default::default() }.compile().is_err());
    }

    #[test]
    fn next_and_prev_wrap() {
        let r = SearchResult { lines: vec![3, 8, 20] };
        assert_eq!(r.next_after(None), Some(3));
        assert_eq!(r.next_after(Some(8)), Some(20));
        assert_eq!(r.next_after(Some(20)), Some(3));
        assert_eq!(r.prev_before(None), Some(20));
        assert_eq!(r.prev_before(Some(8)), Some(3));
        assert_eq!(r.prev_before(Some(3)), Some(20));
        assert_eq!(SearchResult::default().next_after(Some(1)), None);
    }

    #[test]
    fn empty_query_finds_nothing_and_cancel_errors() {
        let empty = SearchQuery::default().compile().unwrap();
        assert!(empty.run(&text(), &SearchScope::All, 8, &CancelToken::new()).unwrap().lines.is_empty());
        assert!(empty.spans("anything").is_empty());
        let cancel = CancelToken::new();
        cancel.cancel();
        let q = SearchQuery { text: "a".into(), ..Default::default() }.compile().unwrap();
        assert!(q.run(&text(), &SearchScope::All, 8, &cancel).is_err());
        assert!(q.run(&text(), &SearchScope::Lines(Arc::new(vec![0])), 8, &cancel).is_err());
    }

    #[test]
    fn shown_lines_scope_is_chunked_by_line_number_and_robust_to_input() {
        let body: Vec<String> = (0..50).map(|i| if i % 5 == 0 { format!("hit {i}") } else { format!("miss {i}") }).collect();
        let t = LogText::from_bytes(body.join("\n").into_bytes());
        let c = SearchQuery { text: "hit".into(), ..Default::default() }.compile().unwrap();
        // Sparse, with a small chunk size so several chunks are empty.
        let shown = Arc::new(vec![0, 7, 10, 11, 35, 45, 49]);
        let r = c.run(&t, &SearchScope::Lines(shown), 3, &CancelToken::new()).unwrap();
        assert_eq!(r.lines, vec![0, 10, 35, 45]);
        // Unsorted with duplicates still gives ascending, unique hits.
        let r = c.run(&t, &SearchScope::Lines(Arc::new(vec![45, 10, 10, 0, 7])), 3, &CancelToken::new()).unwrap();
        assert_eq!(r.lines, vec![0, 10, 45]);
        assert!(c.run(&t, &SearchScope::Lines(Arc::new(Vec::new())), 3, &CancelToken::new()).unwrap().lines.is_empty());
    }

    #[test]
    fn numbers_past_the_end_are_not_lines() {
        let empty_line = SearchQuery { text: "^$".into(), regex: true, ..Default::default() }.compile().unwrap();
        let scope = SearchScope::Lines(Arc::new(vec![1, 99, u64::MAX]));
        let r = empty_line.run(&text(), &scope, 2, &CancelToken::new()).unwrap();
        assert!(r.lines.is_empty(), "{:?}", r.lines);
    }

    #[test]
    fn spans_mark_every_hit() {
        let q = SearchQuery { text: "time".into(), ..Default::default() }.compile().unwrap();
        assert_eq!(q.spans("Time to timeout"), vec![0..4, 8..12]);
    }

    #[test]
    fn byte_regex_compiles_for_search() {
        let q = SearchQuery { text: r"(?-u)\xA9".into(), regex: true, ..Default::default() }.compile().unwrap();
        assert_eq!(q.spans("é"), vec![0..2]);
        assert!(SearchQuery { text: r"(?-u)\xFF".into(), regex: true, ..Default::default() }.compile().is_ok());
    }

    #[test]
    fn wraparound_edges() {
        let one = SearchResult { lines: vec![5] };
        assert_eq!(one.next_after(None), Some(5));
        assert_eq!(one.prev_before(None), Some(5));
        assert_eq!(one.next_after(Some(5)), Some(5));
        assert_eq!(one.prev_before(Some(5)), Some(5));
        let none = SearchResult::default();
        assert_eq!(none.next_after(None), None);
        assert_eq!(none.prev_before(None), None);
    }
}
