//! What the updater knows: the release on offer, where an update stands, and why one failed.

use rogg_core::update::{Asset, Manifest};
use std::path::PathBuf;

/// A newer build and the asset that fits this install.
#[derive(Clone, Debug)]
pub struct Release {
    pub manifest: Manifest,
    pub asset: Asset,
}

#[derive(Clone, Debug, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking {
        manual: bool,
    },
    UpToDate,
    /// No build number: updates apply to release builds only.
    LocalBuild,
    Available(Release),
    Downloading {
        release: Release,
        done: u64,
        total: Option<u64>,
    },
    Installing(Release),
    /// The new build is in place; a restart starts it.
    Installed(Release),
    /// `release` is set when an update (not a check) failed, so it can be retried.
    Failed {
        release: Option<Release>,
        failure: Failure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Offline,
    TimedOut,
    Http(u16),
    BadManifest,
    NoAsset,
    Checksum,
    Unpack,
    Declined,
    NoPkexec,
    RolledBack,
    /// Where the previous version was left.
    RollbackFailed(PathBuf),
}

impl Failure {
    /// The one-line reason shown in the Update dialog (spec §5).
    pub fn message(&self) -> String {
        match self {
            Failure::Offline => "No network connection.".into(),
            Failure::TimedOut => "The download timed out.".into(),
            Failure::Http(status) => format!("GitHub returned {status}."),
            Failure::BadManifest => "The release information couldn't be read.".into(),
            Failure::NoAsset => "No update is published for this platform.".into(),
            Failure::Checksum => "The download is damaged (checksum mismatch). Nothing was changed.".into(),
            Failure::Unpack => "The download couldn't be unpacked. Nothing was changed.".into(),
            Failure::Declined => "Admin rights were declined.".into(),
            Failure::NoPkexec => "rogg can't ask for admin rights here (pkexec not found).".into(),
            Failure::RolledBack => "The update couldn't be installed. The current version was kept.".into(),
            Failure::RollbackFailed(old) => format!("The update failed. The previous version is at {}.", old.display()),
        }
    }
}
