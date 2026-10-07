//! Where the app keeps its state. A global, so tests and the gallery can point it at a temp directory.

use gpui_kit::Global;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct AppPaths {
    pub history: PathBuf,
    pub recent: PathBuf,
    /// The cache directory (spec §4.2): conversions, extracted members, decompressed files, stream captures.
    pub cache: PathBuf,
    pub settings: PathBuf,
    /// The profile library directory (`*.toml`).
    pub profiles: PathBuf,
    pub session: PathBuf,
}

impl Global for AppPaths {}

impl AppPaths {
    /// `~/Library/Application Support/rogg/…` and `~/Library/Caches/rogg`.
    pub fn system() -> Self {
        Self {
            history: rogg_core::paths::history_path(),
            recent: rogg_core::paths::recent_path(),
            cache: rogg_core::paths::cache_dir(),
            settings: rogg_core::paths::settings_path(),
            profiles: rogg_core::paths::profiles_dir(),
            session: rogg_core::paths::session_path(),
        }
    }

    /// Everything under `dir` (tests, gallery captures).
    pub fn under(dir: &Path) -> Self {
        Self {
            history: dir.join("history.json"),
            recent: dir.join("recent.json"),
            cache: dir.join("cache"),
            settings: dir.join("settings.toml"),
            profiles: dir.join("profiles"),
            session: dir.join("session.json"),
        }
    }
}
