//! Self-update building blocks: the release manifest, how this copy of rogg is installed and which release asset
//! fits it. Pure and network-free; downloading and replacing live elsewhere.

use crate::platform::Os;
use serde::Deserialize;
use std::io;
use std::path::{Component, Path, PathBuf};

pub mod apply;

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("the release information couldn't be read: {0}")]
    BadManifest(String),
    #[error("checksum mismatch")]
    Checksum,
    #[error("the download couldn't be read: {0}")]
    Read(#[source] io::Error),
    #[error("the download is larger than the {limit}-byte limit")]
    TooLarge { limit: u64 },
    #[error("the download couldn't be unpacked: {0}")]
    Unpack(String),
    #[error("the archive has no {0}")]
    MissingItem(&'static str),
    #[error("{} is not a rogg install", .0.display())]
    NotRogg(PathBuf),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("replacing failed (rolled back: {rolled_back}); the previous version is at {}: {source}", old.display())]
    Replace {
        rolled_back: bool,
        old: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub os: String,
    pub arch: String,
    pub sha256: String,
    pub size: u64,
}

/// `manifest.json` as CI publishes it; unknown fields are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub build: u64,
    pub commit: String,
    pub date: String,
    #[serde(default)]
    pub channel: String,
    pub assets: Vec<Asset>,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Manifest, UpdateError> {
        serde_json::from_str(json).map_err(|e| UpdateError::BadManifest(e.to_string()))
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    pub fn short_commit(&self) -> &str {
        match self.commit.char_indices().nth(7) {
            Some((i, _)) => &self.commit[..i],
            None => &self.commit,
        }
    }

    pub fn release_url(&self) -> String {
        format!("https://github.com/rogg-log/rogg/releases/tag/v{}-build.{}", self.version, self.build)
    }
}

/// Only a strictly higher build is offered.
pub fn is_newer(available: u64, running: u64) -> bool {
    available > running
}

/// What gets replaced by an update.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    App { bundle: PathBuf },
    Binary { path: PathBuf },
    Unsupported,
}

impl Install {
    /// Classifies the running executable (symlinks resolved; the path as given if it can't be resolved).
    pub fn detect(exe: &Path, os: Os) -> Install {
        let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
        if os == Os::Mac {
            if exe.components().any(|c| c == Component::Normal("AppTranslocation".as_ref())) {
                return Install::Unsupported;
            }
            let macos = exe.parent();
            let contents = macos.and_then(Path::parent);
            let bundle = contents.and_then(Path::parent);
            if let (Some(macos), Some(contents), Some(bundle)) = (macos, contents, bundle)
                && macos.file_name().is_some_and(|n| n == "MacOS")
                && contents.file_name().is_some_and(|n| n == "Contents")
                && bundle.extension().is_some_and(|e| e == "app")
            {
                return Install::App { bundle: bundle.to_path_buf() };
            }
        }
        Install::Binary { path: exe }
    }

    pub fn target(&self) -> Option<&Path> {
        match self {
            Install::App { bundle } => Some(bundle),
            Install::Binary { path } => Some(path),
            Install::Unsupported => None,
        }
    }
}

/// The release asset that fits this install, if one is published. `arch` is `std::env::consts::ARCH`.
pub fn asset_name(install: &Install, os: Os, arch: &str) -> Option<&'static str> {
    match (install, os, arch) {
        (Install::Unsupported, ..) => None,
        (Install::App { .. }, Os::Mac, _) => Some("rogg-macos-universal-app.tar.gz"),
        (_, Os::Mac, _) => Some("rogg-macos-universal.tar.gz"),
        (_, Os::Linux, "x86_64") => Some("rogg-linux-x86_64.tar.gz"),
        (_, Os::Linux, "aarch64") => Some("rogg-linux-arm64.tar.gz"),
        (_, Os::Windows, "x86_64") => Some("rogg-windows-x86_64.zip"),
        _ => None,
    }
}
