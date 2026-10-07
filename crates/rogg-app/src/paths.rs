//! Where the app keeps its state. A global, so tests and the gallery can point it at a temp directory.

use gpui_kit::Global;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct AppPaths {
    pub history: PathBuf,
    pub recent: PathBuf,
    /// Transcoded UTF-16 files (spec §4.2).
    pub cache: PathBuf,
}

impl Global for AppPaths {}

impl AppPaths {
    /// `~/Library/Application Support/rogg/…` and `~/Library/Caches/rogg`.
    pub fn system() -> Self {
        Self { history: rogg_core::paths::history_path(), recent: rogg_core::paths::recent_path(), cache: rogg_core::paths::cache_dir() }
    }

    /// Everything under `dir` (tests, gallery captures).
    pub fn under(dir: &Path) -> Self {
        Self { history: dir.join("history.json"), recent: dir.join("recent.json"), cache: dir.join("cache") }
    }
}
