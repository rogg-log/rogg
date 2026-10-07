//! Global filter history (spec §4.12): loaded at start, saved after each recorded filter. Saves run on the UI
//! thread one at a time, so the history file's single `.partial` name is never shared.

use gpui_kit::Global;
use rogg_core::filter::FilterSpec;
use rogg_core::history::{History, HistoryEntry};
use std::path::PathBuf;

pub struct HistoryStore {
    history: History,
    path: PathBuf,
    /// The file was unreadable and was moved to `history.json.bak` (shown as a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for HistoryStore {}

impl HistoryStore {
    pub fn load(path: PathBuf) -> Self {
        match History::load(&path) {
            Ok(outcome) => Self { history: outcome.history, path, recovered: outcome.recovered, last_error: None },
            Err(e) => Self { history: History::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        self.history.entries()
    }

    pub fn record(&mut self, spec: &FilterSpec) {
        self.history.record(HistoryEntry::from_spec(spec));
        self.last_error = self.history.save(&self.path).err().map(|e| e.to_string());
    }
}
