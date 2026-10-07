//! macOS locations for rogg's data (spec §4.2, §4.12, §6).

use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `~/Library/Application Support/rogg`
pub fn app_support_dir() -> PathBuf {
    home_dir().join("Library/Application Support/rogg")
}

/// `~/Library/Caches/rogg`
pub fn cache_dir() -> PathBuf {
    home_dir().join("Library/Caches/rogg")
}

pub fn history_path() -> PathBuf {
    app_support_dir().join("history.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_locations() {
        assert!(app_support_dir().ends_with("Library/Application Support/rogg"));
        assert!(cache_dir().ends_with("Library/Caches/rogg"));
        assert!(history_path().ends_with("Library/Application Support/rogg/history.json"));
    }
}
