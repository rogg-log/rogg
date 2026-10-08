//! The About dialog's content (Refinements 1, A4): version, build details, data folders and credits.

use crate::paths::AppPaths;
use gpui_kit::{App, SharedString};
use rogg_ui::dialogs::AboutView;
use std::path::{Path, PathBuf};

pub const UNKNOWN: &str = "unknown";
pub const TAGLINE: &str = "A fast log explorer for macOS";
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

/// The folders the About dialog offers to show in Finder: the one holding settings, profiles, session, history and
/// recent files, then the cache. From `AppPaths`, so tests see their temp directories.
pub fn data_folders(cx: &App) -> Vec<(SharedString, PathBuf)> {
    let Some(paths) = cx.try_global::<AppPaths>() else { return vec![] };
    let support = paths.settings.parent().map(Path::to_path_buf).unwrap_or_default();
    vec![("Settings, profiles, session, history and recent files".into(), support), ("Cache".into(), paths.cache.clone())]
}

pub fn view(cx: &App) -> AboutView {
    let b = build_info();
    AboutView {
        version: format!("rogg {}", b.version).into(),
        tagline: TAGLINE.into(),
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
}
