//! Filter statistics (spec §4.11).

use crate::engine::FilterResult;
use crate::filter::{FilterSpec, TermState};
use crate::view::FilteredView;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermHits {
    pub text: String,
    pub state: TermState,
    pub lines: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterStats {
    pub total_lines: u64,
    pub matched: u64,
    pub pinned: u64,
    pub pinned_unmatched: u64,
    pub context_lines: u64,
    pub term_hits: Vec<TermHits>,
    pub duration: Duration,
}

/// `1204` → `"1,204"`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl FilterStats {
    pub fn compute(spec: &FilterSpec, result: &FilterResult, view: &FilteredView) -> Self {
        let term_hits = spec
            .terms
            .iter()
            .enumerate()
            .map(|(i, t)| TermHits { text: t.text.clone(), state: t.state, lines: result.term_hits.get(i).copied().unwrap_or(0) })
            .collect();
        FilterStats {
            total_lines: result.total_lines,
            matched: result.matches.len() as u64,
            pinned: view.pinned_count(),
            pinned_unmatched: view.pinned_unmatched(),
            context_lines: view.context_lines(),
            term_hits,
            duration: result.duration,
        }
    }

    /// Footer text: `6 matches / 40 lines · 1 pinned (1 added) · 0 context`.
    pub fn summary(&self) -> String {
        let added = if self.pinned_unmatched > 0 { format!(" ({} added)", group_digits(self.pinned_unmatched)) } else { String::new() };
        format!(
            "{} matches / {} lines · {} pinned{} · {} context",
            group_digits(self.matched),
            group_digits(self.total_lines),
            group_digits(self.pinned),
            added,
            group_digits(self.context_lines)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::FilterResult;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::view::FilteredView;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    #[test]
    fn computes_reference_numbers_and_summary() {
        let spec = FilterSpec {
            terms: vec![Term::include("error"), Term::include("timeout"), Term::new("healthcheck", TermState::Exclude), Term::new("retry", TermState::Off)],
            ..Default::default()
        };
        let result = FilterResult { matches: vec![12, 15, 20, 23, 28, 34], term_hits: vec![7, 7, 4, 0], total_lines: 40, duration: Duration::from_millis(3) };
        let view = FilteredView::build(Arc::new(result.matches.clone()), &BTreeSet::from([7]), 0, 40, |_| false);
        let stats = FilterStats::compute(&spec, &result, &view);
        assert_eq!((stats.matched, stats.total_lines, stats.pinned, stats.pinned_unmatched, stats.context_lines), (6, 40, 1, 1, 0));
        assert_eq!(stats.term_hits[2], TermHits { text: "healthcheck".into(), state: TermState::Exclude, lines: 4 });
        assert_eq!(stats.summary(), "6 matches / 40 lines · 1 pinned (1 added) · 0 context");
    }

    #[test]
    fn summary_omits_added_and_groups_digits() {
        let stats = FilterStats { total_lines: 10_000_000, matched: 1_204, pinned: 0, pinned_unmatched: 0, context_lines: 12, term_hits: vec![], duration: Duration::ZERO };
        assert_eq!(stats.summary(), "1,204 matches / 10,000,000 lines · 0 pinned · 12 context");
        assert_eq!([group_digits(0), group_digits(999), group_digits(1000), group_digits(1234567)], ["0", "999", "1,000", "1,234,567"]);
    }
}
