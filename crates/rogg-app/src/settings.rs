//! User settings (spec §6.2) as an app global: loaded at start, saved on every change and applied at once.

use crate::cache::{CacheStore, evict_soon};
use gpui_kit::{App, BorrowAppContext, Global, SharedString, WindowAppearance};
use rogg_core::settings::{DEFAULT_LOG_FONT, DensitySetting, LOG_FONTS, Settings, ThemeSetting};
use rogg_ui::theme::{Appearance, ChromeStyle, Density, JETBRAINS_MONO, MONO_FONT, RoggTheme};
use std::path::PathBuf;

pub struct SettingsStore {
    settings: Settings,
    path: PathBuf,
    /// The file was unreadable and was moved to `settings.toml.bak` (a toast at start).
    pub recovered: bool,
    pub last_error: Option<String>,
}

impl Global for SettingsStore {}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        match Settings::load(&path) {
            Ok((settings, recovered)) => Self { settings, path, recovered, last_error: None },
            Err(e) => Self { settings: Settings::default(), path, recovered: false, last_error: Some(e.to_string()) },
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The current settings; the defaults when the store is not installed.
    pub fn get(cx: &App) -> Settings {
        cx.try_global::<SettingsStore>().map(|s| s.settings.clone()).unwrap_or_default()
    }

    /// Changes, clamps and saves the settings; returns them before and after.
    pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) -> (Settings, Settings) {
        if !cx.has_global::<SettingsStore>() {
            let before = Settings::default();
            let mut after = before.clone();
            f(&mut after);
            return (before, after.clamped());
        }
        cx.update_global::<SettingsStore, _>(|store, _| {
            let before = store.settings.clone();
            let mut after = before.clone();
            f(&mut after);
            store.settings = after.clamped();
            store.last_error = store.settings.save(&store.path).err().map(|e| e.to_string());
            (before, store.settings.clone())
        })
    }
}

pub fn appearance_for(choice: ThemeSetting, window: WindowAppearance) -> Appearance {
    match choice {
        ThemeSetting::Dark => Appearance::Dark,
        ThemeSetting::Light => Appearance::Light,
        ThemeSetting::System => match window {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
            _ => Appearance::Light,
        },
    }
}

pub fn density_for(density: DensitySetting) -> Density {
    match density {
        DensitySetting::Compact => Density::Compact,
        DensitySetting::Comfortable => Density::Comfortable,
    }
}

/// The font family for a log font setting. The default entry ("SF Mono" in the file, so settings carry across
/// platforms) is the platform's monospaced font: on Mac the system one, whose family name is
/// `.AppleSystemUIFontMonospaced` (the name "SF Mono" silently falls back to Helvetica in gpui), elsewhere the bundled
/// JetBrains Mono.
pub fn log_font_family_for(name: &str, chrome: ChromeStyle) -> SharedString {
    if name != DEFAULT_LOG_FONT {
        return SharedString::from(name.to_string());
    }
    if chrome.is_mac() { MONO_FONT.into() } else { JETBRAINS_MONO.into() }
}

/// What the Settings font picker calls the default entry.
pub fn default_log_font_label(chrome: ChromeStyle) -> &'static str {
    if chrome.is_mac() { "SF Mono" } else { "JetBrains Mono" }
}

/// The curated log fonts installed on this Mac, looked up once per run (`all_font_names` lists every installed family:
/// too slow for every frame of the Settings dialog).
pub struct LogFonts(pub Vec<String>);

impl Global for LogFonts {}

/// The curated log fonts installed on this Mac (spec §6.2: fonts that are not available are hidden). SF Mono always is.
/// Cached by `cache_log_fonts`; looked up afresh until then.
pub fn available_log_fonts(cx: &App) -> Vec<String> {
    match cx.try_global::<LogFonts>() {
        Some(fonts) => fonts.0.clone(),
        None => find_log_fonts(cx),
    }
}

/// Looks the curated log fonts up once (when the Settings dialog opens).
pub fn cache_log_fonts(cx: &mut App) {
    if !cx.has_global::<LogFonts>() {
        let fonts = find_log_fonts(cx);
        cx.set_global(LogFonts(fonts));
    }
}

fn find_log_fonts(cx: &App) -> Vec<String> {
    let installed = cx.text_system().all_font_names();
    LOG_FONTS.iter().filter(|f| **f == DEFAULT_LOG_FONT || installed.iter().any(|n| n == *f)).map(|f| f.to_string()).collect()
}

/// The theme for `settings`, with `theme` choosing light or dark and `density` the row density (the settings' choices,
/// or a pinned pair).
pub fn theme_with(settings: &Settings, theme: ThemeSetting, density: DensitySetting, window: WindowAppearance) -> RoggTheme {
    let a = &settings.appearance;
    let chrome = ChromeStyle::resolved();
    // The chrome first: it resets the log font to the platform default, which the user's choice then replaces.
    RoggTheme::new(appearance_for(theme, window), density_for(density))
        .with_chrome(chrome)
        .with_log_font(log_font_family_for(&a.log_font, chrome), a.log_font_size as f32)
        .with_ui_font_size(a.ui_font_size as f32)
}

pub fn theme_for(settings: &Settings, window: WindowAppearance) -> RoggTheme {
    theme_with(settings, settings.appearance.theme, settings.appearance.density, window)
}

/// Installs the theme for the current settings (with `theme` deciding light or dark and `density` the density) unless
/// it is already active; returns whether it changed.
pub fn apply_theme(theme: ThemeSetting, density: DensitySetting, window: WindowAppearance, cx: &mut App) -> bool {
    let next = theme_with(&SettingsStore::get(cx), theme, density, window);
    if cx.try_global::<RoggTheme>() == Some(&next) {
        return false;
    }
    next.install(cx);
    true
}

/// Applies the cache size limit and trims the cache to it in the background.
pub fn apply_cache_limit(cx: &mut App) {
    CacheStore::get(cx).set_limit(SettingsStore::get(cx).cache_limit_bytes());
    evict_soon(cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_log_font_token_maps_to_the_platform_mono() {
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Mac).as_ref(), MONO_FONT);
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Linux).as_ref(), JETBRAINS_MONO);
        assert_eq!(log_font_family_for(DEFAULT_LOG_FONT, ChromeStyle::Windows).as_ref(), JETBRAINS_MONO);
        assert_eq!(log_font_family_for("Menlo", ChromeStyle::Linux).as_ref(), "Menlo");
        assert_eq!(default_log_font_label(ChromeStyle::Mac), "SF Mono");
        assert_eq!(default_log_font_label(ChromeStyle::Linux), "JetBrains Mono");
    }

    #[test]
    fn theme_follows_the_settings() {
        let mut s = Settings::default();
        assert_eq!(theme_for(&s, WindowAppearance::Dark).appearance, Appearance::Dark);
        assert_eq!(theme_for(&s, WindowAppearance::Light).appearance, Appearance::Light);
        s.appearance.theme = ThemeSetting::Dark;
        s.appearance.log_font = "SF Mono".into();
        let t = theme_for(&s, WindowAppearance::Light);
        assert_eq!(t.appearance, Appearance::Dark);
        assert_eq!(t.fonts.log.as_ref(), rogg_ui::theme::MONO_FONT, "SF Mono is the system monospaced font");
        assert_eq!(t, RoggTheme::new(Appearance::Dark, Density::Compact), "the defaults are the reference theme");
    }
}
