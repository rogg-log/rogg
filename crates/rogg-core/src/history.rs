//! Global filter history (spec §4.12), persisted as JSON with corruption recovery (spec §6.6).

use crate::error::CoreError;
use crate::filter::{FilterSpec, MAX_CONTEXT, Term, join_expression};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HISTORY_LIMIT: usize = 100;

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

#[derive(Serialize, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
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

    pub fn load(path: &Path) -> Result<LoadOutcome, CoreError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadOutcome { history: History::default(), recovered: false });
            }
            Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
        };
        match serde_json::from_slice::<HistoryFile>(&bytes) {
            Ok(file) => {
                let mut history = History { entries: file.entries };
                for entry in &mut history.entries {
                    entry.context = entry.context.min(MAX_CONTEXT);
                }
                history.entries.truncate(HISTORY_LIMIT);
                Ok(LoadOutcome { history, recovered: false })
            }
            Err(_) => {
                let backup = with_suffix(path, ".bak");
                std::fs::rename(path, &backup).map_err(CoreError::io(path))?;
                Ok(LoadOutcome { history: History::default(), recovered: true })
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
        }
        let json = serde_json::to_string_pretty(&HistoryFile { version: 1, entries: self.entries.clone() })
            .expect("history serializes");
        let partial = with_suffix(path, ".partial");
        std::fs::write(&partial, json).map_err(CoreError::io(&partial))?;
        std::fs::rename(&partial, path).map_err(CoreError::io(path))
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
