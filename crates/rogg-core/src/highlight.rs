//! Highlight spans for painted rows (spec §4.8): Include terms over the display string.

use crate::error::CoreError;
use crate::filter::{FilterSpec, TermState};
use aho_corasick::{AhoCorasick, MatchKind};
use std::ops::Range;

/// At most this many matches per term are considered per line (bounds work on hostile lines).
pub const MAX_SPANS_PER_TERM: usize = 4096;

enum Finder {
    Plain(AhoCorasick),
    Regex(regex::bytes::Regex),
}

impl Finder {
    fn build(text: &str, regex: bool, case_sensitive: bool) -> Result<Self, CoreError> {
        if regex {
            regex::bytes::RegexBuilder::new(text)
                .case_insensitive(!case_sensitive)
                .build()
                .map(Finder::Regex)
                .map_err(|e| CoreError::InvalidRegex { term: text.to_string(), message: e.to_string() })
        } else {
            AhoCorasick::builder()
                .ascii_case_insensitive(!case_sensitive)
                .match_kind(MatchKind::LeftmostFirst)
                .build([text])
                .map(Finder::Plain)
                .map_err(|e| CoreError::InvalidRegex { term: text.to_string(), message: e.to_string() })
        }
    }

    fn ranges<'a>(&'a self, display: &'a str) -> Box<dyn Iterator<Item = Range<usize>> + 'a> {
        match self {
            Finder::Plain(ac) => Box::new(ac.find_iter(display).map(|m| m.range())),
            Finder::Regex(re) => Box::new(re.find_iter(display.as_bytes()).map(|m| m.range())),
        }
    }
}

/// Widens `range` to the nearest char boundaries (byte regexes such as `(?-u)\xA9` can match mid-char).
fn snap_to_char_boundaries(display: &str, range: Range<usize>) -> Range<usize> {
    let mut start = range.start;
    while !display.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = range.end.min(display.len());
    while !display.is_char_boundary(end) {
        end += 1;
    }
    start..end
}

pub struct Highlighter {
    finders: Vec<(usize, Finder)>,
}

impl Highlighter {
    /// Highlighter for the Include terms of `spec`, keeping each term's index for its color slot.
    pub fn new(spec: &FilterSpec) -> Result<Self, CoreError> {
        let mut finders = Vec::new();
        for (i, term) in spec.terms.iter().enumerate() {
            if term.state == TermState::Include && !term.text.is_empty() {
                finders.push((i, Finder::build(&term.text, spec.regex, spec.case_sensitive)?));
            }
        }
        Ok(Self { finders })
    }

    pub(crate) fn single(text: &str, regex: bool, case_sensitive: bool) -> Result<Self, CoreError> {
        if text.is_empty() {
            return Ok(Self { finders: Vec::new() });
        }
        Ok(Self { finders: vec![(0, Finder::build(text, regex, case_sensitive)?)] })
    }

    /// Byte ranges of `display` to highlight with their palette slot; earlier terms win on overlap.
    pub fn spans(&self, display: &str, palette_len: usize) -> Vec<(Range<usize>, usize)> {
        if self.finders.is_empty() || display.is_empty() {
            return Vec::new();
        }
        let mut owner = vec![u32::MAX; display.len()];
        for (term, finder) in &self.finders {
            for range in finder.ranges(display).filter(|r| !r.is_empty()).take(MAX_SPANS_PER_TERM) {
                let range = snap_to_char_boundaries(display, range);
                for slot in &mut owner[range] {
                    if *slot == u32::MAX {
                        *slot = *term as u32;
                    }
                }
            }
        }
        let palette_len = palette_len.max(1);
        let mut out = Vec::new();
        let mut i = 0;
        while i < owner.len() {
            let term = owner[i];
            if term == u32::MAX {
                i += 1;
                continue;
            }
            let start = i;
            while i < owner.len() && owner[i] == term {
                i += 1;
            }
            out.push((start..i, term as usize % palette_len));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};

    fn spec(terms: &[(&str, TermState)], regex: bool) -> FilterSpec {
        FilterSpec { terms: terms.iter().map(|(t, s)| Term::new(*t, *s)).collect(), regex, ..Default::default() }
    }

    #[test]
    fn earlier_term_wins_and_slots_follow_term_index() {
        let h = Highlighter::new(&spec(&[("timeout", TermState::Include), ("out", TermState::Include)], false)).unwrap();
        assert_eq!(h.spans("a timeout, out", 4), vec![(2..9, 0), (11..14, 1)]);
        let many: Vec<(String, TermState)> = (0..6).map(|i| (format!("t{i}"), TermState::Include)).collect();
        let refs: Vec<(&str, TermState)> = many.iter().map(|(t, s)| (t.as_str(), *s)).collect();
        let h = Highlighter::new(&spec(&refs, false)).unwrap();
        assert_eq!(h.spans("t5", 4), vec![(0..2, 1)], "slot = term index 5 % 4");
    }

    #[test]
    fn only_include_terms_are_highlighted_case_insensitively() {
        let h = Highlighter::new(&spec(&[("error", TermState::Include), ("ok", TermState::Exclude), ("x", TermState::Off)], false)).unwrap();
        assert_eq!(h.spans("ERROR ok x error", 4), vec![(0..5, 0), (11..16, 0)]);
    }

    #[test]
    fn spans_land_on_char_boundaries_for_lossy_text() {
        let display = String::from_utf8_lossy(b"\xFF\xFEtimeout \xC3\xA9 t\xFFimeout").into_owned();
        let h = Highlighter::new(&spec(&[("timeout", TermState::Include), ("é", TermState::Include)], false)).unwrap();
        let spans = h.spans(&display, 4);
        assert!(!spans.is_empty());
        for (r, _) in &spans {
            assert!(display.is_char_boundary(r.start) && display.is_char_boundary(r.end), "{r:?} in {display:?}");
        }
        // Display is U+FFFD U+FFFD "timeout " "é" " t" U+FFFD "imeout": the ranges are exact byte offsets.
        assert_eq!(spans, vec![(6..13, 0), (14..16, 1)]);
    }

    #[test]
    fn empty_regex_matches_are_skipped() {
        let h = Highlighter::new(&spec(&[("x*", TermState::Include)], true)).unwrap();
        assert_eq!(h.spans("axxb", 4), vec![(1..3, 0)]);
        assert!(Highlighter::new(&spec(&[("(bad", TermState::Include)], true)).is_err());
    }

    #[test]
    fn spans_per_term_are_capped() {
        let h = Highlighter::new(&spec(&[("a", TermState::Include)], false)).unwrap();
        let line = "a ".repeat(10_000);
        let covered: usize = h.spans(&line, 4).iter().map(|(r, _)| r.len()).sum();
        assert_eq!(covered, MAX_SPANS_PER_TERM);
    }

    #[test]
    fn byte_regexes_compile_and_snap_to_char_boundaries() {
        assert!(Highlighter::new(&spec(&[(r"(?-u)\xFF", TermState::Include)], true)).is_ok());
        let h = Highlighter::new(&spec(&[(r"(?-u)\xA9", TermState::Include)], true)).unwrap();
        assert_eq!(h.spans("é", 4), vec![(0..2, 0)]);
        assert_eq!(h.spans("aéb", 4), vec![(1..3, 0)]);
    }
}
