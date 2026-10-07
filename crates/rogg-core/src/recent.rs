//! Recently opened files and folders (spec §6.3), persisted as `recent.json`.

use crate::error::CoreError;
use crate::persist::{load_json, save_json};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const RECENT_LIMIT: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub is_folder: bool,
}

impl RecentEntry {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: false }
    }

    pub fn folder(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), is_folder: true }
    }

    /// The last path component, for list labels.
    pub fn label(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.path.display().to_string())
    }

    /// True when the path no longer exists; such entries render faint (spec §6.3).
    pub fn is_missing(&self) -> bool {
        !self.path.exists()
    }
}

/// Newest first, at most `RECENT_LIMIT` entries, unique by path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentList {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<RecentEntry>,
}

impl RecentList {
    pub fn entries(&self) -> &[RecentEntry] {
        &self.entries
    }

    /// Moves `entry` to the front (an entry with the same path is replaced) and keeps the newest 20.
    pub fn record(&mut self, entry: RecentEntry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.insert(0, entry);
        self.entries.truncate(RECENT_LIMIT);
    }

    pub fn remove(&mut self, path: &Path) {
        self.entries.retain(|e| e.path != path);
    }

    /// Returns the list and whether a corrupt file was moved aside to `<file>.bak`.
    pub fn load(path: &Path) -> Result<(RecentList, bool), CoreError> {
        let (mut list, recovered) = load_json::<RecentList>(path)?;
        list.entries.truncate(RECENT_LIMIT);
        Ok((list, recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &RecentList { version: 1, entries: self.entries.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_fails_to_save_instead_of_panicking() {
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let mut list = RecentList::default();
        list.record(RecentEntry::file(PathBuf::from(std::ffi::OsStr::from_bytes(b"\xff.log"))));
        let target = dir.path().join("recent.json");
        assert!(matches!(list.save(&target), Err(CoreError::Io { .. })));
        assert!(!target.exists(), "nothing half-written is left behind");
    }

    #[test]
    fn record_dedups_by_path_moves_to_front_and_caps() {
        let mut list = RecentList::default();
        list.record(RecentEntry::file("/a.log"));
        list.record(RecentEntry::folder("/logs"));
        list.record(RecentEntry::file("/a.log"));
        let paths: Vec<_> = list.entries().iter().map(|e| e.path.to_str().unwrap().to_string()).collect();
        assert_eq!(paths, ["/a.log", "/logs"]);
        for i in 0..30 {
            list.record(RecentEntry::file(format!("/f{i}.log")));
        }
        assert_eq!(list.entries().len(), RECENT_LIMIT);
        assert_eq!(list.entries()[0].path.to_str(), Some("/f29.log"));
    }

    #[test]
    fn remove_drops_the_entry() {
        let mut list = RecentList::default();
        list.record(RecentEntry::file("/a.log"));
        list.record(RecentEntry::file("/b.log"));
        list.remove(std::path::Path::new("/a.log"));
        assert_eq!(list.entries().len(), 1);
        assert_eq!(list.entries()[0].path.to_str(), Some("/b.log"));
    }

    #[test]
    fn save_load_round_trip_and_corruption_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let mut list = RecentList::default();
        list.record(RecentEntry::folder("/logs"));
        list.save(&path).unwrap();
        let (loaded, recovered) = RecentList::load(&path).unwrap();
        assert!(!recovered);
        assert_eq!(loaded.entries(), list.entries());
        std::fs::write(&path, "not json").unwrap();
        let (loaded, recovered) = RecentList::load(&path).unwrap();
        assert!(recovered);
        assert!(loaded.entries().is_empty());
    }

    #[test]
    fn label_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("here.log");
        std::fs::write(&present, "x").unwrap();
        assert_eq!(RecentEntry::file(&present).label(), "here.log");
        assert!(!RecentEntry::file(&present).is_missing());
        assert!(RecentEntry::file(dir.path().join("gone.log")).is_missing());
    }
}
