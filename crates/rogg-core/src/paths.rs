//! Where rogg keeps its data on each OS (spec §3).

use crate::platform::{self, Env, Os};
use std::path::PathBuf;

fn env() -> Env {
    Env::from_process()
}

/// The user's home folder; the temp dir when unknown, so paths never become cwd-relative.
pub fn home_dir() -> PathBuf {
    platform::home(Os::current(), &env())
}

/// Settings and profiles (`~/Library/Application Support/rogg`, `~/.config/rogg`, `%APPDATA%\rogg`).
pub fn config_dir() -> PathBuf {
    platform::config_base(Os::current(), &env())
}

/// History, recent files and the session.
pub fn data_dir() -> PathBuf {
    platform::data_base(Os::current(), &env())
}

/// Kept for callers: the config folder.
pub fn app_support_dir() -> PathBuf {
    config_dir()
}

/// Conversions, extracted members, stream captures.
pub fn cache_dir() -> PathBuf {
    platform::cache_base(Os::current(), &env())
}

pub fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

pub fn recent_path() -> PathBuf {
    data_dir().join("recent.json")
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.toml")
}

/// One `<name>.toml` per highlighting profile (spec §6.1).
pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

pub fn session_path() -> PathBuf {
    data_dir().join("session.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_locations() {
        assert!(app_support_dir().ends_with("Library/Application Support/rogg"));
        assert!(cache_dir().ends_with("Library/Caches/rogg"));
        assert!(history_path().ends_with("Library/Application Support/rogg/history.json"));
    }

    /// Skipped when an `XDG_*` override is set, since it then decides the location.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_locations() {
        if ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"].iter().any(|k| std::env::var_os(k).is_some()) {
            return;
        }
        assert!(config_dir().ends_with(".config/rogg"));
        assert!(data_dir().ends_with(".local/share/rogg"));
        assert!(cache_dir().ends_with(".cache/rogg"));
        assert!(history_path().ends_with(".local/share/rogg/history.json"));
        assert!(settings_path().ends_with(".config/rogg/settings.toml"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn recent_path_is_in_app_support() {
        assert!(recent_path().ends_with("Library/Application Support/rogg/recent.json"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn m4_state_paths_are_in_app_support() {
        assert!(settings_path().ends_with("Application Support/rogg/settings.toml"));
        assert!(profiles_dir().ends_with("Application Support/rogg/profiles"));
        assert!(session_path().ends_with("Application Support/rogg/session.json"));
    }
}
