//! Recent files and folders (spec §6.3), shared by the sidebar, the welcome view and File → Open Recent.

use gpui_kit::Global;
use rogg_core::recent::{RecentEntry, RecentList};
use std::path::{Path, PathBuf};

pub struct RecentStore {
    list: RecentList,
    path: PathBuf,
    /// Whether each entry's path was missing when the list was last loaded or changed, parallel to `entries()`.
    /// Cached so rendering never touches the filesystem (a stale network mount would stall every frame).
    missing: Vec<bool>,
    /// The file was unreadable and was moved to `recent.json.bak` (shown as a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for RecentStore {}

impl RecentStore {
    pub fn load(path: PathBuf) -> Self {
        match RecentList::load(&path) {
            Ok((list, recovered)) => Self::with_list(list, path, recovered, None),
            Err(e) => Self::with_list(RecentList::default(), path, false, Some(e.to_string())),
        }
    }

    fn with_list(list: RecentList, path: PathBuf, recovered: bool, last_error: Option<String>) -> Self {
        let mut store = Self { list, path, missing: Vec::new(), recovered, last_error };
        store.refresh_missing();
        store
    }

    fn refresh_missing(&mut self) {
        self.missing = self.list.entries().iter().map(RecentEntry::is_missing).collect();
    }

    /// Whether entry `index` was missing at the last load, record or removal (no filesystem access).
    pub fn is_missing(&self, index: usize) -> bool {
        self.missing.get(index).copied().unwrap_or(false)
    }

    pub fn entries(&self) -> &[RecentEntry] {
        self.list.entries()
    }

    pub fn record(&mut self, entry: RecentEntry) {
        self.list.record(entry);
        self.refresh_missing();
        self.save();
    }

    pub fn remove(&mut self, path: &Path) {
        self.list.remove(path);
        self.refresh_missing();
        self.save();
    }

    fn save(&mut self) {
        self.last_error = self.list.save(&self.path).err().map(|e| e.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_flags_are_computed_on_load_and_record_only() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("recent.json");
        let present = dir.path().join("present.log");
        let gone = dir.path().join("gone.log");
        std::fs::write(&present, "x").unwrap();
        std::fs::write(&gone, "x").unwrap();

        let mut store = RecentStore::load(state.clone());
        store.record(RecentEntry::file(&gone));
        store.record(RecentEntry::file(&present));
        assert!(!store.is_missing(0) && !store.is_missing(1), "both exist when recorded");

        std::fs::remove_file(&gone).unwrap();
        assert!(!store.is_missing(1), "deleting a file does not touch the cache until the next refresh");

        let reloaded = RecentStore::load(state);
        assert!(!reloaded.is_missing(0) && reloaded.is_missing(1), "load recomputes");
        assert!(!reloaded.is_missing(9), "out of range is not missing");

        store.record(RecentEntry::file(&present));
        assert!(store.is_missing(1), "record recomputes");
        store.remove(&gone);
        assert_eq!(store.entries().len(), 1);
        assert!(!store.is_missing(0));
    }
}
