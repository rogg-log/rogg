use super::spec::{FilterSpec, TermState};
use crate::error::CoreError;
use crate::text::LogText;
use aho_corasick::{AhoCorasick, MatchKind};
use regex::bytes::{RegexBuilder, RegexSet, RegexSetBuilder};
use std::ops::Range;

/// Lines matched in a scanned range plus per-term line hit counts (indexed like `FilterSpec::terms`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub matches: Vec<u64>,
    pub term_hits: Vec<u64>,
}

enum TermSet {
    Empty,
    Plain { ac: AhoCorasick, terms: Vec<usize> },
    Regex { set: RegexSet, terms: Vec<usize> },
}

impl TermSet {
    fn build(spec: &FilterSpec, state: TermState) -> Result<Self, CoreError> {
        let terms: Vec<usize> =
            spec.terms.iter().enumerate().filter(|(_, t)| t.state == state).map(|(i, _)| i).collect();
        if terms.is_empty() {
            return Ok(TermSet::Empty);
        }
        let patterns: Vec<&str> = terms.iter().map(|&i| spec.terms[i].text.as_str()).collect();
        if spec.regex {
            for pattern in &patterns {
                RegexBuilder::new(pattern)
                    .case_insensitive(!spec.case_sensitive)
                    .build()
                    .map_err(|e| CoreError::InvalidRegex { term: pattern.to_string(), message: e.to_string() })?;
            }
            let set = RegexSetBuilder::new(&patterns)
                .case_insensitive(!spec.case_sensitive)
                .build()
                .map_err(|e| CoreError::InvalidRegex { term: patterns.join(" || "), message: e.to_string() })?;
            Ok(TermSet::Regex { set, terms })
        } else {
            let ac = AhoCorasick::builder()
                .ascii_case_insensitive(!spec.case_sensitive)
                .match_kind(MatchKind::Standard)
                .build(&patterns)
                .map_err(|e| CoreError::InvalidRegex { term: patterns.join(" || "), message: e.to_string() })?;
            Ok(TermSet::Plain { ac, terms })
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, TermSet::Empty)
    }

    fn is_match(&self, line: &[u8]) -> bool {
        match self {
            TermSet::Empty => false,
            TermSet::Plain { ac, .. } => ac.is_match(line),
            TermSet::Regex { set, .. } => set.is_match(line),
        }
    }

    /// Sets `hit[k]` for every line `lines.start + k` containing a term and counts per-term line hits.
    fn mark(&self, text: &LogText, lines: &Range<u64>, starts: &[u64], hit: &mut [bool], hits: &mut [u64], last: &mut [u64]) {
        match self {
            TermSet::Empty => {}
            TermSet::Plain { ac, terms } => {
                let (region, base) = text.span(lines.clone());
                for m in ac.find_overlapping_iter(region) {
                    let pos = base + m.start() as u64;
                    let rel = starts.partition_point(|&s| s <= pos) - 1;
                    hit[rel] = true;
                    let line = lines.start + rel as u64;
                    let term = terms[m.pattern().as_usize()];
                    if last[term] != line {
                        last[term] = line;
                        hits[term] += 1;
                    }
                }
            }
            TermSet::Regex { set, terms } => {
                for (rel, line) in lines.clone().enumerate() {
                    let content = text.line(line);
                    if !set.is_match(content) {
                        continue;
                    }
                    hit[rel] = true;
                    for p in set.matches(content).iter() {
                        hits[terms[p]] += 1;
                    }
                }
            }
        }
    }
}

fn decide(has_include: bool, included: bool, has_exclude: bool, excluded: bool) -> bool {
    if has_include {
        included && !excluded
    } else if has_exclude {
        !excluded
    } else {
        false
    }
}

/// A `FilterSpec` compiled into matchers (spec §4.8).
pub struct CompiledFilter {
    spec: FilterSpec,
    include: TermSet,
    exclude: TermSet,
}

impl CompiledFilter {
    pub fn compile(spec: &FilterSpec) -> Result<Self, CoreError> {
        Ok(Self {
            spec: spec.clone(),
            include: TermSet::build(spec, TermState::Include)?,
            exclude: TermSet::build(spec, TermState::Exclude)?,
        })
    }

    pub fn spec(&self) -> &FilterSpec {
        &self.spec
    }

    pub fn is_active(&self) -> bool {
        !self.include.is_empty() || !self.exclude.is_empty()
    }

    pub fn is_excluded(&self, line: &[u8]) -> bool {
        self.exclude.is_match(line)
    }

    /// Spec §4.7 semantics for one line (`line` already has `\r` stripped).
    pub fn line_matches(&self, line: &[u8]) -> bool {
        let has_include = !self.include.is_empty();
        let included = has_include && self.include.is_match(line);
        if has_include && !included {
            return false;
        }
        decide(has_include, included, !self.exclude.is_empty(), self.exclude.is_match(line))
    }

    /// Scans `lines` of `text`: matched lines (ascending) and per-term hit counts.
    pub fn scan(&self, text: &LogText, lines: Range<u64>) -> ScanResult {
        let lines = lines.start..lines.end.min(text.line_count());
        let mut term_hits = vec![0u64; self.spec.terms.len()];
        if lines.start >= lines.end || !self.is_active() {
            return ScanResult { matches: Vec::new(), term_hits };
        }
        let n = (lines.end - lines.start) as usize;
        let starts: Vec<u64> = lines.clone().map(|l| text.index().line_start(l)).collect();
        let mut last = vec![u64::MAX; term_hits.len()];
        let mut included = vec![false; n];
        let mut excluded = vec![false; n];
        self.include.mark(text, &lines, &starts, &mut included, &mut term_hits, &mut last);
        self.exclude.mark(text, &lines, &starts, &mut excluded, &mut term_hits, &mut last);
        let (has_include, has_exclude) = (!self.include.is_empty(), !self.exclude.is_empty());
        let matches = (0..n)
            .filter(|&k| decide(has_include, included[k], has_exclude, excluded[k]))
            .map(|k| lines.start + k as u64)
            .collect();
        ScanResult { matches, term_hits }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::text::LogText;

    fn text(lines: &[&str]) -> LogText {
        LogText::from_bytes(lines.join("\n").into_bytes())
    }

    fn spec(terms: &[(&str, TermState)]) -> FilterSpec {
        FilterSpec { terms: terms.iter().map(|(t, s)| Term::new(*t, *s)).collect(), ..Default::default() }
    }

    fn scan_all(spec: &FilterSpec, t: &LogText) -> ScanResult {
        CompiledFilter::compile(spec).unwrap().scan(t, 0..t.line_count())
    }

    use TermState::{Exclude, Include, Off};

    #[test]
    fn semantics_follow_spec_4_7() {
        let t = text(&["error a", "timeout b", "error healthcheck", "ok", "retry"]);
        // include any, minus excludes
        assert_eq!(scan_all(&spec(&[("error", Include), ("timeout", Include), ("healthcheck", Exclude)]), &t).matches, vec![0, 1]);
        // only excludes: everything not excluded
        assert_eq!(scan_all(&spec(&[("healthcheck", Exclude)]), &t).matches, vec![0, 1, 3, 4]);
        // no active terms: nothing
        assert_eq!(scan_all(&spec(&[("error", Off)]), &t).matches, Vec::<u64>::new());
        assert_eq!(scan_all(&FilterSpec::default(), &t).matches, Vec::<u64>::new());
        // off terms are ignored entirely
        assert_eq!(scan_all(&spec(&[("error", Include), ("retry", Off)]), &t).matches, vec![0, 2]);
    }

    #[test]
    fn case_handling() {
        let t = text(&["ERROR one", "error two", "Error three"]);
        assert_eq!(scan_all(&spec(&[("error", Include)]), &t).matches, vec![0, 1, 2], "case-insensitive by default");
        let cs = FilterSpec { case_sensitive: true, ..spec(&[("error", Include)]) };
        assert_eq!(scan_all(&cs, &t).matches, vec![1]);
        let re = FilterSpec { regex: true, ..spec(&[("^error", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 1, 2]);
    }

    #[test]
    fn regex_terms_and_errors() {
        let t = text(&["request_id=req_8f29", "request_id=x", "duration=30241ms"]);
        let re = FilterSpec { regex: true, ..spec(&[(r"req_[0-9a-f]{4}", Include), (r"duration=\d{4,}ms", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 2]);
        let bad = FilterSpec { regex: true, ..spec(&[("ok", Include), ("(unclosed", Include)]) };
        match CompiledFilter::compile(&bad) {
            Err(CoreError::InvalidRegex { term, .. }) => assert_eq!(term, "(unclosed"),
            other => panic!("expected InvalidRegex, got {:?}", other.err()),
        }
    }

    #[test]
    fn empty_matching_regex_matches_every_line() {
        let t = text(&["a", "", "xyz"]);
        let re = FilterSpec { regex: true, ..spec(&[("x*", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0, 1, 2]);
    }

    #[test]
    fn trailing_cr_is_never_matched() {
        let t = LogText::from_bytes(b"error\r\nok\r\n".to_vec());
        let re = FilterSpec { regex: true, ..spec(&[("error$", Include)]) };
        assert_eq!(scan_all(&re, &t).matches, vec![0]);
        let plain_cr = spec(&[("ok", Include)]);
        assert_eq!(scan_all(&plain_cr, &t).matches, vec![1]);
    }

    #[test]
    fn per_term_hits_count_lines_once_and_include_overlaps() {
        let t = text(&["timeout timeout", "time", "error timeout", "ok"]);
        let s = spec(&[("time", Include), ("timeout", Include), ("error", Exclude), ("ok", Off)]);
        let r = scan_all(&s, &t);
        assert_eq!(r.term_hits, vec![3, 2, 1, 0]);
        assert_eq!(r.matches, vec![0, 1]);
    }

    #[test]
    fn invalid_utf8_lines_still_match_raw_bytes() {
        // Not `\xFF\xFE ...` at the start: that is a UTF-16LE BOM and the whole input would be decoded as UTF-16.
        let t = LogText::from_bytes(b"\xFF\xFD error\nok\n".to_vec());
        assert_eq!(scan_all(&spec(&[("error", Include)]), &t).matches, vec![0]);
        let mid = LogText::from_bytes(b"ok\n\xFF\xFE error\n".to_vec());
        assert_eq!(scan_all(&spec(&[("error", Include)]), &mid).matches, vec![1]);
    }

    #[test]
    fn partial_ranges_and_many_terms() {
        let lines: Vec<String> = (0..300).map(|i| format!("line {i} term{}", i % 200)).collect();
        let t = LogText::from_bytes(lines.join("\n").into_bytes());
        let terms: Vec<Term> = (0..200).map(|i| Term::include(format!("term{i}"))).collect();
        let compiled = CompiledFilter::compile(&FilterSpec { terms, ..Default::default() }).unwrap();
        let r = compiled.scan(&t, 100..150);
        assert_eq!(r.matches, (100..150).collect::<Vec<u64>>());
        // A line containing `term1XY` also contains the terms `term1` and `term1X` (overlapping matches count).
        assert_eq!(r.term_hits[1], 50);
        assert_eq!(r.term_hits[10], 10);
        assert_eq!(r.term_hits[123], 1);
        assert_eq!(r.term_hits.iter().sum::<u64>(), 150);
    }

    #[test]
    fn line_matches_agrees_with_scan() {
        let t = text(&["error a", "timeout b", "error healthcheck", "ok"]);
        let s = spec(&[("error", Include), ("healthcheck", Exclude)]);
        let c = CompiledFilter::compile(&s).unwrap();
        let by_line: Vec<u64> = (0..t.line_count()).filter(|&i| c.line_matches(t.line(i))).collect();
        assert_eq!(by_line, c.scan(&t, 0..t.line_count()).matches);
        assert!(c.is_excluded(b"x healthcheck"));
    }
}
