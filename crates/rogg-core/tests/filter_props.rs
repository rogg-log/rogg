use proptest::prelude::*;
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::text::LogText;

const WORDS: &[&str] = &["error", "timeout", "retry", "ok", "ERROR", "Time", "req_1", "req_22", "x", "é"];
const TERMS: &[&str] = &["error", "time", "timeout", "retry", "req_", "ok", "é", r"req_\d+", "^ok", "error|retry", "t.me", "ok\r", "x\ne", "\n", "ok\r\nx"];

fn naive(line: &str, spec: &FilterSpec) -> bool {
    let hit = |t: &Term| {
        if spec.regex {
            regex::RegexBuilder::new(&t.text).case_insensitive(!spec.case_sensitive).build().unwrap().is_match(line)
        } else if spec.case_sensitive {
            line.contains(&t.text)
        } else {
            line.to_ascii_lowercase().contains(&t.text.to_ascii_lowercase())
        }
    };
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
}
