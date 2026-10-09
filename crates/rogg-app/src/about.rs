//! The About dialog's content (Refinements 1, A4): version, build details, data folders and credits.

use crate::paths::AppPaths;
use gpui_kit::{App, SharedString};
use rogg_ui::ActiveRoggTheme;
use rogg_ui::dialogs::AboutView;
use rogg_ui::theme::ChromeStyle;
use std::path::{Path, PathBuf};

pub const UNKNOWN: &str = "unknown";
/// The Mac style's tagline; `tagline` names the platform of the other styles.
pub const TAGLINE: &str = "A fast log explorer for macOS";

pub fn tagline(chrome: ChromeStyle) -> &'static str {
    match chrome {
        ChromeStyle::Mac => TAGLINE,
        ChromeStyle::Linux => "A fast log explorer for Linux",
        ChromeStyle::Windows => "A fast log explorer for Windows",
    }
}
pub const BUILT_WITH: &str = "Built with Rust, gpui and gpui-kit";

pub struct BuildInfo {
    pub version: &'static str,
    pub commit: &'static str,
    pub date: &'static str,
    pub rustc: &'static str,
}

/// Build details captured by `build.rs`.
pub fn build_info() -> BuildInfo {
    describe(option_env!("ROGG_GIT_COMMIT"), option_env!("ROGG_BUILD_DATE"), option_env!("ROGG_RUSTC_VERSION"))
}

/// `unknown` for each detail the build could not find.
pub fn describe(commit: Option<&'static str>, date: Option<&'static str>, rustc: Option<&'static str>) -> BuildInfo {
    BuildInfo { version: env!("CARGO_PKG_VERSION"), commit: commit.unwrap_or(UNKNOWN), date: date.unwrap_or(UNKNOWN), rustc: rustc.unwrap_or(UNKNOWN) }
}

/// The main open-source crates and their licenses, copied from each crate's manifest on 2026-10-08.
pub const CREDITS: [(&str, &str); 16] = [
    ("gpui-kit", "Apache-2.0"),
    ("regex", "MIT OR Apache-2.0"),
    ("aho-corasick", "Unlicense OR MIT"),
    ("memchr", "Unlicense OR MIT"),
    ("memmap2", "MIT OR Apache-2.0"),
    ("rayon", "MIT OR Apache-2.0"),
    ("notify", "CC0-1.0"),
    ("flate2", "MIT OR Apache-2.0"),
    ("zip", "MIT"),
    ("tar", "MIT OR Apache-2.0"),
    ("encoding_rs", "(Apache-2.0 OR MIT) AND BSD-3-Clause"),
    ("serde", "MIT OR Apache-2.0"),
    ("serde_json", "MIT OR Apache-2.0"),
    ("toml", "MIT OR Apache-2.0"),
    ("Inter (font)", "OFL-1.1"),
    ("JetBrains Mono (font)", "OFL-1.1"),
];

/// The folders the About dialog offers to show in the file manager. From `AppPaths`, so tests see their temp directories.
pub fn data_folders(cx: &App) -> Vec<(SharedString, PathBuf)> {
    cx.try_global::<AppPaths>().map(folders).unwrap_or_default()
}

/// The config folder (settings, profiles) and the data folder (session, history, recent files) as one row when they are
/// the same folder (macOS, Windows), as two when they differ (Linux: `~/.config/rogg`, `~/.local/share/rogg`); then
/// the cache.
pub fn folders(paths: &AppPaths) -> Vec<(SharedString, PathBuf)> {
    let parent = |p: &Path| p.parent().map(Path::to_path_buf).unwrap_or_default();
    let (config, data) = (parent(&paths.settings), parent(&paths.session));
    let mut rows: Vec<(SharedString, PathBuf)> = if config == data {
        vec![("Settings, profiles, session, history and recent files".into(), config)]
    } else {
        vec![("Settings and profiles".into(), config), ("Session, history and recent files".into(), data)]
    };
    rows.push(("Cache".into(), paths.cache.clone()));
    rows
}

pub fn view(cx: &App) -> AboutView {
    let b = build_info();
    AboutView {
        version: format!("rogg {}", b.version).into(),
        tagline: tagline(cx.rogg().chrome).into(),
        build: vec![("Commit".into(), b.commit.into()), ("Built".into(), b.date.into()), ("Rust".into(), b.rustc.into())],
        data: data_folders(cx).into_iter().map(|(label, path)| (label, path.display().to_string().into())).collect(),
        built_with: BUILT_WITH.into(),
        credits: CREDITS.iter().map(|(n, l)| (SharedString::from(*n), SharedString::from(*l))).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_build_details_read_unknown() {
        let b = describe(None, None, None);
        assert_eq!((b.commit, b.date, b.rustc), ("unknown", "unknown", "unknown"));
        assert_eq!(b.version, env!("CARGO_PKG_VERSION"));
        let b = describe(Some("abc1234"), Some("2026-10-08"), Some("rustc 1.99.0"));
        assert_eq!((b.commit, b.date, b.rustc), ("abc1234", "2026-10-08", "rustc 1.99.0"));
    }

    #[test]
    fn credits_list_the_spec_crates_with_licenses() {
        let names: Vec<&str> = CREDITS.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            ["gpui-kit", "regex", "aho-corasick", "memchr", "memmap2", "rayon", "notify", "flate2", "zip", "tar", "encoding_rs", "serde", "serde_json", "toml", "Inter (font)", "JetBrains Mono (font)"]
        );
        assert!(CREDITS.iter().all(|(_, license)| !license.is_empty()));
    }

    #[test]
    fn one_folder_row_when_config_and_data_share_a_folder() {
        let paths = AppPaths::under(Path::new("/support/rogg"));
        let rows = folders(&paths);
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_ref()).collect();
        assert_eq!(labels, ["Settings, profiles, session, history and recent files", "Cache"]);
        assert_eq!(rows[0].1, Path::new("/support/rogg"));
        assert_eq!(rows[1].1, Path::new("/support/rogg/cache"));
    }

    #[test]
    fn separate_config_and_data_folders_get_a_row_each() {
        let (config, data) = (Path::new("/home/u/.config/rogg"), Path::new("/home/u/.local/share/rogg"));
        let paths = AppPaths {
            history: data.join("history.json"),
            recent: data.join("recent.json"),
            cache: PathBuf::from("/home/u/.cache/rogg"),
            settings: config.join("settings.toml"),
            profiles: config.join("profiles"),
            session: data.join("session.json"),
        };
        let rows = folders(&paths);
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_ref()).collect();
        assert_eq!(labels, ["Settings and profiles", "Session, history and recent files", "Cache"]);
        assert_eq!(rows.iter().map(|(_, p)| p.as_path()).collect::<Vec<_>>(), [config, data, Path::new("/home/u/.cache/rogg")]);
    }

    #[test]
    fn the_tagline_names_the_platform() {
        assert_eq!(tagline(ChromeStyle::Mac), "A fast log explorer for macOS");
        assert_eq!(tagline(ChromeStyle::Linux), "A fast log explorer for Linux");
        assert_eq!(tagline(ChromeStyle::Windows), "A fast log explorer for Windows");
    }
}
