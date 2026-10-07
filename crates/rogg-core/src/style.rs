//! Applying a highlighting profile to a painted row (spec §6.1). Rules run top to bottom over the row's display text;
//! for each effect the first matching enabled rule wins: one rule colours the line, one tints the row, and where match
//! colours overlap the earlier rule keeps the bytes. Only painted rows are styled, so the cost is per visible line.

use crate::highlight::{Finder, MAX_SPANS_PER_TERM, snap_to_char_boundaries};
use crate::profile::{LineRule, Profile};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleError {
    pub rule: usize,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineStyle {
    /// The rule whose colour paints the whole line.
    pub line_color: Option<usize>,
    /// The rule whose colour tints the row.
    pub tint: Option<usize>,
    /// Byte ranges painted in a rule's colour (ascending, non-overlapping).
    pub match_spans: Vec<(Range<usize>, usize)>,
    /// Byte ranges drawn bold (ascending, non-overlapping).
    pub bold: Vec<Range<usize>>,
}

struct Compiled {
    index: usize,
    rule: LineRule,
    finder: Finder,
}

pub struct CompiledProfile {
    rules: Vec<Compiled>,
}

fn runs<T: Copy + PartialEq>(owner: &[T], none: T) -> Vec<(Range<usize>, T)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < owner.len() {
        let value = owner[i];
        let start = i;
        while i < owner.len() && owner[i] == value {
            i += 1;
        }
        if value != none {
            out.push((start..i, value));
        }
    }
    out
}

impl CompiledProfile {
    /// Compiles every enabled rule with a pattern; rules that do not compile are skipped and reported.
    pub fn compile(profile: &Profile) -> (Self, Vec<RuleError>) {
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for (index, rule) in profile.rules.iter().enumerate() {
            if !rule.enabled || rule.pattern.is_empty() {
                continue;
            }
            match Finder::build(&rule.pattern, rule.regex, rule.case_sensitive) {
                Ok(finder) => rules.push(Compiled { index, rule: rule.clone(), finder }),
                Err(e) => errors.push(RuleError { rule: index, message: e.to_string() }),
            }
        }
        (Self { rules }, errors)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The style of one row's display text in the full (`filtered == false`) or the filtered log.
    pub fn style(&self, display: &str, filtered: bool) -> LineStyle {
        let mut style = LineStyle::default();
        if display.is_empty() {
            return style;
        }
        let mut color_owner: Option<Vec<u32>> = None;
        let mut bold_owner: Option<Vec<bool>> = None;
        for c in &self.rules {
            let r = &c.rule;
            if !r.panes.includes(filtered) {
                continue;
            }
            let wants_line = (r.line_color && style.line_color.is_none()) || (r.row_tint && style.tint.is_none());
            if !(wants_line || r.match_color || r.bold) || !c.finder.is_match(display) {
                continue;
            }
            if r.line_color && style.line_color.is_none() {
                style.line_color = Some(c.index);
            }
            if r.row_tint && style.tint.is_none() {
                style.tint = Some(c.index);
            }
            if r.match_color || r.bold {
                for range in c.finder.ranges(display).filter(|m| !m.is_empty()).take(MAX_SPANS_PER_TERM) {
                    let range = snap_to_char_boundaries(display, range);
                    if r.match_color {
                        let owner = color_owner.get_or_insert_with(|| vec![u32::MAX; display.len()]);
                        for slot in &mut owner[range.clone()] {
                            if *slot == u32::MAX {
                                *slot = c.index as u32;
                            }
                        }
                    }
                    if r.bold {
                        bold_owner.get_or_insert_with(|| vec![false; display.len()])[range].fill(true);
                    }
                }
            }
        }
        if let Some(owner) = color_owner {
            style.match_spans = runs(&owner, u32::MAX).into_iter().map(|(r, i)| (r, i as usize)).collect();
        }
        if let Some(owner) = bold_owner {
            style.bold = runs(&owner, false).into_iter().map(|(r, _)| r).collect();
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{LineRule, Panes, Profile, builtin_default};

    fn profile(rules: Vec<LineRule>) -> Profile {
        Profile { name: "t".into(), rules, hit_palette: Vec::new() }
    }

    fn rule(pattern: &str, f: impl FnOnce(&mut LineRule)) -> LineRule {
        let mut r = LineRule::new(pattern);
        f(&mut r);
        r
    }

    #[test]
    fn the_first_matching_rule_wins_for_each_effect() {
        let p = profile(vec![
            rule("disk", |r| {
                r.match_color = false;
                r.line_color = true;
            }),
            rule("full", |r| {
                r.match_color = false;
                r.line_color = true;
                r.row_tint = true;
            }),
        ]);
        let (c, errors) = CompiledProfile::compile(&p);
        assert!(errors.is_empty());
        let s = c.style("disk full", false);
        assert_eq!((s.line_color, s.tint), (Some(0), Some(1)), "line colour from rule 0, tint from the first rule with a tint");
        assert_eq!(c.style("nothing", false), LineStyle::default());
    }

    #[test]
    fn match_spans_prefer_earlier_rules_and_bold_is_separate() {
        let p = profile(vec![rule("timeout", |_| {}), rule("out", |r| r.bold = true), rule("time", |r| r.match_color = false)]);
        let (c, _) = CompiledProfile::compile(&p);
        let s = c.style("a timeout, out", false);
        assert_eq!(s.match_spans, vec![(2..9, 0), (11..14, 1)]);
        assert_eq!(s.bold, vec![6..9, 11..14]);
    }

    #[test]
    fn panes_disabled_and_empty_rules_are_respected() {
        let p = profile(vec![
            rule("a", |r| r.panes = Panes::Filtered),
            rule("b", |r| r.enabled = false),
            rule("", |_| {}),
            rule("c", |r| r.panes = Panes::Full),
        ]);
        let (c, _) = CompiledProfile::compile(&p);
        assert_eq!(c.style("abc", true).match_spans, vec![(0..1, 0)]);
        assert_eq!(c.style("abc", false).match_spans, vec![(2..3, 3)]);
    }

    #[test]
    fn invalid_rules_are_skipped_and_reported() {
        let p = profile(vec![rule("(oops", |r| r.regex = true), rule("ok", |_| {})]);
        let (c, errors) = CompiledProfile::compile(&p);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].rule, 0);
        assert!(errors[0].message.contains("oops"), "{}", errors[0].message);
        assert_eq!(c.style("ok (oops", false).match_spans, vec![(0..2, 1)]);
    }

    #[test]
    fn case_and_regex_flags_apply() {
        let p = profile(vec![rule("error", |r| r.case_sensitive = true), rule(r"\d+ms", |r| r.regex = true)]);
        let (c, _) = CompiledProfile::compile(&p);
        assert_eq!(c.style("ERROR error 250ms", false).match_spans, vec![(6..11, 0), (12..17, 1)]);
    }

    #[test]
    fn the_default_profile_styles_a_reference_line() {
        let (c, errors) = CompiledProfile::compile(&builtin_default());
        assert!(errors.is_empty());
        let s = c.style("2026-10-05 14:32:08.142  ERROR  client       upstream timeout", false);
        assert_eq!(s.match_spans, vec![(0..23, 0)], "the timestamp");
        assert_eq!((s.line_color, s.tint), (Some(1), Some(1)), "ERROR colours the line and tints the row");
        let warn = c.style("2026-10-05 14:32:08.142  WARN   client  retry", false);
        assert_eq!((warn.line_color, warn.tint), (Some(2), None));
        assert_eq!(c.style("ERRORS were few", false).line_color, None, "whole words only");
    }

    #[test]
    fn spans_stay_on_char_boundaries_and_are_capped() {
        let p = profile(vec![rule("é", |_| {}), rule(r"(?-u)\xA9", |r| r.regex = true)]);
        let (c, _) = CompiledProfile::compile(&p);
        for (r, _) in c.style("aéb", false).match_spans {
            assert!("aéb".is_char_boundary(r.start) && "aéb".is_char_boundary(r.end));
        }
        let line = "a ".repeat(10_000);
        let (c, _) = CompiledProfile::compile(&profile(vec![rule("a", |_| {})]));
        let covered: usize = c.style(&line, false).match_spans.iter().map(|(r, _)| r.len()).sum();
        assert_eq!(covered, crate::highlight::MAX_SPANS_PER_TERM);
    }
}
