//! User settings (spec §6.2), stored as `settings.toml`. Every field has a default, so a partial or older file loads;
//! values out of range are clamped.

use crate::error::CoreError;
use crate::filter::MAX_CONTEXT;
use crate::persist::{load_toml, save_toml};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use std::path::Path;

pub const UI_FONT_SIZES: RangeInclusive<u32> = 11..=16;
pub const DEFAULT_UI_FONT_SIZE: u32 = 13;
/// Log font sizes (compact density; comfortable adds 1 px as in the reference).
pub const LOG_FONT_SIZES: RangeInclusive<u32> = 9..=20;
pub const DEFAULT_LOG_FONT_SIZE: u32 = 12;
/// The curated monospace fonts that ship with macOS (spec §6.2); "Other…" picks any installed one.
pub const LOG_FONTS: [&str; 4] = ["SF Mono", "Menlo", "Monaco", "Courier New"];
pub const DEFAULT_LOG_FONT: &str = "SF Mono";
pub const CACHE_LIMITS_GB: [u64; 9] = [1, 2, 5, 10, 20, 50, 100, 200, 500];
pub const DEFAULT_CACHE_LIMIT_GB: u64 = 20;
pub const SCAN_DEPTHS: RangeInclusive<u32> = 1..=10;
pub const DEFAULT_PROFILE_NAME: &str = "Default";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSetting {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DensitySetting {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeSetting,
    pub ui_font_size: u32,
    pub log_font: String,
    pub log_font_size: u32,
    pub density: DensitySetting,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeSetting::System,
            ui_font_size: DEFAULT_UI_FONT_SIZE,
            log_font: DEFAULT_LOG_FONT.into(),
            log_font_size: DEFAULT_LOG_FONT_SIZE,
            density: DensitySetting::Compact,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorSettings {
    pub wrap: bool,
    pub follow_selection: bool,
    pub context: u32,
    pub regex: bool,
    pub case_sensitive: bool,
    pub restore_session: bool,
    pub cache_limit_gb: u64,
    pub scan_depth: u32,
    pub default_profile: String,
    /// Whether the filtered log shows `··· N lines omitted` rows between shown lines.
    pub omitted_markers: bool,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            wrap: false,
            follow_selection: true,
            context: 0,
            regex: false,
            case_sensitive: false,
            restore_session: true,
            cache_limit_gb: DEFAULT_CACHE_LIMIT_GB,
            scan_depth: crate::folder::DEFAULT_SCAN_DEPTH as u32,
            default_profile: DEFAULT_PROFILE_NAME.into(),
            omitted_markers: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub appearance: AppearanceSettings,
    pub behavior: BehaviorSettings,
}

/// The next preset above (or below) `gb`; a value between presets moves to the neighbouring preset.
pub fn step_cache_limit(gb: u64, up: bool) -> u64 {
    if up {
        CACHE_LIMITS_GB.iter().copied().find(|&p| p > gb).unwrap_or(CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1])
    } else {
        CACHE_LIMITS_GB.iter().rev().copied().find(|&p| p < gb).unwrap_or(CACHE_LIMITS_GB[0])
    }
}

impl Settings {
    /// Every value within its range; empty names replaced by their defaults.
    pub fn clamped(mut self) -> Self {
        let a = &mut self.appearance;
        a.ui_font_size = a.ui_font_size.clamp(*UI_FONT_SIZES.start(), *UI_FONT_SIZES.end());
        a.log_font_size = a.log_font_size.clamp(*LOG_FONT_SIZES.start(), *LOG_FONT_SIZES.end());
        if a.log_font.trim().is_empty() {
            a.log_font = DEFAULT_LOG_FONT.into();
        }
        let b = &mut self.behavior;
        b.context = b.context.min(MAX_CONTEXT);
        b.cache_limit_gb = b.cache_limit_gb.clamp(CACHE_LIMITS_GB[0], CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1]);
        b.scan_depth = b.scan_depth.clamp(*SCAN_DEPTHS.start(), *SCAN_DEPTHS.end());
        if b.default_profile.trim().is_empty() {
            b.default_profile = DEFAULT_PROFILE_NAME.into();
        }
        self
    }

    pub fn cache_limit_bytes(&self) -> u64 {
        self.behavior.cache_limit_gb << 30
    }

    /// Loads `path`; a missing file gives the defaults, an unparseable one is set aside as `.bak` (`true`).
    pub fn load(path: &Path) -> Result<(Settings, bool), CoreError> {
        let (settings, recovered) = load_toml::<Settings>(path)?;
        Ok((settings.clamped(), recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_toml(path, &Settings { version: 1, ..self.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let s = Settings::default();
        assert_eq!(s.appearance.theme, ThemeSetting::System);
        assert_eq!((s.appearance.ui_font_size, s.appearance.log_font_size), (13, 12));
        assert_eq!(s.appearance.log_font, "SF Mono");
        assert_eq!(s.appearance.density, DensitySetting::Compact);
        let b = &s.behavior;
        assert!(!b.wrap && b.follow_selection && !b.regex && !b.case_sensitive && b.restore_session);
        assert!(b.omitted_markers);
        assert_eq!((b.context, b.cache_limit_gb, b.scan_depth), (0, 20, 4));
        assert_eq!(b.default_profile, "Default");
        assert_eq!(s.cache_limit_bytes(), crate::cache::DEFAULT_LIMIT_BYTES);
    }

    #[test]
    fn round_trip_through_toml_keeps_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let mut s = Settings::default();
        s.appearance.theme = ThemeSetting::Dark;
        s.appearance.log_font = "Menlo".into();
        s.appearance.density = DensitySetting::Comfortable;
        s.behavior.context = 7;
        s.behavior.default_profile = "Services".into();
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("theme = \"dark\"") && text.contains("[behavior]"), "{text}");
        assert_eq!(Settings::load(&path).unwrap(), (Settings { version: 1, ..s }, false));
    }

    #[test]
    fn corrupt_settings_recover_and_values_clamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[appearance]\nui_font_size = 40\nlog_font_size = 2\nlog_font = \"\"\n[behavior]\ncontext = 99\ncache_limit_gb = 0\nscan_depth = 50\ndefault_profile = \"\"\n").unwrap();
        let (s, recovered) = Settings::load(&path).unwrap();
        assert!(!recovered);
        assert_eq!((s.appearance.ui_font_size, s.appearance.log_font_size), (16, 9));
        assert_eq!(s.appearance.log_font, "SF Mono");
        assert_eq!((s.behavior.context, s.behavior.cache_limit_gb, s.behavior.scan_depth), (10, 1, 10));
        assert_eq!(s.behavior.default_profile, "Default");
        std::fs::write(&path, "theme = \"purple\"\n[appearance\n").unwrap();
        let (s, recovered) = Settings::load(&path).unwrap();
        assert!(recovered && s == Settings::default());
        assert!(dir.path().join("settings.toml.bak").exists());
        std::fs::write(&path, "[appearance]\ntheme = \"purple\"\n").unwrap();
        assert!(Settings::load(&path).unwrap().1, "an unknown theme is unparseable, so the file is set aside");
    }

    #[test]
    fn a_settings_file_without_omitted_markers_loads_with_them_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "[behavior]\nwrap = true\n").unwrap();
        let (s, recovered) = Settings::load(&path).unwrap();
        assert!(!recovered && s.behavior.wrap && s.behavior.omitted_markers);
        std::fs::write(&path, "[behavior]\nomitted_markers = false\n").unwrap();
        assert!(!Settings::load(&path).unwrap().0.behavior.omitted_markers);
        let mut s = Settings::default();
        s.behavior.omitted_markers = false;
        s.save(&path).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("omitted_markers = false"));
    }

    #[test]
    fn cache_limit_steps_through_presets() {
        assert_eq!(step_cache_limit(20, true), 50);
        assert_eq!(step_cache_limit(20, false), 10);
        assert_eq!(step_cache_limit(500, true), 500);
        assert_eq!(step_cache_limit(1, false), 1);
        assert_eq!(step_cache_limit(30, true), 50, "a hand-edited value moves to the next preset");
        assert_eq!(step_cache_limit(30, false), 20);
    }
}
