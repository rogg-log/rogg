//! macOS locations for rogg's data (spec §4.2, §4.12, §6).

use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    home_from(std::env::var_os("HOME"))
}

/// An unset or empty `HOME` falls back to the temp dir, so paths never become cwd-relative.
fn home_from(home: Option<std::ffi::OsString>) -> PathBuf {
    home.filter(|h| !h.is_empty()).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
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

pub fn recent_path() -> PathBuf {
    app_support_dir().join("recent.json")
}

pub fn settings_path() -> PathBuf {
    app_support_dir().join("settings.toml")
}

/// One `<name>.toml` per highlighting profile (spec §6.1).
pub fn profiles_dir() -> PathBuf {
    app_support_dir().join("profiles")
}

pub fn session_path() -> PathBuf {
    app_support_dir().join("session.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_unset_home_falls_back_to_temp_dir() {
        assert_eq!(home_from(None), std::env::temp_dir());
        assert_eq!(home_from(Some("".into())), std::env::temp_dir());
        assert_eq!(home_from(Some("/Users/x".into())), PathBuf::from("/Users/x"));
    }

    #[test]
    fn macos_locations() {
        assert!(app_support_dir().ends_with("Library/Application Support/rogg"));
        assert!(cache_dir().ends_with("Library/Caches/rogg"));
        assert!(history_path().ends_with("Library/Application Support/rogg/history.json"));
    }

    #[test]
    fn recent_path_is_in_app_support() {
        assert!(recent_path().ends_with("Library/Application Support/rogg/recent.json"));
    }

    #[test]
    fn m4_state_paths_are_in_app_support() {
        assert!(settings_path().ends_with("Application Support/rogg/settings.toml"));
        assert!(profiles_dir().ends_with("Application Support/rogg/profiles"));
        assert!(session_path().ends_with("Application Support/rogg/session.json"));
    }
}
