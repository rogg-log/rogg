use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum TermState {
    Include,
    Off,
    Exclude,
}

impl TermState {
    /// Include → Off → Exclude → Include (spec §4.6).
    pub fn next(self) -> Self {
        self.next_in(true)
    }

    /// The state after a click: Include ↔ Off, or Include → Off → Exclude → Include when excluding is allowed (the
    /// `behavior.exclude_terms` setting). A leftover Exclude always turns on.
    pub fn next_in(self, allow_exclude: bool) -> Self {
        match self {
            TermState::Include => TermState::Off,
            TermState::Off if allow_exclude => TermState::Exclude,
            TermState::Off | TermState::Exclude => TermState::Include,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Term {
    pub text: String,
    pub state: TermState,
}

impl Term {
    pub fn new(text: impl Into<String>, state: TermState) -> Self {
        Self { text: text.into(), state }
    }

    pub fn include(text: impl Into<String>) -> Self {
        Self::new(text, TermState::Include)
    }
}

/// Largest context radius (spec §4.7: UI presets ±3, up to 10 via settings).
pub const MAX_CONTEXT: u32 = 10;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct FilterSpec {
    pub terms: Vec<Term>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TermCounts {
    pub include: usize,
    pub exclude: usize,
    pub off: usize,
}

/// The text a new term gets when it stands for the literal `text` (Filter on / Exclude a word): regex-escaped when
/// the filter is in regex mode, so the word matches itself and nothing else.
pub fn literal_term(text: &str, regex: bool) -> String {
    if regex { regex::escape(text) } else { text.to_string() }
}

/// Splits on `||`, honouring `\|\|` as a literal `||`; trims terms and drops empty ones (spec §4.6).
pub fn split_expression(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix("\\|\\|") {
            current.push_str("||");
            rest = r;
        } else if let Some(r) = rest.strip_prefix("||") {
            terms.push(std::mem::take(&mut current));
            rest = r;
        } else {
            current.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    terms.push(current);
    terms.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

/// Inverse of `split_expression` for display in the expression editor.
///
/// Inverts `split_expression` for every term list that `split_expression` can produce;
/// a term whose text itself contains the literal sequence `\|\|` (only possible when
/// constructed programmatically) re-parses as `||`. The editor cycle
/// split → join → split is stable for all terms that originate from `split_expression`.
pub fn join_expression(terms: &[Term]) -> String {
    terms.iter().map(|t| t.text.replace("||", "\\|\\|")).collect::<Vec<_>>().join(" || ")
}

impl FilterSpec {
    pub fn from_expression(text: &str) -> Self {
        Self { terms: split_expression(text).into_iter().map(Term::include).collect(), ..Self::default() }
    }

    /// Re-parses an edited expression; a term whose text is unchanged keeps its state, new terms start as Include.
    pub fn with_expression(&self, text: &str) -> Self {
        let terms = split_expression(text)
            .into_iter()
            .map(|t| {
                let state = self.terms.iter().find(|old| old.text == t).map(|old| old.state).unwrap_or(TermState::Include);
                Term { text: t, state }
            })
            .collect();
        Self { terms, ..self.clone() }
    }

    pub fn expression(&self) -> String {
        join_expression(&self.terms)
    }

    pub fn cycle(&mut self, index: usize, allow_exclude: bool) {
        if let Some(term) = self.terms.get_mut(index) {
            term.state = term.state.next_in(allow_exclude);
        }
    }

    /// The spec with every Exclude term turned Off (excluding is switched off in the settings).
    pub fn without_excludes(mut self) -> FilterSpec {
        for term in &mut self.terms {
            if term.state == TermState::Exclude {
                term.state = TermState::Off;
            }
        }
        self
    }

    pub fn set_context(&mut self, context: u32) {
        self.context = context.min(MAX_CONTEXT);
    }

    pub fn counts(&self) -> TermCounts {
        let mut c = TermCounts::default();
        for t in &self.terms {
            match t.state {
                TermState::Include => c.include += 1,
                TermState::Exclude => c.exclude += 1,
                TermState::Off => c.off += 1,
            }
        }
        c
    }

    pub fn summary(&self) -> String {
        let c = self.counts();
        if c.exclude == 0 {
            return format!("{} on · {} off", c.include, c.off);
        }
        format!("{} include · {} exclude · {} off", c.include, c.exclude, c.off)
    }

    /// True when at least one term is Include or Exclude.
    pub fn is_active(&self) -> bool {
        self.terms.iter().any(|t| t.state != TermState::Off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_term_escapes_only_in_regex_mode() {
        let word = "{payments_v2:true,";
        assert_eq!(literal_term(word, false), word);
        let escaped = literal_term(word, true);
        assert_ne!(escaped, word);
        let re = regex::Regex::new(&escaped).expect("the escaped word compiles");
        assert!(re.is_match(&format!("x {word} y")));
        assert!(!re.is_match("{payments_v2:true"), "the punctuation is literal");
        assert!(regex::Regex::new(word).is_err(), "unescaped it is not even a regex");
    }

    #[test]
    fn splits_on_double_pipe_trims_and_drops_empty() {
        assert_eq!(split_expression("error || timeout ||  healthcheck "), ["error", "timeout", "healthcheck"]);
        assert_eq!(split_expression(" || a ||  || b ||"), ["a", "b"]);
        assert_eq!(split_expression(""), Vec::<String>::new());
        assert_eq!(split_expression("a|b || c"), ["a|b", "c"], "a single pipe is literal");
    }

    #[test]
    fn escaped_double_pipe_is_literal() {
        assert_eq!(split_expression(r"a \|\| b || c"), ["a || b", "c"]);
        assert_eq!(split_expression(r"x\|\|y"), ["x||y"]);
        let terms = vec![Term::include("a || b"), Term::include("c")];
        assert_eq!(join_expression(&terms), r"a \|\| b || c");
        assert_eq!(split_expression(&join_expression(&terms)), ["a || b", "c"]);
    }

    #[test]
    fn editing_keeps_states_of_unchanged_terms() {
        let mut spec = FilterSpec::from_expression("error || timeout || healthcheck || retry");
        spec.cycle(2, true);
        spec.cycle(2, true); // healthcheck: Include → Off → Exclude
        spec.cycle(3, true); // retry: Off
        let edited = spec.with_expression("timeout || healthcheck || connection reset");
        assert_eq!(edited.terms, vec![
            Term::new("timeout", TermState::Include),
            Term::new("healthcheck", TermState::Exclude),
            Term::new("connection reset", TermState::Include),
        ]);
        assert_eq!(edited.regex, spec.regex);
    }

    #[test]
    fn state_cycle_counts_and_summary() {
        assert_eq!(TermState::Include.next(), TermState::Off);
        assert_eq!(TermState::Off.next(), TermState::Exclude);
        assert_eq!(TermState::Exclude.next(), TermState::Include);
        let mut spec = FilterSpec::from_expression("error || timeout || healthcheck || retry");
        spec.terms[2].state = TermState::Exclude;
        spec.terms[3].state = TermState::Off;
        assert_eq!(spec.counts(), TermCounts { include: 2, exclude: 1, off: 1 });
        assert_eq!(spec.summary(), "2 include · 1 exclude · 1 off");
        assert!(spec.is_active());
        spec.cycle(99, true); // out of range is ignored
        let all_off = FilterSpec { terms: vec![Term::new("a", TermState::Off)], ..Default::default() };
        assert!(!all_off.is_active());
    }

    #[test]
    fn context_is_clamped() {
        let mut spec = FilterSpec::default();
        spec.set_context(25);
        assert_eq!(spec.context, MAX_CONTEXT);
    }

    #[test]
    fn serde_round_trip() {
        let spec = FilterSpec { regex: true, case_sensitive: true, context: 2, ..FilterSpec::from_expression("a || b") };
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<FilterSpec>(&json).unwrap(), spec);
    }

    #[test]
    fn known_limitation_backslash_pipe_pipe() {
        // A term whose text contains the literal sequence \|\| (only possible when
        // constructed programmatically) re-parses as ||. This is the known limitation:
        // split(join([Term::include(r"a\|\|b")])) produces ["a||b"] not ["a\|\|b"].
        assert_eq!(
            split_expression(&join_expression(&[Term::include(r"a\|\|b")])),
            ["a||b"]
        );
    }
}
