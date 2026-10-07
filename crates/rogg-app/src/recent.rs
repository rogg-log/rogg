//! Recent files and folders (spec §6.3), shared by the sidebar, the welcome view and File → Open Recent.

use gpui_kit::Global;
use rogg_core::recent::{RecentEntry, RecentList};
use std::path::{Path, PathBuf};

pub struct RecentStore {
    list: RecentList,
    path: PathBuf,
    /// The file was unreadable and was moved to `recent.json.bak` (shown as a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for RecentStore {}

impl RecentStore {
    pub fn load(path: PathBuf) -> Self {
        match RecentList::load(&path) {
            Ok((list, recovered)) => Self { list, path, recovered, last_error: None },
            Err(e) => Self { list: RecentList::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn entries(&self) -> &[RecentEntry] {
        self.list.entries()
    }

    pub fn record(&mut self, entry: RecentEntry) {
        self.list.record(entry);
        self.save();
    }

    pub fn remove(&mut self, path: &Path) {
        self.list.remove(path);
        self.save();
    }

    fn save(&mut self) {
        self.last_error = self.list.save(&self.path).err().map(|e| e.to_string());
    }
}
