//! Global filter history (spec §4.12), persisted as JSON with corruption recovery (spec §6.6).

use crate::error::CoreError;
use crate::filter::{FilterSpec, MAX_CONTEXT, Term, join_expression};
use crate::persist::{load_json, save_json};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const HISTORY_LIMIT: usize = 100;

/// Recent filters offered in the empty filtered view.
pub const RECENT_IN_EMPTY_VIEW: usize = 3;

/// Up to `n` entries, newest first, without the one equal to `current` (the filter already applied).
pub fn recent_except(entries: &[HistoryEntry], current: &FilterSpec, n: usize) -> Vec<HistoryEntry> {
    let current = HistoryEntry::from_spec(current);
    entries.iter().filter(|e| **e != current).take(n).cloned().collect()
}

/// Optional fields default when missing so older or hand-edited history files still load.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub terms: Vec<Term>,
    #[serde(default)]
    pub regex: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub context: u32,
}

impl HistoryEntry {
    pub fn from_spec(spec: &FilterSpec) -> Self {
        Self { terms: spec.terms.clone(), regex: spec.regex, case_sensitive: spec.case_sensitive, context: spec.context }
    }

    /// The context radius is clamped to `MAX_CONTEXT`, since a loaded file may hold any value.
    pub fn to_spec(&self) -> FilterSpec {
        FilterSpec { terms: self.terms.clone(), regex: self.regex, case_sensitive: self.case_sensitive, context: self.context.min(MAX_CONTEXT) }
    }

    pub fn expression(&self) -> String {
        join_expression(&self.terms)
    }

    /// `2 include · 1 exclude · 1 off · regex · case · ±2` — states, then the flags and context that are set.
    pub fn summary(&self) -> String {
        let mut parts = vec![self.to_spec().summary()];
        if self.regex {
            parts.push("regex".into());
        }
        if self.case_sensitive {
            parts.push("case".into());
        }
        if self.context > 0 {
            parts.push(format!("±{}", self.context.min(MAX_CONTEXT)));
        }
        parts.join(" · ")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    entries: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadOutcome {
    pub history: History,
    /// True when the file was unparseable and was moved aside to `<file>.bak`.
    pub recovered: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

impl History {
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Adds `entry` at the front; an identical existing entry moves to the front; keeps the newest 100.
    pub fn record(&mut self, entry: HistoryEntry) {
        if entry.terms.is_empty() {
            return;
        }
        self.entries.retain(|e| e != &entry);
        self.entries.insert(0, entry);
        self.entries.truncate(HISTORY_LIMIT);
    }

    /// Like `record`, but when the newest entry has the same terms (texts and states), regex flag and case flag
    /// it is replaced in place: a context-only change updates the entry instead of adding one (spec §4.12).
    pub fn record_or_update_newest(&mut self, entry: HistoryEntry) {
        if entry.terms.is_empty() {
            return;
        }
        match self.entries.first_mut() {
            Some(newest) if newest.terms == entry.terms && newest.regex == entry.regex && newest.case_sensitive == entry.case_sensitive => {
                *newest = entry;
                // The updated entry may now equal an older one.
                let newest = self.entries.remove(0);
                self.entries.retain(|e| e != &newest);
                self.entries.insert(0, newest);
            }
            _ => self.record(entry),
        }
    }

    pub fn load(path: &Path) -> Result<LoadOutcome, CoreError> {
        let (file, recovered) = load_json::<HistoryFile>(path)?;
        let mut history = History { entries: file.entries };
        for entry in &mut history.entries {
            entry.context = entry.context.min(MAX_CONTEXT);
        }
        history.entries.truncate(HISTORY_LIMIT);
        Ok(LoadOutcome { history, recovered })
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &HistoryFile { version: 1, entries: self.entries.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};

    fn entry(expr: &str) -> HistoryEntry {
        HistoryEntry::from_spec(&FilterSpec::from_expression(expr))
    }

    #[test]
    fn recent_except_skips_the_current_filter_and_keeps_the_newest() {
        let mut h = History::default();
        for e in ["a", "b", "c", "d", "e"] {
            h.record(entry(e));
        }
        let names = |v: Vec<HistoryEntry>| v.iter().map(HistoryEntry::expression).collect::<Vec<_>>();
        assert_eq!(names(recent_except(h.entries(), &entry("e").to_spec(), 3)), ["d", "c", "b"], "newest first, current skipped");
        assert_eq!(names(recent_except(h.entries(), &entry("zzz").to_spec(), 3)), ["e", "d", "c"]);
        assert!(recent_except(&[entry("only")], &entry("only").to_spec(), 3).is_empty());
    }

    #[test]
    fn summaries_show_states_flags_and_context() {
        let spec = FilterSpec {
            terms: vec![Term::include("a"), Term::include("b"), Term::new("c", TermState::Exclude), Term::new("d", TermState::Off)],
            regex: true,
            case_sensitive: true,
            context: 2,
        };
        assert_eq!(HistoryEntry::from_spec(&spec).summary(), "2 include · 1 exclude · 1 off · regex · case · ±2");
        assert_eq!(entry("a").summary(), "1 on · 0 off", "without exclusions the summary is on / off");
    }

    #[test]
    fn record_dedups_moves_to_front_and_caps() {
        let mut h = History::default();
        h.record(entry("a"));
        h.record(entry("b"));
        h.record(entry("a"));
        assert_eq!(h.entries().iter().map(|e| e.expression()).collect::<Vec<_>>(), ["a", "b"]);
        for i in 0..150 {
            h.record(entry(&format!("t{i}")));
        }
        assert_eq!(h.entries().len(), HISTORY_LIMIT);
        assert_eq!(h.entries()[0].expression(), "t149");
        h.record(HistoryEntry::from_spec(&FilterSpec::default()));
        assert_eq!(h.entries()[0].expression(), "t149", "empty filters are not recorded");
    }

    #[test]
    fn record_or_update_newest_replaces_a_context_only_change() {
        let with_context = |expr: &str, context: u32| HistoryEntry::from_spec(&FilterSpec { context, ..FilterSpec::from_expression(expr) });
        let mut h = History::default();
        h.record(with_context("a", 0));
        h.record(with_context("b", 0));
        h.record_or_update_newest(with_context("b", 2));
        assert_eq!(h.entries().len(), 2);
        assert_eq!((h.entries()[0].expression(), h.entries()[0].context), ("b".to_string(), 2));
        h.record_or_update_newest(with_context("a", 1));
        assert_eq!(h.entries().len(), 3, "a different filter is a new entry");
        assert_eq!(h.entries()[0].expression(), "a");
        h.record_or_update_newest(HistoryEntry::from_spec(&FilterSpec::default()));
        assert_eq!(h.entries().len(), 3, "empty filters are not recorded");
        let mut regex = with_context("a", 1);
        regex.regex = true;
        h.record_or_update_newest(regex);
        assert_eq!(h.entries().len(), 4, "a flag change is a new entry");
    }

    #[test]
    fn entries_restore_states_and_flags() {
        let spec = FilterSpec { terms: vec![Term::include("a"), Term::new("b", TermState::Exclude)], regex: true, case_sensitive: true, context: 2 };
        assert_eq!(HistoryEntry::from_spec(&spec).to_spec(), spec);
        let same_text_other_state = HistoryEntry::from_spec(&FilterSpec { terms: vec![Term::include("a"), Term::include("b")], ..spec.clone() });
        let mut h = History::default();
        h.record(HistoryEntry::from_spec(&spec));
        h.record(same_text_other_state);
        assert_eq!(h.entries().len(), 2, "different term states are different entries");
    }

    #[test]
    fn save_load_round_trip_missing_and_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/history.json");
        let missing = History::load(&path).unwrap();
        assert!(missing.history.entries().is_empty() && !missing.recovered);
        let mut h = History::default();
        h.record(entry("error || timeout"));
        h.save(&path).unwrap();
        assert_eq!(History::load(&path).unwrap().history, h);
        std::fs::write(&path, "{ not json").unwrap();
        let recovered = History::load(&path).unwrap();
        assert!(recovered.recovered && recovered.history.entries().is_empty());
        assert!(dir.path().join("nested/history.json.bak").exists());
    }

    #[test]
    fn non_utf8_file_is_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, [0x7b, 0xff, 0xfe]).unwrap();
        let outcome = History::load(&path).unwrap();
        assert!(outcome.recovered && outcome.history.entries().is_empty());
        assert!(!path.exists());
        assert_eq!(std::fs::read(dir.path().join("history.json.bak")).unwrap(), [0x7b, 0xff, 0xfe]);
    }

    #[test]
    fn load_tolerates_missing_fields_and_clamps_context() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, r#"{"entries":[{"terms":[{"text":"a","state":"Include"}],"context":99},{"terms":[{"text":"b","state":"Off"}]}]}"#).unwrap();
        let outcome = History::load(&path).unwrap();
        assert!(!outcome.recovered);
        let specs: Vec<_> = outcome.history.entries().iter().map(|e| e.to_spec()).collect();
        assert_eq!(specs[0].context, crate::filter::MAX_CONTEXT);
        assert_eq!((specs[1].context, specs[1].regex, specs[1].case_sensitive), (0, false, false));
        let unclamped = HistoryEntry { terms: vec![Term::include("x")], regex: false, case_sensitive: false, context: 50 };
        assert_eq!(unclamped.to_spec().context, crate::filter::MAX_CONTEXT);
    }
}
