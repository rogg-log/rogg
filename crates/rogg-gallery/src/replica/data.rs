//! Port of the reference mockup's sample data and filter logic (`samples`, `lineText`, `passes`, `spans`).
use rogg_ui::filter::ChipState;
use std::collections::BTreeSet;
use std::ops::Range;

pub use rogg_app::demo::{FIRST_LINE_NUMBER, SAMPLES, line_text};
/// `text.slice(0, 23)`: the timestamp painted faint.
pub const STAMP_LEN: usize = 23;

#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub text: String,
    pub state: ChipState,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filtered {
    pub matched: Vec<usize>,
    pub visible: Vec<usize>,
    pub anchors: BTreeSet<usize>,
}

pub fn reference_terms() -> Vec<Term> {
    [("error", ChipState::Include), ("timeout", ChipState::Include), ("healthcheck", ChipState::Exclude), ("retry", ChipState::Off)]
        .map(|(text, state)| Term { text: text.into(), state })
        .into()
}

fn contains(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive { haystack.contains(needle) } else { haystack.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) }
}

fn excluded(i: usize, terms: &[Term], case: bool) -> bool {
    let text = line_text(i);
    terms.iter().any(|t| t.state == ChipState::Exclude && contains(&text, &t.text, case))
}

fn passes(i: usize, terms: &[Term], case: bool) -> bool {
    let text = line_text(i);
    let includes: Vec<_> = terms.iter().filter(|t| t.state == ChipState::Include).collect();
    let has_excludes = terms.iter().any(|t| t.state == ChipState::Exclude);
    let included = if includes.is_empty() { has_excludes } else { includes.iter().any(|t| contains(&text, &t.text, case)) };
    included && !excluded(i, terms, case)
}

/// Mirrors the reference `renderBottom`: matches ∪ pins, plus ±context lines that are not excluded (or are pinned).
pub fn filter(terms: &[Term], pins: &BTreeSet<usize>, context: usize, case: bool) -> Filtered {
    let matched: Vec<usize> = (0..SAMPLES.len()).filter(|&i| passes(i, terms, case)).collect();
    let anchors: BTreeSet<usize> = matched.iter().copied().chain(pins.iter().copied()).collect();
    let mut shown = anchors.clone();
    for &i in &anchors {
        let lo = i.saturating_sub(context);
        let hi = (i + context).min(SAMPLES.len() - 1);
        for j in lo..=hi {
            if !excluded(j, terms, case) || pins.contains(&j) {
                shown.insert(j);
            }
        }
    }
    Filtered { matched, visible: shown.into_iter().collect(), anchors }
}

/// Include-term highlight ranges over `rest` (text after the timestamp). Earlier terms own overlapping bytes;
/// the slot is the term's index in the full list, as in the reference (`hit-(index % 4)`).
pub fn hit_spans(rest: &str, terms: &[Term], case: bool) -> Vec<(Range<usize>, usize)> {
    let hay = if case { rest.to_string() } else { rest.to_ascii_lowercase() };
    let mut owner: Vec<Option<usize>> = vec![None; rest.len()];
    for (index, term) in terms.iter().enumerate() {
        if term.state != ChipState::Include || term.text.is_empty() {
            continue;
        }
        let needle = if case { term.text.clone() } else { term.text.to_ascii_lowercase() };
        let mut from = 0;
        while let Some(at) = hay[from..].find(&needle) {
            let start = from + at;
            for slot in &mut owner[start..start + needle.len()] {
                slot.get_or_insert(index);
            }
            from = start + needle.len();
        }
    }
    let mut out: Vec<(Range<usize>, usize)> = Vec::new();
    for (i, o) in owner.iter().enumerate() {
        let Some(term) = *o else { continue };
        match out.last_mut() {
            Some((r, t)) if r.end == i && *t == term % 4 => r.end = i + 1,
            _ => out.push((i..i + 1, term % 4)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_text_matches_reference_formula() {
        assert_eq!(line_text(0), "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29");
        assert_eq!(
            line_text(7),
            "2026-10-05 14:32:10.639  INFO   config       deployment revision=8d72f6a feature_flags={payments_v2:true, strict_idempotency:true} changed_by=release-controller region=eu-west-1"
        );
        assert!(line_text(12).starts_with("2026-10-05 14:32:12.144  ERROR  client       upstream timeout after 30000ms"));
        assert_eq!(line_text(39), "2026-10-05 14:32:21.361  INFO   server       idle connections pruned count=8 remaining=16");
    }

    #[test]
    fn reference_filter_results() {
        let pins = BTreeSet::from([7]);
        let f = filter(&reference_terms(), &pins, 0, false);
        assert_eq!(f.matched, vec![12, 15, 20, 23, 28, 34]);
        assert_eq!(f.visible, vec![7, 12, 15, 20, 23, 28, 34]);
        let f1 = filter(&reference_terms(), &pins, 1, false);
        assert_eq!(f1.visible, vec![7, 8, 11, 12, 13, 14, 15, 16, 19, 20, 21, 22, 23, 24, 27, 28, 29, 33, 34, 35]);
        let f3 = filter(&reference_terms(), &pins, 3, false);
        assert_eq!(f3.visible.len(), 30);
        assert_eq!(&f3.visible[..3], &[4, 7, 8]);
    }

    #[test]
    fn only_excludes_shows_everything_else_and_no_active_terms_shows_nothing() {
        let only_not = vec![Term { text: "healthcheck".into(), state: ChipState::Exclude }];
        let f = filter(&only_not, &BTreeSet::new(), 0, false);
        assert_eq!(f.matched.len(), 40 - 4); // healthcheck appears on lines 5, 6, 18, 26
        let all_off = vec![Term { text: "error".into(), state: ChipState::Off }];
        assert!(filter(&all_off, &BTreeSet::new(), 0, false).matched.is_empty());
    }

    #[test]
    fn hit_spans_earlier_term_wins_and_slots_follow_term_index() {
        let rest = &line_text(12)[STAMP_LEN..];
        assert_eq!(hit_spans(rest, &reference_terms(), false), vec![(2..7, 0), (31..38, 1)]);
        let overlapping = vec![Term { text: "timeout".into(), state: ChipState::Include }, Term { text: "out".into(), state: ChipState::Include }];
        assert_eq!(hit_spans("a timeout", &overlapping, false), vec![(2..9, 0)]);
        assert!(hit_spans("ERROR", &reference_terms(), true).is_empty());
    }
}
