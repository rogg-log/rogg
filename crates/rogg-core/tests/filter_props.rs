use proptest::prelude::*;
use rogg_core::CancelToken;
use rogg_core::engine::{TailBase, run_filter, run_filter_lines};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::text::LogText;

const WORDS: &[&str] = &["error", "timeout", "retry", "ok", "ERROR", "Time", "req_1", "req_22", "x", "é"];
const TERMS: &[&str] = &["error", "time", "timeout", "retry", "req_", "ok", "é", r"req_\d+", "^ok", "error|retry", "t.me", "ok\r", "x\ne", "\n", "ok\r\nx"];

fn naive_term_hit(line: &str, spec: &FilterSpec, t: &Term) -> bool {
    if spec.regex {
        regex::RegexBuilder::new(&t.text).case_insensitive(!spec.case_sensitive).build().unwrap().is_match(line)
    } else if spec.case_sensitive {
        line.contains(&t.text)
    } else {
        line.to_ascii_lowercase().contains(&t.text.to_ascii_lowercase())
    }
}

/// Lines each term occurs in; `Off` terms are never evaluated, so they stay at 0.
fn naive_term_hits(lines: &[String], spec: &FilterSpec) -> Vec<u64> {
    spec.terms
        .iter()
        .map(|t| {
            if t.state == TermState::Off {
                return 0;
            }
            lines.iter().filter(|l| naive_term_hit(l, spec, t)).count() as u64
        })
        .collect()
}

fn naive(line: &str, spec: &FilterSpec) -> bool {
    let hit = |t: &Term| naive_term_hit(line, spec, t);
    let inc: Vec<&Term> = spec.terms.iter().filter(|t| t.state == TermState::Include).collect();
    let exc: Vec<&Term> = spec.terms.iter().filter(|t| t.state == TermState::Exclude).collect();
    if !inc.is_empty() {
        inc.iter().any(|t| hit(t)) && !exc.iter().any(|t| hit(t))
    } else if !exc.is_empty() {
        !exc.iter().any(|t| hit(t))
    } else {
        false
    }
}

fn lines() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        (proptest::collection::vec(proptest::sample::select(WORDS), 0..5), any::<bool>())
            .prop_map(|(words, cr)| format!("{}{}", words.join(" "), if cr { "\r" } else { "" })),
        0..60,
    )
}

fn spec() -> impl Strategy<Value = FilterSpec> {
    (
        proptest::collection::vec(
            (proptest::sample::select(TERMS), prop_oneof![Just(TermState::Include), Just(TermState::Off), Just(TermState::Exclude)]),
            0..4,
        ),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(terms, regex, case_sensitive)| FilterSpec {
            terms: terms.into_iter().map(|(t, s)| Term::new(t, s)).collect(),
            regex,
            case_sensitive,
            context: 0,
        })
}

proptest! {
    #[test]
    fn chunked_scan_equals_naive(lines in lines(), spec in spec(), chunk in 1u64..9) {
        let text = LogText::from_bytes(lines.join("\n").into_bytes());
        let compiled = CompiledFilter::compile(&spec).unwrap();
        let mut got = Vec::new();
        let mut start = 0;
        while start < text.line_count() {
            let end = (start + chunk).min(text.line_count());
            got.extend(compiled.scan(&text, start..end).matches);
            start = end;
        }
        let by_line: Vec<u64> = (0..text.line_count()).filter(|&i| compiled.line_matches(text.line(i))).collect();
        prop_assert_eq!(&got, &by_line);
        let expected: Vec<u64> = (0..text.line_count()).filter(|&i| naive(&text.line_text(i), &spec)).collect();
        prop_assert_eq!(got, expected);
    }

    /// Spec §8: the parallel engine equals the naive model, for any chunk size.
    #[test]
    fn parallel_filter_equals_naive(lines in lines(), spec in spec(), chunk_lines in 1u64..=64) {
        let text = LogText::from_bytes(lines.join("\n").into_bytes());
        let compiled = CompiledFilter::compile(&spec).unwrap();
        let result = run_filter(&text, &compiled, chunk_lines, &CancelToken::new()).unwrap();
        let display: Vec<String> = (0..text.line_count()).map(|i| text.line_text(i).into_owned()).collect();
        let expected: Vec<u64> = (0..text.line_count()).filter(|&i| naive(&display[i as usize], &spec)).collect();
        prop_assert_eq!(&result.matches, &expected);
        prop_assert_eq!(result.total_lines, text.line_count());
        prop_assert_eq!(result.term_hits, naive_term_hits(&display, &spec));
    }
}

proptest! {
    /// Spec §4.5 + Review Focus 3: a tail scan after an append, merged with the previous result, equals a full scan of
    /// the grown text — even when the cut fell inside a line that the append completes.
    #[test]
    fn tail_scan_merge_equals_a_full_scan(lines in lines(), spec in spec(), cut in any::<prop::sample::Index>(), chunk in 1u64..9) {
        let full = lines.join("\n").into_bytes();
        let cut = cut.index(full.len() + 1);
        let old = LogText::from_bytes(full[..cut].to_vec());
        let new = LogText::from_bytes(full.clone());
        let compiled = CompiledFilter::compile(&spec).unwrap();
        let prev = run_filter(&old, &compiled, chunk, &CancelToken::new()).unwrap();
        let base = TailBase::new(&old, &compiled, &prev.matches, &prev.term_hits);
        let tail = run_filter_lines(&new, &compiled, base.from..new.line_count(), chunk, &CancelToken::new()).unwrap();
        let (matches, hits) = base.merge(tail.matches, &tail.term_hits);
        let whole = run_filter(&new, &compiled, chunk, &CancelToken::new()).unwrap();
        prop_assert_eq!(matches, whole.matches);
        prop_assert_eq!(hits, whole.term_hits);
    }
}
